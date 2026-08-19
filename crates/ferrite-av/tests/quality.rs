//! Objective quality against real encodes. Skips unless this ffmpeg was built
//! with libvmaf.

#![cfg(feature = "ffmpeg")]

use ferrite_av::NeverCancel;
use ferrite_av::encoder::Preset;
use ferrite_av::quality::{self, GATE_VMAF, Options};
use ferrite_av::transcode::{self, Output};
use std::path::PathBuf;
use std::sync::Arc;

fn has_libvmaf() -> bool {
    std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-filters"])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("libvmaf"))
}

struct Fixture {
    dir: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Option<Self> {
        if !has_libvmaf() {
            eprintln!("skipped: this ffmpeg has no libvmaf");
            return None;
        }

        let dir = std::env::temp_dir().join(format!("ferrite-quality-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;

        let source = dir.join("source.mp4");
        let made = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg("testsrc2=size=640x360:rate=30:duration=2")
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                "60",
                "-pix_fmt",
                "yuv420p",
            ])
            .arg(&source)
            .status()
            .ok()?;
        made.success().then_some(Self { dir, source })
    }

    fn out(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Fast enough for a test; a release gate scores every frame.
fn options() -> Options {
    Options {
        subsample: 3,
        ..Options::default()
    }
}

#[test]
fn a_faithful_encode_clears_the_top_rung_gate() {
    let Some(fx) = Fixture::new("gate") else {
        return;
    };

    let info = ferrite_av::probe(&fx.source).expect("probe");
    let video = info.primary_video().expect("video");
    let spec = ferrite_av::EncodeSpec::new(
        ferrite_av::VideoCodecName::H264,
        ferrite_av::Resolution::new(video.width, video.height),
        video.frame_rate,
    )
    .with_preset(Preset::Medium)
    .with_rate_control(ferrite_av::RateControl::crf(18, 8_000_000));

    let same_size = fx.out("faithful.mp4");
    transcode::run(
        &fx.source,
        &[Output {
            path: same_size.clone(),
            spec,
        }],
        Arc::new(NeverCancel),
    )
    .expect("transcode");

    let m = quality::measure(&fx.source, &same_size, &options()).expect("measure");
    assert!(m.frames > 0, "nothing was scored");
    assert!(
        m.passes(GATE_VMAF),
        "a same-size CRF 18 encode scored {:.2}, below the {GATE_VMAF} gate",
        m.vmaf.mean
    );
    assert!(m.cambi.is_some(), "no banding metric");
    assert!(m.ciede2000.is_some(), "no colour metric");
}

#[test]
fn quality_falls_monotonically_down_the_ladder() {
    // If a lower rung ever scores higher, the ladder is misconfigured.
    let Some(fx) = Fixture::new("ladder") else {
        return;
    };

    let info = ferrite_av::probe(&fx.source).expect("probe");
    let video = info.primary_video().expect("video");
    let steps = ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, Preset::Ultrafast);
    if steps.len() < 2 {
        return;
    }

    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: fx.out(&format!("{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();
    transcode::run(&fx.source, &outputs, Arc::new(NeverCancel)).expect("transcode");

    let scores: Vec<f64> = outputs
        .iter()
        .map(|o| {
            quality::measure(&fx.source, &o.path, &options())
                .expect("measure")
                .vmaf
                .mean
        })
        .collect();

    assert!(
        scores.windows(2).all(|w| w[0] > w[1]),
        "VMAF is not descending down the ladder: {scores:?}"
    );
}

#[test]
fn a_deliberately_bad_encode_scores_far_worse() {
    let Some(fx) = Fixture::new("bad") else {
        return;
    };

    let ruined = fx.out("ruined.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(&fx.source)
        .args([
            "-vf",
            "scale=64:36,scale=640:360",
            "-c:v",
            "libx264",
            "-crf",
            "45",
        ])
        .arg(&ruined)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let m = quality::measure(&fx.source, &ruined, &options()).expect("measure");
    assert!(
        !m.passes(GATE_VMAF),
        "a ruined encode passed the gate at {:.2}",
        m.vmaf.mean
    );
    assert!(
        m.vmaf.mean < 60.0,
        "expected a low score, got {:.2}",
        m.vmaf.mean
    );
}

#[test]
fn the_worst_frame_is_reported_alongside_the_mean() {
    let Some(fx) = Fixture::new("worst") else {
        return;
    };

    let m = quality::measure(&fx.source, &fx.source, &options()).expect("measure");
    assert!(m.vmaf.min <= m.vmaf.mean);
    assert!(m.vmaf.mean <= m.vmaf.max);
}

#[test]
fn a_file_compared_with_itself_is_effectively_perfect() {
    let Some(fx) = Fixture::new("identity") else {
        return;
    };

    let m = quality::measure(&fx.source, &fx.source, &options()).expect("measure");
    assert!(
        m.vmaf.mean > 99.0,
        "a file scored {:.2} against itself",
        m.vmaf.mean
    );
    assert!(
        m.psnr_y.mean > 50.0,
        "PSNR against itself was {:.2}",
        m.psnr_y.mean
    );
}

#[test]
fn a_missing_file_is_an_error_not_a_zero_score() {
    let Some(fx) = Fixture::new("missing") else {
        return;
    };
    assert!(quality::measure(&fx.source, &fx.out("nope.mp4"), &options()).is_err());
}

#[test]
fn the_report_is_cleaned_up_after_a_run() {
    let Some(fx) = Fixture::new("cleanup") else {
        return;
    };
    quality::measure(&fx.source, &fx.source, &options()).expect("measure");

    let leftovers: Vec<_> = std::fs::read_dir(&fx.dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".vmaf.json"))
        .collect();
    assert!(leftovers.is_empty(), "a report was left behind");
}
