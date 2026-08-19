//! What a probe tells us about a file — plain data, no FFmpeg types.

use serde::{Deserialize, Serialize};

/// What pipeline step 2 extracts from a source file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    /// Container format short name, e.g. `mov,mp4,m4a,3gp,3g2,mj2`.
    pub format: String,
    /// Duration in milliseconds, as reported by the container.
    pub duration_ms: u64,
    /// Total file size in bytes.
    pub size_bytes: u64,
    /// Container-level bitrate in bits per second, when known.
    pub bit_rate: Option<u64>,
    /// Video streams, in container order. Usually zero or one.
    pub video: Vec<VideoStream>,
    /// Audio streams, in container order.
    pub audio: Vec<AudioStream>,
    /// Keyframe presentation times in ms. The split planner cuts only at these.
    pub keyframes_ms: Vec<u64>,
    /// Things that will bite us later if we do not normalize them.
    pub warnings: Vec<Warning>,
}

impl MediaInfo {
    /// The stream we build the ladder from: the first video stream.
    pub fn primary_video(&self) -> Option<&VideoStream> {
        self.video.first()
    }

    /// Mezzanine if the file has problems a normalising pass fixes, OR more
    /// than one output comes from it. Asset mode always passes `outputs > 1`,
    /// so this only skips work in job mode.
    pub fn needs_mezzanine(&self, outputs: usize) -> bool {
        outputs > 1 || self.warnings.iter().any(|w| w.needs_normalising())
    }
}

/// One video stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoStream {
    /// Container stream index.
    pub index: usize,
    /// Codec short name, e.g. `h264`.
    pub codec: String,
    /// Coded width in pixels, before rotation is applied.
    pub width: u32,
    /// Coded height in pixels, before rotation is applied.
    pub height: u32,
    /// Average frame rate as an exact rational, e.g. `30000/1001`.
    pub frame_rate: Rational,
    /// Pixel format short name, e.g. `yuv420p`.
    pub pixel_format: String,
    /// Rotation in degrees from display matrix side data.
    pub rotation_degrees: i32,
    /// Pixel shape. Anamorphic sources code a 16:9 picture into a 4:3 grid, so
    /// the coded size is not what a viewer sees.
    pub sample_aspect_ratio: Rational,
    /// Colour description, when the container states one.
    pub color: ColorInfo,
    /// Stream bitrate in bits per second, when known.
    pub bit_rate: Option<u64>,
    /// Number of frames, when the container states it.
    pub frame_count: Option<u64>,
}

impl VideoStream {
    /// Width and height as a viewer sees them: pixel shape applied, then
    /// rotation. Laddering from the coded size squashes anamorphic sources.
    pub fn display_size(&self) -> (u32, u32) {
        let (num, den) = (self.sample_aspect_ratio.num, self.sample_aspect_ratio.den);
        let width = if num > 0 && den > 0 && num != den {
            ((u64::from(self.width) * num as u64 + den as u64 / 2) / den as u64) as u32
        } else {
            self.width
        };

        if self.rotation_degrees.rem_euclid(180) == 90 {
            (self.height, width)
        } else {
            (width, self.height)
        }
    }
}

/// One audio stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioStream {
    /// Container stream index.
    pub index: usize,
    /// Codec short name, e.g. `aac`.
    pub codec: String,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u16,
    /// Channel layout description, e.g. `stereo`.
    pub channel_layout: String,
    /// Stream bitrate in bits per second, when known.
    pub bit_rate: Option<u64>,
    /// BCP-47-ish language tag from container metadata.
    pub language: Option<String>,
}

/// Colour description, carried through so an HDR source is recognisable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorInfo {
    /// Colour primaries, e.g. `bt709`.
    pub primaries: Option<String>,
    /// Transfer characteristics, e.g. `smpte2084`.
    pub transfer: Option<String>,
    /// Matrix coefficients, e.g. `bt709`.
    pub matrix: Option<String>,
    /// Colour range, e.g. `tv` or `pc`.
    pub range: Option<String>,
}

impl ColorInfo {
    /// Whether this looks like HDR — PQ or HLG transfer.
    pub fn is_hdr(&self) -> bool {
        matches!(self.transfer.as_deref(), Some("smpte2084" | "arib-std-b67"))
    }
}

/// An exact rational, so `30000/1001` never becomes `29.97`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rational {
    /// Numerator.
    pub num: i32,
    /// Denominator.
    pub den: i32,
}

impl Rational {
    /// A new rational. Denominator of zero means "unknown".
    pub const fn new(num: i32, den: i32) -> Self {
        Self { num, den }
    }

    /// Approximate value. Never for maths that ends up in a timestamp.
    pub fn as_f64(self) -> f64 {
        if self.den == 0 {
            0.0
        } else {
            f64::from(self.num) / f64::from(self.den)
        }
    }
}

impl std::fmt::Display for Rational {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

/// A problem in the source that the mezzanine pass exists to fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Warning {
    /// Frame durations vary; boundaries can't be predicted from a frame rate.
    VariableFrameRate,
    /// Rotated by a display matrix. The mezzanine bakes it into pixels.
    RotationMetadata,
    /// Presentation timestamps go backwards or jump.
    NonMonotonicTimestamps,
    /// The first timestamp is not zero.
    StartTimeOffset,
    /// No keyframe index — seeking is a scan, so chunking is unreliable.
    NoKeyframeIndex,
    /// Keyframes are far enough apart that ~10s chunks cannot land on them.
    SparseKeyframes,
    /// Container reports no duration.
    UnknownDuration,
    /// More than one video stream; we take the first and say so.
    MultipleVideoStreams,
    /// No audio at all. Not fatal, but the ladder and manifests differ.
    NoAudio,
    /// Interlaced content, which needs deinterlacing before encode.
    Interlaced,
    /// Anamorphic pixels — sample aspect ratio is not 1:1.
    NonSquarePixels,
    /// HDR transfer function; needs passthrough plus a tonemapped rung.
    HdrTransfer,
}

impl Warning {
    /// Whether a normalising pass fixes this.
    ///
    /// Silence is not a defect, and neither is an unknown duration — a
    /// mezzanine changes neither, so neither should cost one.
    pub fn needs_normalising(self) -> bool {
        match self {
            Self::VariableFrameRate
            | Self::RotationMetadata
            | Self::NonMonotonicTimestamps
            | Self::StartTimeOffset
            | Self::Interlaced
            | Self::NonSquarePixels => true,

            // Real problems, but ones a mezzanine does not solve.
            Self::NoAudio
            | Self::UnknownDuration
            | Self::MultipleVideoStreams
            | Self::HdrTransfer
            // Both only matter for chunking, which is asset mode's problem.
            | Self::NoKeyframeIndex
            | Self::SparseKeyframes => false,
        }
    }

    /// The stable string used in JSON output and in `sources.warnings`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VariableFrameRate => "variable_frame_rate",
            Self::RotationMetadata => "rotation_metadata",
            Self::NonMonotonicTimestamps => "non_monotonic_timestamps",
            Self::StartTimeOffset => "start_time_offset",
            Self::NoKeyframeIndex => "no_keyframe_index",
            Self::SparseKeyframes => "sparse_keyframes",
            Self::UnknownDuration => "unknown_duration",
            Self::MultipleVideoStreams => "multiple_video_streams",
            Self::NoAudio => "no_audio",
            Self::Interlaced => "interlaced",
            Self::NonSquarePixels => "non_square_pixels",
            Self::HdrTransfer => "hdr_transfer",
        }
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(rotation: i32) -> VideoStream {
        VideoStream {
            index: 0,
            codec: "h264".into(),
            width: 1920,
            height: 1080,
            frame_rate: Rational::new(30000, 1001),
            pixel_format: "yuv420p".into(),
            rotation_degrees: rotation,
            sample_aspect_ratio: Rational::new(1, 1),
            color: ColorInfo::default(),
            bit_rate: Some(8_000_000),
            frame_count: None,
        }
    }

    fn info(warnings: Vec<Warning>) -> MediaInfo {
        MediaInfo {
            format: "mov,mp4,m4a,3gp,3g2,mj2".into(),
            duration_ms: 612_480,
            size_bytes: 1_000_000,
            bit_rate: Some(8_000_000),
            video: vec![stream(0)],
            audio: vec![],
            keyframes_ms: vec![0, 10_000],
            warnings,
        }
    }

    #[test]
    fn anamorphic_pixels_widen_the_display_size() {
        // 720x576 carrying a 16:9 picture displays as 1024x576.
        let mut wide = stream(0);
        wide.width = 720;
        wide.height = 576;
        wide.sample_aspect_ratio = Rational::new(64, 45);
        assert_eq!(wide.display_size(), (1024, 576));
    }

    #[test]
    fn rotation_swaps_display_size() {
        assert_eq!(stream(0).display_size(), (1920, 1080));
        assert_eq!(stream(90).display_size(), (1080, 1920));
        assert_eq!(stream(-90).display_size(), (1080, 1920));
        assert_eq!(stream(180).display_size(), (1920, 1080));
        assert_eq!(stream(270).display_size(), (1080, 1920));
    }

    #[test]
    fn mezzanine_is_skipped_only_for_a_clean_single_output() {
        assert!(!info(vec![]).needs_mezzanine(1));
        assert!(info(vec![]).needs_mezzanine(4));
        assert!(info(vec![Warning::VariableFrameRate]).needs_mezzanine(1));
    }

    #[test]
    fn a_silent_video_does_not_pay_for_a_mezzanine() {
        // A mezzanine fixes timing and geometry. It does not add audio, and
        // charging ~23% of the job for that would be for nothing.
        for harmless in [
            Warning::NoAudio,
            Warning::UnknownDuration,
            Warning::MultipleVideoStreams,
            Warning::HdrTransfer,
            Warning::NoKeyframeIndex,
            Warning::SparseKeyframes,
        ] {
            assert!(
                !info(vec![harmless]).needs_mezzanine(1),
                "{harmless} demanded a mezzanine it does not need"
            );
        }
    }

    #[test]
    fn everything_a_normalising_pass_fixes_still_demands_one() {
        for real in [
            Warning::VariableFrameRate,
            Warning::RotationMetadata,
            Warning::NonMonotonicTimestamps,
            Warning::StartTimeOffset,
            Warning::Interlaced,
            Warning::NonSquarePixels,
        ] {
            assert!(
                info(vec![real]).needs_mezzanine(1),
                "{real} was let through"
            );
        }
    }

    #[test]
    fn frame_rate_stays_exact() {
        let r = Rational::new(30000, 1001);
        assert_eq!(r.to_string(), "30000/1001");
        assert_eq!(
            serde_json::to_string(&stream(0).frame_rate).unwrap(),
            r#"{"num":30000,"den":1001}"#
        );
    }

    #[test]
    fn warnings_serialize_as_the_documented_strings() {
        let json = serde_json::to_string(&Warning::VariableFrameRate).unwrap();
        assert_eq!(json, r#""variable_frame_rate""#);
        assert_eq!(Warning::RotationMetadata.as_str(), "rotation_metadata");
    }

    #[test]
    fn hdr_is_detected_from_the_transfer_function() {
        let mut c = ColorInfo::default();
        assert!(!c.is_hdr());
        c.transfer = Some("smpte2084".into());
        assert!(c.is_hdr());
        c.transfer = Some("bt709".into());
        assert!(!c.is_hdr());
    }
}
