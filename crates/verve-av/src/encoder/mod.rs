//! The encoder backend seam. The pipeline is written against these traits and
//! never names a backend; what ran is recorded per rendition as [`Provenance`].

mod cpu;
mod spec;

pub use cpu::CpuBackend;
pub use spec::{EncodeSpec, PixelFormat, Preset, RateControl, Resolution, VideoCodecName};

use crate::error::{AvError, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Which implementation produced a rendition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendId {
    /// libx264 / libx265. The only implementation we ship.
    Cpu,
    /// NVIDIA NVENC. Seam only.
    Nvenc,
    /// Intel Quick Sync. Seam only.
    Qsv,
    /// VA-API. Seam only.
    Vaapi,
    /// A deterministic in-process fake, for tests and `--dry-run`.
    Null,
}

impl BackendId {
    /// Stable string for storage and logs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Nvenc => "nvenc",
            Self::Qsv => "qsv",
            Self::Vaapi => "vaapi",
            Self::Null => "null",
        }
    }
}

/// Everything needed to reproduce a rendition. Stored per rendition, not per
/// fleet — a worker upgraded mid-ladder must not make the record a lie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Which backend ran.
    pub backend: BackendId,
    /// The encoder as the backend names it, e.g. `libx264`.
    pub encoder: String,
    /// Version string of the encoder library, when it exposes one.
    pub encoder_version: Option<String>,
    /// The FFmpeg build we were linked against.
    pub ffmpeg_version: String,
}

/// A raw frame handed to an encoder. Without the `ffmpeg` feature it carries
/// geometry only, so the seam still compiles and tests.
pub struct Frame {
    /// Presentation timestamp in the encoder's time base.
    pub pts: i64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    #[cfg(feature = "ffmpeg")]
    pub(crate) inner: Option<ffmpeg_next::frame::Video>,
}

impl std::fmt::Debug for Frame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Frame")
            .field("pts", &self.pts)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

impl Frame {
    /// A frame carrying geometry only. Test scaffolding.
    #[cfg(test)]
    pub fn placeholder(pts: i64, width: u32, height: u32) -> Self {
        Self {
            pts,
            width,
            height,
            #[cfg(feature = "ffmpeg")]
            inner: None,
        }
    }
}

/// An encoded packet coming back out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Presentation timestamp in the encoder's time base.
    pub pts: i64,
    /// Decode timestamp in the encoder's time base.
    pub dts: i64,
    /// Starts a GOP. The split planner and the check step depend on this.
    pub keyframe: bool,
    /// Compressed payload.
    pub data: Vec<u8>,
}

#[cfg(feature = "ffmpeg")]
impl Frame {
    /// Wrap a decoded FFmpeg frame. Decode-once hands one to every rung.
    pub fn from_video(pts: i64, inner: ffmpeg_next::frame::Video) -> Self {
        Self {
            pts,
            width: inner.width(),
            height: inner.height(),
            inner: Some(inner),
        }
    }

    /// A blank mid-grey 4:2:0 frame, for tests and synthetic bench inputs.
    pub fn blank_yuv420p(pts: i64, width: u32, height: u32) -> Self {
        let mut v =
            ffmpeg_next::frame::Video::new(ffmpeg_next::format::Pixel::YUV420P, width, height);
        v.data_mut(0).fill(128);
        v.data_mut(1).fill(128);
        v.data_mut(2).fill(128);
        v.set_pts(Some(pts));
        Self::from_video(pts, v)
    }
}

/// Asked between frames so a cancelled job stops burning cores.
pub trait CancelSignal: Send + Sync {
    /// `true` once the caller wants the encode to stop.
    fn is_cancelled(&self) -> bool;
}

/// Never cancels.
#[derive(Debug, Clone, Copy, Default)]
pub struct NeverCancel;

impl CancelSignal for NeverCancel {
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// One open encoder producing one rendition. Lifecycle: `send_frame`* →
/// `finish` → `receive_packet`* until `None`. Drain between frames or hit EAGAIN.
pub trait VideoEncoder: Send + std::fmt::Debug {
    /// Feed one frame. Returns [`AvError::Cancelled`] if the signal fired.
    fn send_frame(&mut self, frame: &Frame) -> Result<()>;

    /// Signal end of stream. No frames may be sent afterwards.
    fn finish(&mut self) -> Result<()>;

    /// Take the next available packet, or `None` when the encoder is drained.
    fn receive_packet(&mut self) -> Result<Option<Packet>>;

    /// What ran, for the rendition record.
    fn provenance(&self) -> Provenance;
}

/// A family of encoders — CPU, or one day a GPU.
pub trait EncoderBackend: Send + Sync + std::fmt::Debug {
    /// Which backend this is.
    fn id(&self) -> BackendId;

    /// Whether this build can produce `codec`. Asked of FFmpeg, not hardcoded.
    fn supports(&self, codec: VideoCodecName) -> bool;

    /// Open an encoder. Validates `spec` before allocating anything.
    fn open(
        &self,
        spec: &EncodeSpec,
        cancel: Arc<dyn CancelSignal>,
    ) -> Result<Box<dyn VideoEncoder>>;
}

/// Backends available to this process, in preference order. Callers ask for a
/// codec, never for a backend by name.
#[derive(Debug, Clone, Default)]
pub struct BackendRegistry {
    backends: Vec<Arc<dyn EncoderBackend>>,
}

impl BackendRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// What a worker gets by default: CPU, and only CPU.
    pub fn with_shipped_backends() -> Self {
        let mut registry = Self::new();
        registry.register(Arc::new(CpuBackend::new()));
        registry
    }

    /// Add a backend at the end of the preference order.
    pub fn register(&mut self, backend: Arc<dyn EncoderBackend>) -> &mut Self {
        self.backends.push(backend);
        self
    }

    /// Every registered backend.
    pub fn backends(&self) -> &[Arc<dyn EncoderBackend>] {
        &self.backends
    }

    /// The first registered backend that supports `codec`.
    pub fn for_codec(&self, codec: VideoCodecName) -> Result<&Arc<dyn EncoderBackend>> {
        self.backends
            .iter()
            .find(|b| b.supports(codec))
            .ok_or(AvError::NoBackend { codec })
    }

    /// Open an encoder for `spec` using the first backend that can.
    pub fn open(
        &self,
        spec: &EncodeSpec,
        cancel: Arc<dyn CancelSignal>,
    ) -> Result<Box<dyn VideoEncoder>> {
        self.for_codec(spec.codec)?.open(spec, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves the seam is usable without FFmpeg.
    #[derive(Debug)]
    struct FakeBackend(VideoCodecName);

    impl EncoderBackend for FakeBackend {
        fn id(&self) -> BackendId {
            BackendId::Null
        }
        fn supports(&self, codec: VideoCodecName) -> bool {
            codec == self.0
        }
        fn open(
            &self,
            spec: &EncodeSpec,
            cancel: Arc<dyn CancelSignal>,
        ) -> Result<Box<dyn VideoEncoder>> {
            spec.validate()?;
            Ok(Box::new(FakeEncoder {
                cancel,
                pending: Vec::new(),
                sent: 0,
                gop: spec.gop_frames,
            }))
        }
    }

    struct FakeEncoder {
        cancel: Arc<dyn CancelSignal>,
        pending: Vec<Packet>,
        sent: u32,
        gop: u32,
    }

    impl std::fmt::Debug for FakeEncoder {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("FakeEncoder")
                .field("sent", &self.sent)
                .finish()
        }
    }

    impl VideoEncoder for FakeEncoder {
        fn send_frame(&mut self, frame: &Frame) -> Result<()> {
            if self.cancel.is_cancelled() {
                return Err(AvError::Cancelled);
            }
            self.pending.push(Packet {
                pts: frame.pts,
                dts: frame.pts,
                keyframe: self.sent.is_multiple_of(self.gop),
                data: vec![0u8; 8],
            });
            self.sent += 1;
            Ok(())
        }
        fn finish(&mut self) -> Result<()> {
            Ok(())
        }
        fn receive_packet(&mut self) -> Result<Option<Packet>> {
            Ok(if self.pending.is_empty() {
                None
            } else {
                Some(self.pending.remove(0))
            })
        }
        fn provenance(&self) -> Provenance {
            Provenance {
                backend: BackendId::Null,
                encoder: "null".into(),
                encoder_version: None,
                ffmpeg_version: "none".into(),
            }
        }
    }

    struct AlwaysCancelled;
    impl CancelSignal for AlwaysCancelled {
        fn is_cancelled(&self) -> bool {
            true
        }
    }

    fn spec(codec: VideoCodecName) -> EncodeSpec {
        EncodeSpec::new(
            codec,
            Resolution::new(1280, 720),
            crate::Rational::new(30, 1),
        )
    }

    #[test]
    fn registry_picks_the_first_backend_that_supports_the_codec() {
        let mut registry = BackendRegistry::new();
        registry.register(Arc::new(FakeBackend(VideoCodecName::H265)));
        registry.register(Arc::new(FakeBackend(VideoCodecName::H264)));

        assert!(registry.for_codec(VideoCodecName::H264).is_ok());
        assert!(matches!(
            registry.for_codec(VideoCodecName::Av1),
            Err(AvError::NoBackend {
                codec: VideoCodecName::Av1
            })
        ));
    }

    #[test]
    fn the_caller_asks_for_a_codec_and_never_for_a_backend() {
        let mut registry = BackendRegistry::new();
        registry.register(Arc::new(FakeBackend(VideoCodecName::H264)));

        let mut enc = registry
            .open(&spec(VideoCodecName::H264), Arc::new(NeverCancel))
            .unwrap();

        for pts in 0..5 {
            enc.send_frame(&Frame::placeholder(pts, 1280, 720)).unwrap();
        }
        enc.finish().unwrap();

        let mut packets = Vec::new();
        while let Some(p) = enc.receive_packet().unwrap() {
            packets.push(p);
        }
        assert_eq!(packets.len(), 5);
        assert!(
            packets[0].keyframe,
            "first packet of a chunk must be a keyframe"
        );
        assert_eq!(enc.provenance().backend, BackendId::Null);
    }

    #[test]
    fn cancellation_reaches_a_running_encode() {
        let backend = FakeBackend(VideoCodecName::H264);
        let mut enc = backend
            .open(&spec(VideoCodecName::H264), Arc::new(AlwaysCancelled))
            .unwrap();
        assert!(matches!(
            enc.send_frame(&Frame::placeholder(0, 1280, 720)),
            Err(AvError::Cancelled)
        ));
    }

    #[test]
    fn a_bad_spec_fails_at_open_not_mid_encode() {
        let backend = FakeBackend(VideoCodecName::H264);
        let mut bad = spec(VideoCodecName::H264);
        bad.resolution = Resolution::new(1281, 720);
        assert!(matches!(
            backend.open(&bad, Arc::new(NeverCancel)),
            Err(AvError::InvalidSpec(_))
        ));
    }

    #[test]
    fn backend_ids_have_stable_strings() {
        assert_eq!(BackendId::Cpu.as_str(), "cpu");
        assert_eq!(
            serde_json::to_string(&BackendId::Nvenc).unwrap(),
            r#""nvenc""#
        );
    }
}
