//! Which rungs to encode (pipeline step 4).
//!
//! Pure: rungs in, [`EncodeSpec`]s out. No FFmpeg, so the rules are testable
//! without a video file.

use crate::encoder::{EncodeSpec, Preset, RateControl, Resolution, VideoCodecName};
use crate::media::VideoStream;
use serde::{Deserialize, Serialize};

/// One step of the ladder, defined by height. Width follows the source's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rung {
    /// What the rendition is called: `1080p`, `720p`.
    pub name: &'static str,
    /// Output height in pixels.
    pub height: u32,
    /// Bitrate ceiling in bits per second.
    pub max_bitrate: u32,
    /// Quality target.
    pub crf: u8,
}

/// The rungs a plan asks for, highest first.
pub const STANDARD: [Rung; 4] = [
    Rung {
        name: "1080p",
        height: 1080,
        max_bitrate: 5_000_000,
        crf: 23,
    },
    Rung {
        name: "720p",
        height: 720,
        max_bitrate: 2_800_000,
        crf: 23,
    },
    Rung {
        name: "480p",
        height: 480,
        max_bitrate: 1_400_000,
        crf: 24,
    },
    Rung {
        name: "360p",
        height: 360,
        max_bitrate: 800_000,
        crf: 25,
    },
];

/// A rung paired with the spec that produces it.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// The rendition name, unique per asset.
    pub name: &'static str,
    /// What to hand the encoder.
    pub spec: EncodeSpec,
}

/// Choose rungs for `source`.
///
/// Two rules: never make a rung bigger than the
/// source, and drop rungs above the source bitrate. The smallest rung always
/// survives, so a tiny or low-bitrate source still produces something.
pub fn plan(source: &VideoStream, rungs: &[Rung], preset: Preset) -> Vec<Step> {
    let (src_w, src_h) = source.display_size();
    if src_w == 0 || src_h == 0 {
        return Vec::new();
    }

    let mut steps: Vec<Step> = rungs
        .iter()
        .filter(|r| r.height <= src_h)
        .filter(|r| {
            source
                .bit_rate
                .is_none_or(|src| u64::from(r.max_bitrate) < src)
        })
        .map(|r| step(source, r, preset, src_w, src_h))
        .collect();

    // Both filters can empty the list — a 240p source, or one already smaller
    // than our lowest ceiling. Publishing nothing is never the right answer.
    if steps.is_empty() {
        let smallest = rungs.iter().min_by_key(|r| r.height);
        if let Some(r) = smallest {
            let capped = Rung {
                height: r.height.min(even(src_h)),
                ..*r
            };
            steps.push(step(source, &capped, preset, src_w, src_h));
        }
    }
    steps
}

/// The one rung the fast path encodes so the asset becomes playable.
///
/// The middle of what was planned: high enough to watch, small enough to finish.
pub fn fast_path(steps: &[Step]) -> Option<&Step> {
    steps.get(steps.len() / 2)
}

fn step(source: &VideoStream, rung: &Rung, preset: Preset, src_w: u32, src_h: u32) -> Step {
    // Height rounds down so a rung never exceeds the source; width rounds to
    // nearest, because truncating it twice visibly drifts the aspect ratio.
    let height = even(rung.height.min(src_h));
    let exact = u64::from(height) * u64::from(src_w);
    let width = nearest_even((exact + u64::from(src_h) / 2) / u64::from(src_h));

    Step {
        name: rung.name,
        spec: EncodeSpec::new(
            VideoCodecName::H264,
            Resolution::new(width.max(2), height.max(2)),
            source.frame_rate,
        )
        .with_preset(preset)
        .with_rate_control(RateControl::crf(rung.crf, rung.max_bitrate)),
    }
}

/// 4:2:0 chroma needs both dimensions even. Rounds down.
fn even(n: u32) -> u32 {
    n & !1
}

/// Nearest even, rounding up on odd.
fn nearest_even(n: u64) -> u32 {
    ((n + 1) & !1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{ColorInfo, Rational};

    fn source(width: u32, height: u32, bit_rate: Option<u64>) -> VideoStream {
        VideoStream {
            index: 0,
            codec: "h264".into(),
            width,
            height,
            frame_rate: Rational::new(30, 1),
            pixel_format: "yuv420p".into(),
            rotation_degrees: 0,
            sample_aspect_ratio: Rational::new(1, 1),
            color: ColorInfo::default(),
            bit_rate,
            frame_count: None,
        }
    }

    fn names(steps: &[Step]) -> Vec<&str> {
        steps.iter().map(|s| s.name).collect()
    }

    #[test]
    fn a_1080p_source_gets_the_whole_ladder() {
        let steps = plan(&source(1920, 1080, None), &STANDARD, Preset::Medium);
        assert_eq!(names(&steps), ["1080p", "720p", "480p", "360p"]);
        assert_eq!(steps[0].spec.resolution, Resolution::new(1920, 1080));
        assert_eq!(steps[1].spec.resolution, Resolution::new(1280, 720));
    }

    #[test]
    fn no_rung_is_ever_bigger_than_the_source() {
        let steps = plan(&source(1280, 720, None), &STANDARD, Preset::Medium);
        assert_eq!(names(&steps), ["720p", "480p", "360p"]);
        assert!(steps.iter().all(|s| s.spec.resolution.height <= 720));
    }

    #[test]
    fn rungs_above_the_source_bitrate_are_dropped() {
        // A 1080p source at 1.5 Mbps: the 5 Mbps and 2.8 Mbps ceilings are
        // above it, so encoding them would spend bytes inventing detail.
        let steps = plan(
            &source(1920, 1080, Some(1_500_000)),
            &STANDARD,
            Preset::Medium,
        );
        assert_eq!(names(&steps), ["480p", "360p"]);
    }

    #[test]
    fn width_follows_the_sources_aspect_ratio_not_the_rungs() {
        // 4:3 source. A 720p rung must be 960x720, never 1280x720.
        let steps = plan(&source(1440, 1080, None), &STANDARD, Preset::Medium);
        assert_eq!(steps[1].spec.resolution, Resolution::new(960, 720));
    }

    #[test]
    fn a_rotated_source_is_laddered_on_its_display_size() {
        let mut portrait = source(1920, 1080, None);
        portrait.rotation_degrees = 90;
        let steps = plan(&portrait, &STANDARD, Preset::Medium);
        // Displayed 1080x1920, so 1080p means 1080 tall and 608 wide.
        assert_eq!(steps[0].spec.resolution, Resolution::new(608, 1080));
    }

    #[test]
    fn every_rung_is_encodable() {
        for (w, h) in [
            (1920, 1080),
            (1440, 1080),
            (640, 360),
            (1080, 1920),
            (854, 480),
        ] {
            for step in plan(&source(w, h, None), &STANDARD, Preset::Medium) {
                step.spec
                    .validate()
                    .unwrap_or_else(|e| panic!("{w}x{h} {}: {e}", step.name));
            }
        }
    }

    #[test]
    fn a_tiny_source_still_produces_one_rung() {
        let steps = plan(&source(320, 240, None), &STANDARD, Preset::Medium);
        assert_eq!(steps.len(), 1, "publishing nothing is not an option");
        assert!(steps[0].spec.resolution.height <= 240);
        steps[0].spec.validate().unwrap();
    }

    #[test]
    fn a_source_below_every_ceiling_still_produces_one_rung() {
        let steps = plan(&source(640, 360, Some(200_000)), &STANDARD, Preset::Medium);
        assert_eq!(steps.len(), 1);
        steps[0].spec.validate().unwrap();
    }

    #[test]
    fn odd_dimensions_are_rounded_down_to_even() {
        let steps = plan(&source(1919, 1081, None), &STANDARD, Preset::Medium);
        for s in &steps {
            assert_eq!(s.spec.resolution.width % 2, 0, "{}", s.name);
            assert_eq!(s.spec.resolution.height % 2, 0, "{}", s.name);
        }
    }

    #[test]
    fn every_rung_shares_one_gop_so_players_can_switch() {
        let steps = plan(&source(1920, 1080, None), &STANDARD, Preset::Medium);
        let gops: Vec<u32> = steps.iter().map(|s| s.spec.gop_frames).collect();
        assert!(gops.windows(2).all(|w| w[0] == w[1]), "{gops:?}");
    }

    #[test]
    fn the_fast_path_takes_a_middle_rung_not_the_biggest() {
        let steps = plan(&source(1920, 1080, None), &STANDARD, Preset::Veryfast);
        let fast = fast_path(&steps).unwrap();
        assert_eq!(fast.name, "480p");
        assert!(fast_path(&[]).is_none());
    }

    #[test]
    fn the_aspect_ratio_survives_every_rung() {
        for (w, h) in [(1920, 1080), (1440, 1080), (1080, 1920), (2048, 858)] {
            let src = f64::from(w) / f64::from(h);
            for step in plan(&source(w, h, None), &STANDARD, Preset::Medium) {
                let got =
                    f64::from(step.spec.resolution.width) / f64::from(step.spec.resolution.height);
                assert!(
                    (got - src).abs() / src < 0.01,
                    "{w}x{h} {} came out {} ({got:.4} vs {src:.4})",
                    step.name,
                    step.spec.resolution
                );
            }
        }
    }

    #[test]
    fn a_zero_sized_source_plans_nothing_rather_than_panicking() {
        assert!(plan(&source(0, 0, None), &STANDARD, Preset::Medium).is_empty());
    }
}
