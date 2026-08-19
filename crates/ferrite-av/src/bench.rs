//! The corpus report, and the diff that gates a merge.
//!
//! `ferrite bench` runs every awkward file through the pipeline and measures the
//! results. `ferrite compare` diffs two reports — that diff is the CI gate.
//!
//! Reports are kept per release, so "when did banding get worse?" is answerable
//! by looking rather than guessing.

use crate::quality::Metrics;
use serde::{Deserialize, Serialize};

/// Which way a metric moves when things improve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// VMAF, PSNR, SSIM, CIEDE2000 as libvmaf reports it.
    HigherIsBetter,
    /// CAMBI: it counts banding.
    LowerIsBetter,
}

/// One rung's numbers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scores {
    /// Overall perceived quality.
    pub vmaf: f64,
    /// Worst frame's VMAF. A good mean hides a terrible scene.
    pub vmaf_min: f64,
    /// Luma signal error.
    pub psnr_y: f64,
    /// Structural similarity.
    pub ssim: Option<f64>,
    /// Multi-scale structural similarity.
    pub ms_ssim: Option<f64>,
    /// Colour difference.
    pub ciede2000: Option<f64>,
    /// Banding.
    pub cambi: Option<f64>,
}

impl Scores {
    /// Read the pooled means out of a measurement.
    pub fn from_metrics(m: &Metrics) -> Self {
        Self {
            vmaf: m.vmaf.mean,
            vmaf_min: m.vmaf.min,
            psnr_y: m.psnr_y.mean,
            ssim: m.ssim.map(|p| p.mean),
            ms_ssim: m.ms_ssim.map(|p| p.mean),
            ciede2000: m.ciede2000.map(|p| p.mean),
            cambi: m.cambi.map(|p| p.mean),
        }
    }

    /// Every metric by name, with the direction that counts as better.
    pub fn named(&self) -> Vec<(&'static str, f64, Direction)> {
        let mut out = vec![
            ("vmaf", self.vmaf, Direction::HigherIsBetter),
            ("vmaf_min", self.vmaf_min, Direction::HigherIsBetter),
            ("psnr_y", self.psnr_y, Direction::HigherIsBetter),
        ];
        for (name, value, direction) in [
            ("ssim", self.ssim, Direction::HigherIsBetter),
            ("ms_ssim", self.ms_ssim, Direction::HigherIsBetter),
            ("ciede2000", self.ciede2000, Direction::HigherIsBetter),
            ("cambi", self.cambi, Direction::LowerIsBetter),
        ] {
            if let Some(value) = value {
                out.push((name, value, direction));
            }
        }
        out
    }
}

/// One rendition in the report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rung {
    /// Rendition name.
    pub name: String,
    /// Output size.
    pub width: u32,
    /// Output size.
    pub height: u32,
    /// Frames encoded.
    pub frames: u64,
    /// Compressed bytes.
    pub bytes: u64,
    /// Quality against the reference, when it was measured.
    pub scores: Option<Scores>,
}

/// One corpus file's results.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// File name, without the directory, so reports diff across machines.
    pub file: String,
    /// Source duration.
    pub duration_ms: u64,
    /// What probing complained about. This is a corpus of awkward files.
    pub warnings: Vec<String>,
    /// Per rendition.
    pub rungs: Vec<Rung>,
    /// Structural findings. Any entry here is a failure, not a regression.
    pub findings: Vec<String>,
    /// Wall-clock seconds to encode the ladder.
    pub encode_seconds: f64,
}

/// A whole run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Seconds since the epoch. Reports are kept per release.
    pub created_unix: u64,
    /// What produced these numbers. Encoder settings, presets and FFmpeg
    /// version all move them, so the report says which was used.
    pub ffmpeg: String,
    /// Encoder preset.
    pub preset: String,
    /// Frame subsampling used for quality. Above 1 is not a release number.
    pub subsample: u32,
    /// One per corpus file.
    pub entries: Vec<Entry>,
}

impl Report {
    /// A report with nothing in it yet.
    pub fn new(ffmpeg: String, preset: String, subsample: u32) -> Self {
        Self {
            created_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            ffmpeg,
            preset,
            subsample,
            entries: Vec::new(),
        }
    }

    /// Every structural finding across the run. Non-empty means broken output,
    /// which is a different thing from a quality regression.
    pub fn findings(&self) -> Vec<String> {
        self.entries
            .iter()
            .flat_map(|e| e.findings.iter().map(move |f| format!("{}: {f}", e.file)))
            .collect()
    }
}

/// How much movement is noise rather than a regression.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tolerance {
    /// Chunked versus single-pass must stay within this.
    pub vmaf: f64,
    /// PSNR in dB.
    pub psnr: f64,
    /// SSIM and MS-SSIM are 0..1.
    pub ssim: f64,
    /// CIEDE2000 as libvmaf reports it.
    pub ciede2000: f64,
    /// CAMBI counts banding, so any rise matters.
    pub cambi: f64,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self {
            vmaf: crate::quality::GATE_DELTA_VMAF,
            psnr: 0.2,
            ssim: 0.001,
            ciede2000: 0.5,
            cambi: 0.05,
        }
    }
}

impl Tolerance {
    fn for_metric(&self, metric: &str) -> f64 {
        match metric {
            "vmaf" | "vmaf_min" => self.vmaf,
            "psnr_y" => self.psnr,
            "ssim" | "ms_ssim" => self.ssim,
            "ciede2000" => self.ciede2000,
            "cambi" => self.cambi,
            _ => f64::INFINITY,
        }
    }
}

/// One metric that moved the wrong way by more than its tolerance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Regression {
    /// Which corpus file.
    pub file: String,
    /// Which rendition.
    pub rung: String,
    /// Which metric.
    pub metric: String,
    /// The baseline value.
    pub before: f64,
    /// The new value.
    pub after: f64,
    /// Signed movement, in the metric's own units.
    pub delta: f64,
}

/// What changed between two runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    /// Metrics that moved the wrong way beyond tolerance.
    pub regressions: Vec<Regression>,
    /// Rungs present in the baseline and absent now. Losing a rendition is not
    /// a quality question, so it is listed separately.
    pub missing: Vec<String>,
    /// Rungs that are new since the baseline.
    pub added: Vec<String>,
}

impl Diff {
    /// Whether this diff may be merged.
    pub fn is_clean(&self) -> bool {
        self.regressions.is_empty() && self.missing.is_empty()
    }
}

/// Diff two reports. Pure, so the gate is testable without running a corpus.
pub fn compare(before: &Report, after: &Report, tolerance: &Tolerance) -> Diff {
    let mut regressions = Vec::new();
    let mut missing = Vec::new();
    let mut added = Vec::new();

    let key = |file: &str, rung: &str| format!("{file}/{rung}");

    let mut seen = std::collections::HashSet::new();
    for old_entry in &before.entries {
        let new_entry = after.entries.iter().find(|e| e.file == old_entry.file);
        for old_rung in &old_entry.rungs {
            let id = key(&old_entry.file, &old_rung.name);
            let new_rung = new_entry.and_then(|e| e.rungs.iter().find(|r| r.name == old_rung.name));

            let Some(new_rung) = new_rung else {
                missing.push(id);
                continue;
            };
            seen.insert(id);

            let (Some(old), Some(new)) = (old_rung.scores, new_rung.scores) else {
                continue;
            };
            let new_by_name: std::collections::HashMap<_, _> =
                new.named().into_iter().map(|(n, v, _)| (n, v)).collect();

            for (metric, before_value, direction) in old.named() {
                let Some(&after_value) = new_by_name.get(metric) else {
                    continue;
                };
                let delta = after_value - before_value;
                let worse = match direction {
                    Direction::HigherIsBetter => -delta,
                    Direction::LowerIsBetter => delta,
                };

                if worse > tolerance.for_metric(metric) {
                    regressions.push(Regression {
                        file: old_entry.file.clone(),
                        rung: old_rung.name.clone(),
                        metric: metric.to_string(),
                        before: before_value,
                        after: after_value,
                        delta,
                    });
                }
            }
        }
    }

    for entry in &after.entries {
        for rung in &entry.rungs {
            let id = key(&entry.file, &rung.name);
            if !seen.contains(&id) && !missing.contains(&id) {
                added.push(id);
            }
        }
    }

    Diff {
        regressions,
        missing,
        added,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scores(vmaf: f64, cambi: f64) -> Scores {
        Scores {
            vmaf,
            vmaf_min: vmaf - 2.0,
            psnr_y: 40.0,
            ssim: Some(0.99),
            ms_ssim: Some(0.98),
            ciede2000: Some(50.0),
            cambi: Some(cambi),
        }
    }

    fn report(rungs: &[(&str, Scores)]) -> Report {
        let mut r = Report::new("7.1".into(), "medium".into(), 1);
        r.entries.push(Entry {
            file: "rotated.mp4".into(),
            duration_ms: 10_000,
            warnings: vec!["rotation_metadata".into()],
            rungs: rungs
                .iter()
                .map(|(name, s)| Rung {
                    name: (*name).to_string(),
                    width: 1920,
                    height: 1080,
                    frames: 300,
                    bytes: 1000,
                    scores: Some(*s),
                })
                .collect(),
            findings: Vec::new(),
            encode_seconds: 1.0,
        });
        r
    }

    #[test]
    fn an_identical_run_is_clean() {
        let a = report(&[("1080p", scores(95.0, 0.1))]);
        let diff = compare(&a, &a, &Tolerance::default());
        assert!(diff.is_clean(), "{diff:?}");
    }

    #[test]
    fn a_vmaf_drop_beyond_half_a_point_is_a_regression() {
        // The documented chunked-versus-single-pass gate.
        let before = report(&[("1080p", scores(95.0, 0.1))]);
        let after = report(&[("1080p", scores(94.4, 0.1))]);

        let diff = compare(&before, &after, &Tolerance::default());
        assert!(!diff.is_clean());
        let hit = diff
            .regressions
            .iter()
            .find(|r| r.metric == "vmaf")
            .expect("vmaf regression");
        assert!((hit.delta - -0.6).abs() < 1e-9, "{hit:?}");
    }

    #[test]
    fn a_vmaf_drop_within_tolerance_is_noise() {
        let before = report(&[("1080p", scores(95.0, 0.1))]);
        let after = report(&[("1080p", scores(94.6, 0.1))]);
        assert!(compare(&before, &after, &Tolerance::default()).is_clean());
    }

    #[test]
    fn an_improvement_is_never_a_regression() {
        let before = report(&[("1080p", scores(90.0, 0.5))]);
        let after = report(&[("1080p", scores(97.0, 0.1))]);
        assert!(compare(&before, &after, &Tolerance::default()).is_clean());
    }

    #[test]
    fn banding_getting_worse_is_caught_even_when_vmaf_holds() {
        // The whole reason CAMBI is measured: VMAF barely sees banding.
        let before = report(&[("1080p", scores(95.0, 0.10))]);
        let after = report(&[("1080p", scores(95.0, 0.40))]);

        let diff = compare(&before, &after, &Tolerance::default());
        assert!(!diff.is_clean(), "banding regression slipped through");
        assert_eq!(diff.regressions.len(), 1);
        assert_eq!(diff.regressions[0].metric, "cambi");
    }

    #[test]
    fn a_colour_shift_is_caught_even_when_vmaf_holds() {
        let mut worse = scores(95.0, 0.1);
        worse.ciede2000 = Some(45.0);
        let diff = compare(
            &report(&[("1080p", scores(95.0, 0.1))]),
            &report(&[("1080p", worse)]),
            &Tolerance::default(),
        );
        assert!(
            diff.regressions.iter().any(|r| r.metric == "ciede2000"),
            "{diff:?}"
        );
    }

    #[test]
    fn a_worst_frame_collapse_is_caught_even_when_the_mean_holds() {
        let mut cliff = scores(95.0, 0.1);
        cliff.vmaf_min = 70.0;
        let diff = compare(
            &report(&[("1080p", scores(95.0, 0.1))]),
            &report(&[("1080p", cliff)]),
            &Tolerance::default(),
        );
        assert!(
            diff.regressions.iter().any(|r| r.metric == "vmaf_min"),
            "{diff:?}"
        );
    }

    #[test]
    fn a_rung_that_disappeared_is_reported_separately_from_quality() {
        let before = report(&[("1080p", scores(95.0, 0.1)), ("720p", scores(93.0, 0.2))]);
        let after = report(&[("1080p", scores(95.0, 0.1))]);

        let diff = compare(&before, &after, &Tolerance::default());
        assert_eq!(diff.missing, ["rotated.mp4/720p"]);
        assert!(diff.regressions.is_empty());
        assert!(!diff.is_clean(), "losing a rendition must not merge");
    }

    #[test]
    fn a_new_rung_is_noted_but_does_not_block() {
        let before = report(&[("1080p", scores(95.0, 0.1))]);
        let after = report(&[("1080p", scores(95.0, 0.1)), ("1440p", scores(96.0, 0.1))]);

        let diff = compare(&before, &after, &Tolerance::default());
        assert_eq!(diff.added, ["rotated.mp4/1440p"]);
        assert!(diff.is_clean());
    }

    #[test]
    fn every_regression_names_the_file_and_rung() {
        let before = report(&[("1080p", scores(95.0, 0.1))]);
        let after = report(&[("1080p", scores(80.0, 0.9))]);

        for r in compare(&before, &after, &Tolerance::default()).regressions {
            assert_eq!(r.file, "rotated.mp4");
            assert_eq!(r.rung, "1080p");
        }
    }

    #[test]
    fn structural_findings_are_collected_with_their_file() {
        let mut r = report(&[("1080p", scores(95.0, 0.1))]);
        r.entries[0].findings.push("720p FrameCount".into());
        assert_eq!(r.findings(), ["rotated.mp4: 720p FrameCount"]);
    }

    #[test]
    fn a_report_survives_a_json_round_trip() {
        let r = report(&[("1080p", scores(95.0, 0.1))]);
        let json = serde_json::to_string(&r).unwrap();
        assert_eq!(serde_json::from_str::<Report>(&json).unwrap(), r);
    }
}
