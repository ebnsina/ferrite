//! What to encode, in terms no backend may widen. Backend-shaped knobs go in
//! [`EncodeSpec::extra`].

use crate::Rational;
use crate::error::{AvError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Output codecs the ladder can ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VideoCodecName {
    /// H.264 / AVC. The compatibility floor.
    H264,
    /// H.265 / HEVC.
    H265,
    /// AV1. Smaller, very slow — a `bulk`-lane re-encode, not a default rung.
    Av1,
}

impl VideoCodecName {
    /// Stable string for storage, logs and CLI flags.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::H265 => "h265",
            Self::Av1 => "av1",
        }
    }
}

impl std::fmt::Display for VideoCodecName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for VideoCodecName {
    type Err = AvError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "h264" | "avc" | "x264" | "libx264" => Ok(Self::H264),
            "h265" | "hevc" | "x265" | "libx265" => Ok(Self::H265),
            "av1" => Ok(Self::Av1),
            other => Err(AvError::InvalidSpec(format!("unknown codec {other:?}"))),
        }
    }
}

/// Output pixel format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PixelFormat {
    /// 8-bit 4:2:0. What everything plays.
    #[default]
    Yuv420p,
    /// 10-bit 4:2:0. Needed for HDR passthrough.
    Yuv420p10le,
}

impl PixelFormat {
    /// FFmpeg's name for this format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yuv420p => "yuv420p",
            Self::Yuv420p10le => "yuv420p10le",
        }
    }
}

/// Speed/efficiency tradeoff. Comes from the plan, not the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    /// Fast path. Latency beats efficiency for one rung.
    Ultrafast,
    /// Faster than default; the usual fast-path choice.
    Veryfast,
    /// Quality path, standard plans.
    Medium,
    /// Premium plans and `bulk`-lane re-encodes.
    Slow,
    /// ~3× the cost of `medium`.
    Slower,
}

impl Preset {
    /// The name libx264/libx265 expect.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ultrafast => "ultrafast",
            Self::Veryfast => "veryfast",
            Self::Medium => "medium",
            Self::Slow => "slow",
            Self::Slower => "slower",
        }
    }
}

/// How bitrate is chosen. Two-pass is deliberately not representable: split
/// rule 5, it needs whole-video context a chunk does not have.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum RateControl {
    /// Constant quality with a hard ceiling.
    Crf {
        /// Quality target. Lower is better; 18–28 is the useful range.
        crf: u8,
        /// Ceiling in bits/s, so one hard chunk cannot break ABR switching.
        max_bitrate: u32,
        /// VBV buffer in bits, conventionally 1–2s of ceiling.
        buffer_size: u32,
    },
    /// Fixed average bitrate. Job-mode outputs only, never a ladder rung.
    ConstantBitrate {
        /// Target in bits per second.
        bitrate: u32,
    },
}

impl RateControl {
    /// CRF with a ceiling and a 2s VBV buffer.
    pub const fn crf(crf: u8, max_bitrate: u32) -> Self {
        Self::Crf {
            crf,
            max_bitrate,
            buffer_size: max_bitrate.saturating_mul(2),
        }
    }
}

impl Default for RateControl {
    fn default() -> Self {
        Self::crf(23, 5_000_000)
    }
}

/// Output frame size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Resolution {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Resolution {
    /// A new resolution.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl std::fmt::Display for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

/// One rendition's encode. Two chunks of a rung differ only in input, which is
/// what lets a retried chunk reproduce identical boundaries (split rule 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncodeSpec {
    /// Output codec.
    pub codec: VideoCodecName,
    /// Output frame size.
    pub resolution: Resolution,
    /// Output frame rate. Fixed by the mezzanine, so exact.
    pub frame_rate: Rational,
    /// Output pixel format.
    pub pixel_format: PixelFormat,
    /// Speed/efficiency tradeoff, from the plan.
    pub preset: Preset,
    /// Bitrate policy.
    pub rate_control: RateControl,
    /// Fixed GOP in frames. Identical across every rung (split rule 2) or
    /// players corrupt when switching quality.
    pub gop_frames: u32,
    /// No keyframes off the GOP grid. Scene cuts would land rung-dependently.
    pub closed_gop: bool,
    /// Threads for this encoder. Encoders stop scaling past ~16; on 64 cores
    /// 8 chunks × 8 threads beats 2 × 32 by ~30%.
    pub threads: u16,
    /// Backend-specific options. At most one backend understands a given key.
    pub extra: BTreeMap<String, String>,
}

/// Above this, more threads cost quality and buy no speed.
pub const MAX_USEFUL_THREADS: u16 = 16;

impl EncodeSpec {
    /// Safe defaults: CRF with a ceiling, closed GOP, medium preset.
    pub fn new(codec: VideoCodecName, resolution: Resolution, frame_rate: Rational) -> Self {
        Self {
            codec,
            resolution,
            frame_rate,
            pixel_format: PixelFormat::default(),
            preset: Preset::Medium,
            rate_control: RateControl::default(),
            gop_frames: default_gop_frames(frame_rate),
            closed_gop: true,
            threads: 8,
            extra: BTreeMap::new(),
        }
    }

    /// Set the preset.
    pub fn with_preset(mut self, preset: Preset) -> Self {
        self.preset = preset;
        self
    }

    /// Set the bitrate policy.
    pub fn with_rate_control(mut self, rate_control: RateControl) -> Self {
        self.rate_control = rate_control;
        self
    }

    /// Set the thread count.
    pub fn with_threads(mut self, threads: u16) -> Self {
        self.threads = threads;
        self
    }

    /// Add a backend-specific option.
    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }

    /// Reject a spec before anything is allocated. Every backend calls this.
    pub fn validate(&self) -> Result<()> {
        let invalid = |msg: String| Err(AvError::InvalidSpec(msg));

        if self.resolution.width == 0 || self.resolution.height == 0 {
            return invalid(format!("zero-sized output {}", self.resolution));
        }
        if !self.resolution.width.is_multiple_of(2) || !self.resolution.height.is_multiple_of(2) {
            return invalid(format!(
                "{} has an odd dimension; 4:2:0 chroma needs both even",
                self.resolution
            ));
        }
        if self.frame_rate.num <= 0 || self.frame_rate.den <= 0 {
            return invalid(format!("frame rate {} is not positive", self.frame_rate));
        }
        if self.gop_frames == 0 {
            return invalid("gop_frames must be at least 1".into());
        }
        if !self.closed_gop {
            return invalid(
                "open GOP places keyframes off the grid; split rule 2 requires \
                 identical cut points across every rung"
                    .into(),
            );
        }
        if self.threads == 0 {
            return invalid("threads must be at least 1".into());
        }
        if self.threads > MAX_USEFUL_THREADS {
            return invalid(format!(
                "{} threads exceeds the useful maximum of {MAX_USEFUL_THREADS}; \
                 run more chunks in parallel instead",
                self.threads
            ));
        }
        match self.rate_control {
            RateControl::Crf {
                crf,
                max_bitrate,
                buffer_size,
            } => {
                if crf > 51 {
                    return invalid(format!("crf {crf} is outside 0..=51"));
                }
                if max_bitrate == 0 {
                    return invalid(
                        "CRF needs a ceiling; an unbounded rung breaks ABR switching".into(),
                    );
                }
                if buffer_size == 0 {
                    return invalid("VBV buffer must be non-zero when a ceiling is set".into());
                }
            }
            RateControl::ConstantBitrate { bitrate } => {
                if bitrate == 0 {
                    return invalid("constant bitrate must be non-zero".into());
                }
            }
        }
        if self.pixel_format == PixelFormat::Yuv420p10le && self.codec == VideoCodecName::H264 {
            return invalid("10-bit H.264 is not a compatibility floor; use H.265".into());
        }
        Ok(())
    }
}

/// A GOP as near 2s as whole frames allow; 2s divides the ~10s chunk target.
fn default_gop_frames(frame_rate: Rational) -> u32 {
    let fps = frame_rate.as_f64();
    if fps <= 0.0 {
        return 48;
    }
    (fps * 2.0).round().max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> EncodeSpec {
        EncodeSpec::new(
            VideoCodecName::H264,
            Resolution::new(1920, 1080),
            Rational::new(30000, 1001),
        )
    }

    #[test]
    fn defaults_are_the_documented_safe_ones() {
        let s = spec();
        s.validate().unwrap();
        assert!(s.closed_gop);
        assert!(matches!(s.rate_control, RateControl::Crf { .. }));
        assert_eq!(s.gop_frames, 60, "29.97fps rounds to a 2s GOP of 60 frames");
    }

    #[test]
    fn odd_dimensions_are_rejected() {
        let mut s = spec();
        s.resolution = Resolution::new(1919, 1080);
        assert!(matches!(s.validate(), Err(AvError::InvalidSpec(_))));
    }

    #[test]
    fn open_gop_is_rejected_because_split_rule_2_forbids_it() {
        let mut s = spec();
        s.closed_gop = false;
        let err = s.validate().unwrap_err().to_string();
        assert!(err.contains("split rule 2"), "{err}");
    }

    #[test]
    fn crf_without_a_ceiling_is_rejected() {
        let s = spec().with_rate_control(RateControl::Crf {
            crf: 23,
            max_bitrate: 0,
            buffer_size: 0,
        });
        let err = s.validate().unwrap_err().to_string();
        assert!(err.contains("ceiling"), "{err}");
    }

    #[test]
    fn thread_count_is_capped_where_encoders_stop_scaling() {
        spec().with_threads(MAX_USEFUL_THREADS).validate().unwrap();
        assert!(
            spec()
                .with_threads(MAX_USEFUL_THREADS + 1)
                .validate()
                .is_err()
        );
    }

    #[test]
    fn ten_bit_h264_is_refused() {
        let mut s = spec();
        s.pixel_format = PixelFormat::Yuv420p10le;
        assert!(s.validate().is_err());

        s.codec = VideoCodecName::H265;
        s.validate().unwrap();
    }

    #[test]
    fn every_rung_of_a_ladder_shares_one_gop() {
        let rungs = [
            Resolution::new(1920, 1080),
            Resolution::new(1280, 720),
            Resolution::new(854, 480),
            Resolution::new(640, 360),
        ];
        let fr = Rational::new(30, 1);
        let gops: Vec<u32> = rungs
            .iter()
            .map(|r| EncodeSpec::new(VideoCodecName::H264, *r, fr).gop_frames)
            .collect();
        assert!(
            gops.windows(2).all(|w| w[0] == w[1]),
            "cut points must be identical across rungs: {gops:?}"
        );
    }

    #[test]
    fn codec_names_round_trip_through_the_cli_spellings() {
        use std::str::FromStr;
        assert_eq!(
            VideoCodecName::from_str("libx264").unwrap(),
            VideoCodecName::H264
        );
        assert_eq!(
            VideoCodecName::from_str("HEVC").unwrap(),
            VideoCodecName::H265
        );
        assert!(VideoCodecName::from_str("vp9").is_err());
    }
}
