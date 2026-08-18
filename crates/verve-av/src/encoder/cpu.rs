//! The CPU backend: libx264 and libx265, the only implementation we ship.
//! Everything libx264-specific stops at this file.

use super::{
    BackendId, CancelSignal, EncodeSpec, EncoderBackend, RateControl, VideoCodecName, VideoEncoder,
};
use crate::error::Result;
use std::sync::Arc;

/// libx264 / libx265 on the CPU.
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuBackend {
    _private: (),
}

impl CpuBackend {
    /// A CPU backend.
    pub const fn new() -> Self {
        Self { _private: () }
    }

    /// The FFmpeg encoder name for `codec`. The only place these appear.
    pub const fn encoder_name(codec: VideoCodecName) -> &'static str {
        match codec {
            VideoCodecName::H264 => "libx264",
            VideoCodecName::H265 => "libx265",
            VideoCodecName::Av1 => "libsvtav1",
        }
    }

    /// Options implied by a spec. Split out so the mapping is testable without
    /// FFmpeg — a missing ceiling is a quality bug that never throws.
    pub fn options(spec: &EncodeSpec) -> Vec<(&'static str, String)> {
        let mut opts: Vec<(&'static str, String)> = Vec::new();
        opts.push(("preset", spec.preset.as_str().to_string()));

        // x265 takes one colon-separated string; x264 takes them individually.
        let x265 = spec.codec == VideoCodecName::H265;

        match spec.rate_control {
            RateControl::Crf {
                crf,
                max_bitrate,
                buffer_size,
            } => {
                opts.push(("crf", crf.to_string()));
                if x265 {
                    opts.push((
                        "x265-params",
                        format!(
                            "vbv-maxrate={}:vbv-bufsize={}:keyint={}:min-keyint={}:scenecut=0:open-gop=0",
                            max_bitrate / 1000,
                            buffer_size / 1000,
                            spec.gop_frames,
                            spec.gop_frames,
                        ),
                    ));
                } else {
                    opts.push(("maxrate", max_bitrate.to_string()));
                    opts.push(("bufsize", buffer_size.to_string()));
                }
            }
            RateControl::ConstantBitrate { bitrate } => {
                opts.push(("b", bitrate.to_string()));
            }
        }

        if !x265 {
            // Split rule 2: scene cuts would land at rung-dependent positions.
            opts.push(("keyint_min", spec.gop_frames.to_string()));
            opts.push(("sc_threshold", "0".to_string()));
            opts.push(("x264-params", "open-gop=0:scenecut=0".to_string()));
        }

        for (k, v) in &spec.extra {
            // Extras are few and per-open; a lifetime here is not worth it.
            opts.push((Box::leak(k.clone().into_boxed_str()), v.clone()));
        }
        opts
    }
}

impl EncoderBackend for CpuBackend {
    fn id(&self) -> BackendId {
        BackendId::Cpu
    }

    fn supports(&self, codec: VideoCodecName) -> bool {
        // Asked of the linked FFmpeg so a build without libx265 says so here.
        #[cfg(feature = "ffmpeg")]
        {
            ffmpeg_next::encoder::find_by_name(Self::encoder_name(codec)).is_some()
        }
        #[cfg(not(feature = "ffmpeg"))]
        {
            let _ = codec;
            false
        }
    }

    fn open(
        &self,
        spec: &EncodeSpec,
        cancel: Arc<dyn CancelSignal>,
    ) -> Result<Box<dyn VideoEncoder>> {
        spec.validate()?;
        #[cfg(feature = "ffmpeg")]
        {
            Ok(Box::new(imp::CpuEncoder::open(spec, cancel)?))
        }
        #[cfg(not(feature = "ffmpeg"))]
        {
            let _ = cancel;
            Err(crate::error::AvError::NotBuiltWithFfmpeg)
        }
    }
}

#[cfg(feature = "ffmpeg")]
mod imp {
    use super::*;
    use crate::encoder::{Frame, Packet, PixelFormat, Provenance};
    use crate::error::AvError;
    use ffmpeg_next as ff;

    fn wrap(op: &'static str) -> impl Fn(ff::Error) -> AvError {
        move |e| AvError::Ffmpeg {
            op,
            source: Box::new(e),
        }
    }

    pub(super) struct CpuEncoder {
        encoder: ff::encoder::Video,
        cancel: Arc<dyn CancelSignal>,
        codec: VideoCodecName,
        encoder_name: &'static str,
        finished: bool,
    }

    impl CpuEncoder {
        pub(super) fn open(spec: &EncodeSpec, cancel: Arc<dyn CancelSignal>) -> Result<Self> {
            let name = CpuBackend::encoder_name(spec.codec);
            let codec =
                ff::encoder::find_by_name(name).ok_or_else(|| AvError::CodecUnavailable {
                    codec: spec.codec.to_string(),
                    reason: format!("this FFmpeg build has no {name}"),
                })?;

            let mut ctx = ff::codec::context::Context::new_with_codec(codec)
                .encoder()
                .video()
                .map_err(wrap("open video encoder"))?;

            ctx.set_width(spec.resolution.width);
            ctx.set_height(spec.resolution.height);
            ctx.set_format(match spec.pixel_format {
                PixelFormat::Yuv420p => ff::format::Pixel::YUV420P,
                PixelFormat::Yuv420p10le => ff::format::Pixel::YUV420P10LE,
            });
            // Time base is 1/fps, so a pts is a frame index. Joins need that.
            ctx.set_time_base(ff::Rational(spec.frame_rate.den, spec.frame_rate.num));
            ctx.set_frame_rate(Some(ff::Rational(spec.frame_rate.num, spec.frame_rate.den)));
            ctx.set_gop(spec.gop_frames);
            ctx.set_threading(ff::codec::threading::Config {
                kind: ff::codec::threading::Type::Frame,
                count: usize::from(spec.threads),
            });
            if spec.closed_gop {
                let mut flags = ff::codec::Flags::CLOSED_GOP;
                flags.insert(ff::codec::Flags::GLOBAL_HEADER);
                ctx.set_flags(flags);
            }

            let mut dict = ff::Dictionary::new();
            for (k, v) in CpuBackend::options(spec) {
                dict.set(k, &v);
            }

            let encoder = ctx.open_with(dict).map_err(wrap("configure encoder"))?;

            Ok(Self {
                encoder,
                cancel,
                codec: spec.codec,
                encoder_name: name,
                finished: false,
            })
        }
    }

    impl VideoEncoder for CpuEncoder {
        fn send_frame(&mut self, frame: &Frame) -> Result<()> {
            // Between frames, not between chunks.
            if self.cancel.is_cancelled() {
                return Err(AvError::Cancelled);
            }
            let inner = frame
                .inner
                .as_ref()
                .ok_or_else(|| AvError::InvalidSpec("frame carries no picture data".into()))?;
            self.encoder.send_frame(inner).map_err(wrap("send frame"))
        }

        fn finish(&mut self) -> Result<()> {
            if !self.finished {
                self.encoder.send_eof().map_err(wrap("send eof"))?;
                self.finished = true;
            }
            Ok(())
        }

        fn receive_packet(&mut self) -> Result<Option<Packet>> {
            let mut pkt = ff::Packet::empty();
            match self.encoder.receive_packet(&mut pkt) {
                Ok(()) => Ok(Some(Packet {
                    pts: pkt.pts().unwrap_or_default(),
                    dts: pkt.dts().unwrap_or_default(),
                    keyframe: pkt.is_key(),
                    data: pkt.data().unwrap_or_default().to_vec(),
                })),
                // Both mean "nothing right now".
                Err(ff::Error::Other { errno }) if errno == libc_eagain() => Ok(None),
                Err(ff::Error::Eof) => Ok(None),
                Err(e) => Err(wrap("receive packet")(e)),
            }
        }

        fn provenance(&self) -> Provenance {
            Provenance {
                backend: BackendId::Cpu,
                encoder: self.encoder_name.to_string(),
                encoder_version: None,
                ffmpeg_version: crate::ffmpeg_version(),
            }
        }
    }

    // EAGAIN: 35 on macOS/BSD, 11 on Linux.
    const fn libc_eagain() -> i32 {
        if cfg!(any(target_os = "macos", target_os = "ios")) {
            35
        } else {
            11
        }
    }

    impl std::fmt::Debug for CpuEncoder {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("CpuEncoder")
                .field("codec", &self.codec)
                .field("encoder", &self.encoder_name)
                .finish()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rational;
    use crate::encoder::{Preset, Resolution};

    fn spec(codec: VideoCodecName) -> EncodeSpec {
        EncodeSpec::new(codec, Resolution::new(1280, 720), Rational::new(30, 1))
            .with_preset(Preset::Veryfast)
            .with_rate_control(RateControl::crf(23, 3_000_000))
    }

    fn opt<'a>(opts: &'a [(&'static str, String)], key: &str) -> Option<&'a str> {
        opts.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }

    #[test]
    fn only_this_file_knows_the_encoder_names() {
        assert_eq!(CpuBackend::encoder_name(VideoCodecName::H264), "libx264");
        assert_eq!(CpuBackend::encoder_name(VideoCodecName::H265), "libx265");
    }

    #[test]
    fn x264_gets_a_ceiling_and_a_buffer() {
        let opts = CpuBackend::options(&spec(VideoCodecName::H264));
        assert_eq!(opt(&opts, "crf"), Some("23"));
        assert_eq!(opt(&opts, "maxrate"), Some("3000000"));
        assert_eq!(opt(&opts, "bufsize"), Some("6000000"));
        assert_eq!(opt(&opts, "preset"), Some("veryfast"));
    }

    #[test]
    fn scene_cut_detection_is_off_in_both_encoders() {
        let x264 = CpuBackend::options(&spec(VideoCodecName::H264));
        assert_eq!(opt(&x264, "sc_threshold"), Some("0"));
        assert!(opt(&x264, "x264-params").unwrap().contains("scenecut=0"));

        let x265 = CpuBackend::options(&spec(VideoCodecName::H265));
        let params = opt(&x265, "x265-params").unwrap();
        assert!(params.contains("scenecut=0"), "{params}");
        assert!(params.contains("open-gop=0"), "{params}");
        assert!(params.contains("keyint=60"), "{params}");
        assert!(params.contains("min-keyint=60"), "{params}");
    }

    #[test]
    fn x265_gets_its_ceiling_in_kilobits() {
        let opts = CpuBackend::options(&spec(VideoCodecName::H265));
        let params = opt(&opts, "x265-params").unwrap();
        assert!(params.contains("vbv-maxrate=3000"), "{params}");
        assert!(params.contains("vbv-bufsize=6000"), "{params}");
    }

    #[test]
    fn extras_reach_the_encoder() {
        let s = spec(VideoCodecName::H264).with_extra("tune", "film");
        let opts = CpuBackend::options(&s);
        assert_eq!(opt(&opts, "tune"), Some("film"));
    }

    #[test]
    fn a_bad_spec_is_refused_before_ffmpeg_is_touched() {
        let mut bad = spec(VideoCodecName::H264);
        bad.threads = 64;
        let err = CpuBackend::new()
            .open(&bad, Arc::new(crate::NeverCancel))
            .unwrap_err();
        assert!(matches!(err, crate::AvError::InvalidSpec(_)), "{err}");
    }

    #[cfg(not(feature = "ffmpeg"))]
    #[test]
    fn without_the_feature_the_backend_supports_nothing_and_says_why() {
        let b = CpuBackend::new();
        assert!(!b.supports(VideoCodecName::H264));
        assert!(matches!(
            b.open(&spec(VideoCodecName::H264), Arc::new(crate::NeverCancel)),
            Err(crate::AvError::NotBuiltWithFfmpeg)
        ));
    }

    #[cfg(feature = "ffmpeg")]
    #[test]
    fn the_shipped_registry_can_encode_h264() {
        crate::init().unwrap();
        let registry = crate::BackendRegistry::with_shipped_backends();
        let backend = registry.for_codec(VideoCodecName::H264).unwrap();
        assert_eq!(backend.id(), BackendId::Cpu);
        let enc = registry
            .open(&spec(VideoCodecName::H264), Arc::new(crate::NeverCancel))
            .unwrap();
        let p = enc.provenance();
        assert_eq!(p.backend, BackendId::Cpu);
        assert_eq!(p.encoder, "libx264");
        assert!(!p.ffmpeg_version.is_empty());
    }
}
