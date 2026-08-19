//! Objective quality: VMAF, PSNR, SSIM, MS-SSIM, CIEDE2000, CAMBI.
//!
//! One libvmaf pass produces every metric. VMAF alone is not enough — a build
//! can hold VMAF steady while introducing visible banding (CAMBI) or a colour
//! shift (CIEDE2000), which are exactly the bugs a tired human misses at 2am.
//!
//! A subprocess, not a binding: this runs in `ferrite quality` and `ferrite bench`,
//! never in the worker pipeline, so linking libvmaf into every worker is cost
//! for nothing.

use crate::error::{AvError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Assumes 1080p at normal viewing distance. `vmaf_4k_v0.6.1` is for 4K, and
/// the wrong model is the wrong answer.
pub const DEFAULT_MODEL: &str = "version=vmaf_v0.6.1";

/// Top rung against the mezzanine.
pub const GATE_VMAF: f64 = 93.0;

/// Chunked against single-pass with the same settings.
pub const GATE_DELTA_VMAF: f64 = 0.5;

/// How to measure.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// libvmaf model.
    pub model: String,
    /// Score every Nth frame. Fine per-commit, never for a release gate.
    pub subsample: u32,
    /// Threads. Zero lets libvmaf decide.
    pub threads: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            model: DEFAULT_MODEL.to_string(),
            subsample: 1,
            threads: 0,
        }
    }
}

/// One metric's pooled scores.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pooled {
    /// Mean across scored frames.
    pub mean: f64,
    /// Worst frame. A good mean hides a terrible scene.
    pub min: f64,
    /// Best frame.
    pub max: f64,
}

/// What one comparison found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
    /// Overall perceived quality. The headline number.
    pub vmaf: Pooled,
    /// Luma signal error.
    pub psnr_y: Pooled,
    /// Structural similarity.
    pub ssim: Option<Pooled>,
    /// Multi-scale structural similarity.
    pub ms_ssim: Option<Pooled>,
    /// Colour difference: wrong matrix, bad tonemapping, range errors.
    pub ciede2000: Option<Pooled>,
    /// Banding. VMAF barely sees this; humans do.
    pub cambi: Option<Pooled>,
    /// Frames scored.
    pub frames: usize,
}

impl Metrics {
    /// Whether the headline number clears `threshold`.
    pub fn passes(&self, threshold: f64) -> bool {
        self.vmaf.mean >= threshold
    }
}

/// Score `distorted` against `reference`.
///
/// The distorted file is scaled back to the reference's size first: comparing
/// 480p to 1080p without upscaling produces meaningless numbers. The reference
/// should be the mezzanine — comparing two encodes tells you they differ, not
/// which is correct.
pub fn measure(reference: &Path, distorted: &Path, options: &Options) -> Result<Metrics> {
    let (width, height) = dimensions(reference)?;
    let log = temp_log(distorted);

    let chain = format!(
        "[0:v]scale={width}:{height}:flags=bicubic,setpts=PTS-STARTPTS[dist];\
         [1:v]setpts=PTS-STARTPTS[ref];\
         [dist][ref]libvmaf=log_path={}:log_fmt=json:model={}:n_subsample={}:n_threads={}:\
         feature='name=psnr|name=float_ssim|name=float_ms_ssim|name=ciede|name=cambi'",
        log.display(),
        options.model,
        options.subsample.max(1),
        options.threads,
    );

    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error", "-i"])
        .arg(distorted)
        .arg("-i")
        .arg(reference)
        .args(["-lavfi", &chain, "-f", "null", "-"])
        .output()
        .map_err(|e| AvError::CodecUnavailable {
            codec: "libvmaf".into(),
            reason: format!("cannot run ffmpeg: {e}"),
        })?;

    if !output.status.success() {
        let _ = std::fs::remove_file(&log);
        return Err(AvError::InvalidSpec(format!(
            "quality run failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let report = std::fs::read_to_string(&log)?;
    let _ = std::fs::remove_file(&log);
    parse(&report)
}

/// Pull the metrics out of a libvmaf JSON report.
pub fn parse(report: &str) -> Result<Metrics> {
    let json: serde_json::Value = serde_json::from_str(report)
        .map_err(|e| AvError::InvalidSpec(format!("libvmaf report is not JSON: {e}")))?;

    let pooled = json
        .get("pooled_metrics")
        .ok_or_else(|| AvError::InvalidSpec("libvmaf report has no pooled_metrics".into()))?;

    let read = |name: &str| -> Option<Pooled> {
        let m = pooled.get(name)?;
        Some(Pooled {
            mean: m.get("mean")?.as_f64()?,
            min: m.get("min")?.as_f64()?,
            max: m.get("max")?.as_f64()?,
        })
    };

    Ok(Metrics {
        vmaf: read("vmaf")
            .ok_or_else(|| AvError::InvalidSpec("libvmaf reported no VMAF score".into()))?,
        psnr_y: read("psnr_y")
            .ok_or_else(|| AvError::InvalidSpec("libvmaf reported no PSNR".into()))?,
        ssim: read("float_ssim"),
        ms_ssim: read("float_ms_ssim"),
        ciede2000: read("ciede2000"),
        cambi: read("cambi"),
        frames: json
            .get("frames")
            .and_then(|f| f.as_array())
            .map_or(0, Vec::len),
    })
}

fn dimensions(path: &Path) -> Result<(u32, u32)> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map_err(|e| AvError::CodecUnavailable {
            codec: "ffprobe".into(),
            reason: format!("cannot run ffprobe: {e}"),
        })?;

    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.trim().split(',');
    let parsed = (|| {
        Some((
            parts.next()?.trim().parse().ok()?,
            parts.next()?.trim().parse().ok()?,
        ))
    })();

    parsed.ok_or_else(|| AvError::OpenInput {
        path: path.display().to_string(),
        source: "no video dimensions".into(),
    })
}

fn temp_log(distorted: &Path) -> std::path::PathBuf {
    let stem = distorted.file_stem().unwrap_or_default().to_string_lossy();
    // Same directory as the file under test, so a crashed run leaves the report
    // next to what produced it rather than somewhere nobody looks.
    let dir = distorted.parent().filter(|d| !d.as_os_str().is_empty());
    let name = format!("{stem}.vmaf.json");
    match dir {
        Some(d) => d.join(name),
        None => std::env::temp_dir().join(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPORT: &str = r#"{
      "frames": [{"frameNum": 0}, {"frameNum": 1}],
      "pooled_metrics": {
        "vmaf":          {"min": 61.5, "max": 70.2, "mean": 64.7, "harmonic_mean": 64.6},
        "psnr_y":        {"min": 33.6, "max": 35.1, "mean": 34.0, "harmonic_mean": 34.0},
        "float_ssim":    {"min": 0.993, "max": 0.996, "mean": 0.994, "harmonic_mean": 0.994},
        "float_ms_ssim": {"min": 0.988, "max": 0.990, "mean": 0.989, "harmonic_mean": 0.989},
        "ciede2000":     {"min": 43.2, "max": 45.9, "mean": 44.5, "harmonic_mean": 44.5},
        "cambi":         {"min": 0.22, "max": 0.91, "mean": 0.60, "harmonic_mean": 0.55}
      }
    }"#;

    #[test]
    fn every_metric_is_read_from_the_report() {
        let m = parse(REPORT).unwrap();
        assert_eq!(m.vmaf.mean, 64.7);
        assert_eq!(m.vmaf.min, 61.5);
        assert_eq!(m.psnr_y.mean, 34.0);
        assert_eq!(m.ssim.unwrap().mean, 0.994);
        assert_eq!(m.ms_ssim.unwrap().mean, 0.989);
        assert_eq!(m.ciede2000.unwrap().mean, 44.5);
        assert_eq!(m.cambi.unwrap().mean, 0.60);
        assert_eq!(m.frames, 2);
    }

    #[test]
    fn the_worst_frame_is_kept_because_a_good_mean_hides_a_bad_scene() {
        let m = parse(REPORT).unwrap();
        assert!(m.vmaf.min < m.vmaf.mean);
    }

    #[test]
    fn a_report_without_vmaf_is_an_error_not_a_zero() {
        let no_vmaf = r#"{"pooled_metrics": {"psnr_y": {"min":1,"max":2,"mean":1.5}}}"#;
        assert!(parse(no_vmaf).is_err());
    }

    #[test]
    fn missing_optional_metrics_are_none_rather_than_fatal() {
        let minimal = r#"{"pooled_metrics": {
            "vmaf":   {"min":90,"max":95,"mean":93},
            "psnr_y": {"min":40,"max":45,"mean":42}
        }}"#;
        let m = parse(minimal).unwrap();
        assert!(m.cambi.is_none() && m.ciede2000.is_none());
        assert_eq!(m.vmaf.mean, 93.0);
    }

    #[test]
    fn garbage_is_rejected_with_a_readable_error() {
        let err = parse("not json at all").unwrap_err().to_string();
        assert!(err.contains("not JSON"), "{err}");
        assert!(parse("{}").is_err());
    }

    #[test]
    fn the_gate_is_the_documented_ninety_three() {
        let m = parse(REPORT).unwrap();
        assert_eq!(GATE_VMAF, 93.0);
        assert!(
            !m.passes(GATE_VMAF),
            "a 64.7 VMAF rung must not pass the top-rung gate"
        );
        assert!(m.passes(60.0));
    }

    #[test]
    fn the_default_model_is_the_1080p_one() {
        assert_eq!(Options::default().model, "version=vmaf_v0.6.1");
        assert_eq!(
            Options::default().subsample,
            1,
            "a release gate scores every frame"
        );
    }
}
