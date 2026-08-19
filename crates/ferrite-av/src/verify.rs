//! Assert before publishing (pipeline step 8), and offline via `ferrite verify`.
//!
//! Why it exists: a dropped chunk produces a file that still plays. Nothing
//! crashes, no alarm fires, and you hear about it weeks later from a customer.
//! Only found by looking on purpose.
//!
//! Pure over probe output, so every rule is testable without a video file.

use crate::media::MediaInfo;
use serde::{Deserialize, Serialize};

/// One rendition, as probed.
#[derive(Debug, Clone)]
pub struct Rendition<'a> {
    /// What it is called: `1080p`, `720p`.
    pub name: &'a str,
    /// What probing it said.
    pub info: &'a MediaInfo,
}

/// Something wrong enough to hold publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "snake_case")]
pub enum Problem {
    /// The file has no video stream at all.
    NoVideo,
    /// Frame counts differ across rungs — the classic dropped chunk.
    FrameCount {
        /// What the other rungs agreed on.
        expected: u64,
        /// What this one has.
        actual: u64,
    },
    /// Keyframes are not at identical positions, so players cannot switch.
    KeyframeMismatch {
        /// Where the reference rung cuts.
        expected: Vec<u64>,
        /// Where this one cuts.
        actual: Vec<u64>,
    },
    /// Duration drifted by more than a frame.
    Duration {
        /// The reference rung's duration.
        expected_ms: u64,
        /// This one's.
        actual_ms: u64,
    },
    /// Probing reported a problem the encode should have removed.
    UnexpectedWarning {
        /// Which one.
        warning: String,
    },
}

/// A problem, and which rendition has it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Which rendition.
    pub rendition: String,
    /// What is wrong.
    pub problem: Problem,
}

/// What the check decided.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    /// Everything wrong. Empty means publishable.
    pub findings: Vec<Finding>,
    /// How many renditions were looked at.
    pub checked: usize,
}

impl Verdict {
    /// Whether this may be published.
    pub fn is_ok(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Warnings that are fine in a source but mean the encode did not do its job.
const MUST_NOT_SURVIVE: [crate::media::Warning; 3] = [
    crate::media::Warning::VariableFrameRate,
    crate::media::Warning::RotationMetadata,
    crate::media::Warning::NonMonotonicTimestamps,
];

/// Check a set of renditions against each other.
///
/// The first rendition is the reference; the rules are about agreement, since
/// a ladder that agrees with itself is what lets a player switch rungs.
pub fn verify(renditions: &[Rendition<'_>]) -> Verdict {
    let mut findings = Vec::new();
    let mut push = |name: &str, problem| {
        findings.push(Finding {
            rendition: name.to_string(),
            problem,
        });
    };

    for r in renditions {
        if r.info.primary_video().is_none() {
            push(r.name, Problem::NoVideo);
        }
        for w in &r.info.warnings {
            if MUST_NOT_SURVIVE.contains(w) {
                push(
                    r.name,
                    Problem::UnexpectedWarning {
                        warning: w.to_string(),
                    },
                );
            }
        }
    }

    let Some(reference) = renditions.first() else {
        return Verdict {
            findings,
            checked: 0,
        };
    };

    // One frame of slack, in milliseconds, from the reference's own rate.
    let frame_ms = reference
        .info
        .primary_video()
        .map(|v| {
            let fps = v.frame_rate.as_f64();
            if fps > 0.0 {
                (1000.0 / fps).ceil() as u64
            } else {
                0
            }
        })
        .unwrap_or(0);

    let expected_frames = reference.info.primary_video().and_then(|v| v.frame_count);

    for r in &renditions[1..] {
        if let (Some(want), Some(got)) = (
            expected_frames,
            r.info.primary_video().and_then(|v| v.frame_count),
        ) && want != got
        {
            push(
                r.name,
                Problem::FrameCount {
                    expected: want,
                    actual: got,
                },
            );
        }

        if r.info.keyframes_ms != reference.info.keyframes_ms {
            push(
                r.name,
                Problem::KeyframeMismatch {
                    expected: reference.info.keyframes_ms.clone(),
                    actual: r.info.keyframes_ms.clone(),
                },
            );
        }

        if r.info.duration_ms.abs_diff(reference.info.duration_ms) > frame_ms {
            push(
                r.name,
                Problem::Duration {
                    expected_ms: reference.info.duration_ms,
                    actual_ms: r.info.duration_ms,
                },
            );
        }
    }

    Verdict {
        findings,
        checked: renditions.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::{ColorInfo, Rational, VideoStream, Warning};

    fn info(frames: u64, duration_ms: u64, keyframes: Vec<u64>) -> MediaInfo {
        MediaInfo {
            format: "mp4".into(),
            duration_ms,
            size_bytes: 1000,
            bit_rate: None,
            video: vec![VideoStream {
                index: 0,
                codec: "h264".into(),
                width: 1280,
                height: 720,
                frame_rate: Rational::new(30, 1),
                pixel_format: "yuv420p".into(),
                rotation_degrees: 0,
                sample_aspect_ratio: Rational::new(1, 1),
                color: ColorInfo::default(),
                bit_rate: None,
                frame_count: Some(frames),
            }],
            audio: vec![],
            keyframes_ms: keyframes,
            warnings: vec![],
        }
    }

    fn keys() -> Vec<u64> {
        vec![0, 2000, 4000, 6000]
    }

    fn check<'a>(pairs: &'a [(&'a str, MediaInfo)]) -> Verdict {
        let rs: Vec<Rendition<'_>> = pairs
            .iter()
            .map(|(n, i)| Rendition { name: n, info: i })
            .collect();
        verify(&rs)
    }

    #[test]
    fn a_ladder_that_agrees_with_itself_passes() {
        let v = check(&[
            ("1080p", info(240, 8000, keys())),
            ("720p", info(240, 8000, keys())),
            ("360p", info(240, 8000, keys())),
        ]);
        assert!(v.is_ok(), "{:?}", v.findings);
        assert_eq!(v.checked, 3);
    }

    #[test]
    fn a_dropped_chunk_is_caught_by_the_frame_count() {
        // The failure this whole step exists for: the file still plays.
        let v = check(&[
            ("1080p", info(240, 8000, keys())),
            ("720p", info(210, 8000, keys())),
        ]);
        assert_eq!(
            v.findings,
            [Finding {
                rendition: "720p".into(),
                problem: Problem::FrameCount {
                    expected: 240,
                    actual: 210
                },
            }]
        );
    }

    #[test]
    fn keyframes_that_drift_are_caught() {
        let v = check(&[
            ("1080p", info(240, 8000, keys())),
            ("720p", info(240, 8000, vec![0, 2000, 4500, 6000])),
        ]);
        assert!(matches!(
            v.findings[0].problem,
            Problem::KeyframeMismatch { .. }
        ));
        assert_eq!(v.findings[0].rendition, "720p");
    }

    #[test]
    fn a_duration_within_one_frame_is_fine_but_beyond_it_is_not() {
        // 30fps means a frame is 34ms once rounded up.
        let ok = check(&[
            ("a", info(240, 8000, keys())),
            ("b", info(240, 8030, keys())),
        ]);
        assert!(ok.is_ok(), "{:?}", ok.findings);

        let bad = check(&[
            ("a", info(240, 8000, keys())),
            ("b", info(240, 8500, keys())),
        ]);
        assert!(matches!(bad.findings[0].problem, Problem::Duration { .. }));
    }

    #[test]
    fn a_rendition_with_no_video_is_caught() {
        let mut empty = info(240, 8000, keys());
        empty.video.clear();
        let v = check(&[("1080p", info(240, 8000, keys())), ("720p", empty)]);
        assert!(v.findings.iter().any(|f| f.problem == Problem::NoVideo));
    }

    #[test]
    fn a_warning_the_encode_should_have_removed_is_caught() {
        // Rotation surviving into a rendition means it was never baked in.
        let mut rotated = info(240, 8000, keys());
        rotated.warnings.push(Warning::RotationMetadata);
        let v = check(&[("1080p", rotated)]);
        assert_eq!(
            v.findings[0].problem,
            Problem::UnexpectedWarning {
                warning: "rotation_metadata".into()
            }
        );
    }

    #[test]
    fn a_warning_that_is_merely_informational_is_not_a_finding() {
        let mut silent = info(240, 8000, keys());
        silent.warnings.push(Warning::NoAudio);
        assert!(check(&[("1080p", silent)]).is_ok());
    }

    #[test]
    fn every_bad_rendition_is_named_not_just_the_first() {
        let v = check(&[
            ("1080p", info(240, 8000, keys())),
            ("720p", info(200, 8000, keys())),
            ("360p", info(180, 8000, keys())),
        ]);
        let named: Vec<&str> = v.findings.iter().map(|f| f.rendition.as_str()).collect();
        assert!(
            named.contains(&"720p") && named.contains(&"360p"),
            "{named:?}"
        );
    }

    #[test]
    fn nothing_to_check_is_not_a_failure() {
        let v = verify(&[]);
        assert!(v.is_ok());
        assert_eq!(v.checked, 0);
    }

    #[test]
    fn a_single_rendition_still_gets_its_own_checks() {
        let mut vfr = info(240, 8000, keys());
        vfr.warnings.push(Warning::VariableFrameRate);
        assert!(!check(&[("only", vfr)]).is_ok());
    }
}
