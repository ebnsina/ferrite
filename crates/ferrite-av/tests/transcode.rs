//! Decode-once fan-out against real files. The properties chunking depends on
//! have to hold in the bytes, not just in the planner.

#![cfg(feature = "ffmpeg")]

use ferrite_av::encoder::{CancelSignal, Preset};
use ferrite_av::transcode::{self, Output};
use ferrite_av::{NeverCancel, Resolution};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A synthetic source, generated once per test into a fresh directory.
struct Fixture {
    dir: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new(name: &str, size: &str, seconds: u32) -> Option<Self> {
        let dir = std::env::temp_dir().join(format!("ferrite-transcode-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;

        let source = dir.join("source.mp4");
        let status = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=size={size}:rate=30:duration={seconds}"))
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

        // No ffmpeg binary on this machine: skip rather than fail.
        status.success().then_some(Self { dir, source })
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

fn ladder(source: &Path) -> Vec<ferrite_av::ladder::Step> {
    let info = ferrite_av::probe(source).expect("probe");
    let video = info.primary_video().expect("video stream");
    ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, Preset::Ultrafast)
}

#[test]
fn one_decode_feeds_every_rung() {
    let Some(fx) = Fixture::new("ladder", "1280x720", 4) else {
        return;
    };

    let steps = ladder(&fx.source);
    assert!(
        steps.len() >= 3,
        "expected a real ladder, got {}",
        steps.len()
    );

    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: fx.out(&format!("{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();

    let reports = transcode::run(&fx.source, &outputs, Arc::new(NeverCancel)).expect("transcode");
    assert_eq!(reports.len(), steps.len());

    let frames = reports[0].frames;
    assert!(frames > 0, "nothing was encoded");
    for (report, step) in reports.iter().zip(&steps) {
        assert_eq!(
            report.frames, frames,
            "{} saw a different frame count",
            step.name
        );
        assert!(report.bytes > 0, "{} produced no bytes", step.name);
        assert!(report.path.exists(), "{} was not written", step.name);
    }

    // Smaller pictures must cost fewer bytes, or the rungs are not what they say.
    let sizes: Vec<u64> = reports.iter().map(|r| r.bytes).collect();
    assert!(
        sizes.windows(2).all(|w| w[0] > w[1]),
        "rung sizes not descending: {sizes:?}"
    );
}

#[test]
fn every_rung_is_a_file_that_actually_decodes() {
    let Some(fx) = Fixture::new("decodes", "1280x720", 4) else {
        return;
    };

    let steps = ladder(&fx.source);
    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: fx.out(&format!("{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();
    let reports = transcode::run(&fx.source, &outputs, Arc::new(NeverCancel)).expect("transcode");

    // Reading our own output back is the check that catches a file which is
    // well-formed but carries no codec setup bytes.
    for (report, step) in reports.iter().zip(&steps) {
        let info = ferrite_av::probe(&report.path)
            .unwrap_or_else(|e| panic!("{} does not open: {e}", step.name));
        let video = info.primary_video().expect("video stream");

        assert_eq!(video.codec, "h264");
        assert_eq!(
            (video.width, video.height),
            (step.spec.resolution.width, step.spec.resolution.height)
        );
        assert_eq!(
            video.frame_rate, step.spec.frame_rate,
            "{} lost its frame rate",
            step.name
        );
        assert!(
            info.duration_ms >= 3_500,
            "{} is {}ms long",
            step.name,
            info.duration_ms
        );
    }
}

#[test]
fn keyframes_land_at_identical_times_in_every_rung() {
    // Split rule 2. If this drifts, players corrupt when switching quality.
    let Some(fx) = Fixture::new("keyframes", "1280x720", 8) else {
        return;
    };

    let steps = ladder(&fx.source);
    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: fx.out(&format!("{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();
    let reports = transcode::run(&fx.source, &outputs, Arc::new(NeverCancel)).expect("transcode");

    let mut per_rung: Vec<(&str, Vec<u64>)> = Vec::new();
    for (report, step) in reports.iter().zip(&steps) {
        let info = ferrite_av::probe(&report.path).expect("probe output");
        per_rung.push((step.name, info.keyframes_ms));
    }

    let (first_name, first) = &per_rung[0];
    assert!(
        first.len() > 1,
        "only one keyframe — the test proves nothing"
    );
    for (name, keys) in &per_rung[1..] {
        assert_eq!(
            keys, first,
            "{name} cuts at {keys:?}, {first_name} at {first:?}"
        );
    }
}

#[test]
fn a_mezzanine_normalizes_to_one_clean_copy() {
    let Some(fx) = Fixture::new("mezzanine", "1280x720", 4) else {
        return;
    };

    let info = ferrite_av::probe(&fx.source).expect("probe");
    let video = info.primary_video().expect("video stream").clone();
    let (w, h) = video.display_size();

    let spec = ferrite_av::EncodeSpec::new(
        ferrite_av::VideoCodecName::H264,
        Resolution::new(w, h),
        video.frame_rate,
    )
    .with_preset(Preset::Ultrafast);

    let out = fx.out("mezzanine.mp4");
    let report = transcode::mezzanine(&fx.source, &out, spec).expect("mezzanine");
    assert!(report.frames > 0);

    let clean = ferrite_av::probe(&out).expect("probe mezzanine");
    let clean_video = clean.primary_video().expect("video stream");
    assert_eq!(
        clean_video.display_size(),
        (w, h),
        "the mezzanine changed the picture size"
    );
    assert_eq!(clean_video.rotation_degrees, 0, "rotation was not baked in");
    assert!(
        !clean
            .warnings
            .contains(&ferrite_av::Warning::VariableFrameRate)
    );
}

/// Mean PSNR between two files, via ffmpeg. `None` if ffmpeg is unavailable.
fn psnr(a: &Path, b: &Path) -> Option<f64> {
    let out = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-i"])
        .arg(a)
        .arg("-i")
        .arg(b)
        .args(["-lavfi", "psnr", "-f", "null", "-"])
        .output()
        .ok()?;

    String::from_utf8_lossy(&out.stderr)
        .split("average:")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

fn ffmpeg(args: &[&str]) -> bool {
    std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y"])
        .args(args)
        .status()
        .is_ok_and(|s| s.success())
}

/// Rotation direction is the one thing here that cannot be reasoned out: the
/// clockwise and counter-clockwise conventions read identically and are
/// opposites. ffmpeg's own autorotate is the oracle.
#[test]
fn rotation_is_baked_in_the_same_way_ffmpeg_does_it() {
    let Some(fx) = Fixture::new("rotate-oracle", "640x360", 2) else {
        return;
    };

    for angle in [90, 180, 270] {
        let tagged = fx.out(&format!("tagged-{angle}.mp4"));
        if !ffmpeg(&[
            "-display_rotation",
            &angle.to_string(),
            "-i",
            fx.source.to_str().unwrap(),
            "-c",
            "copy",
            tagged.to_str().unwrap(),
        ]) {
            return;
        }

        let info = ferrite_av::probe(&tagged).expect("probe tagged");
        let video = info.primary_video().expect("video stream");
        let (w, h) = video.display_size();

        let spec = ferrite_av::EncodeSpec::new(
            ferrite_av::VideoCodecName::H264,
            Resolution::new(w & !1, h & !1),
            video.frame_rate,
        )
        .with_preset(Preset::Ultrafast);

        let ours = fx.out(&format!("ours-{angle}.mp4"));
        transcode::run(
            &tagged,
            &[Output {
                path: ours.clone(),
                spec,
            }],
            Arc::new(NeverCancel),
        )
        .expect("transcode");

        // ffmpeg autorotates on decode, so plain scale gives the upright answer.
        let reference = fx.out(&format!("ref-{angle}.mp4"));
        if !ffmpeg(&[
            "-i",
            tagged.to_str().unwrap(),
            "-vf",
            &format!("scale={}:{}", w & !1, h & !1),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-pix_fmt",
            "yuv420p",
            reference.to_str().unwrap(),
        ]) {
            return;
        }

        // Compare a frame, not the files: our output has no rotation tag left,
        // and psnr would re-rotate anything that still carried one.
        let (a, b) = (fx.out("a.png"), fx.out("b.png"));
        for (src, png) in [(&ours, &a), (&reference, &b)] {
            assert!(ffmpeg(&[
                "-i",
                src.to_str().unwrap(),
                "-vf",
                "select=eq(n\\,15)",
                "-vframes",
                "1",
                png.to_str().unwrap(),
            ]));
        }

        let Some(db) = psnr(&a, &b) else { return };
        assert!(
            db > 25.0,
            "{angle}° came out {db:.1} dB from ffmpeg's answer — wrong direction"
        );
        let _ = std::fs::remove_file(&a);
        let _ = std::fs::remove_file(&b);
    }
}

#[test]
fn an_unrotated_ladder_matches_what_ffmpeg_would_produce() {
    let Some(fx) = Fixture::new("oracle", "1280x720", 2) else {
        return;
    };

    let steps = ladder(&fx.source);
    let step = &steps[0];
    let ours = fx.out("ours.mp4");
    transcode::run(
        &fx.source,
        &[Output {
            path: ours.clone(),
            spec: step.spec.clone(),
        }],
        Arc::new(NeverCancel),
    )
    .expect("transcode");

    let reference = fx.out("ref.mp4");
    if !ffmpeg(&[
        "-i",
        fx.source.to_str().unwrap(),
        "-vf",
        &format!(
            "scale={}:{}",
            step.spec.resolution.width, step.spec.resolution.height
        ),
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p",
        reference.to_str().unwrap(),
    ]) {
        return;
    }

    let Some(db) = psnr(&ours, &reference) else {
        return;
    };
    assert!(db > 30.0, "our {} is {db:.1} dB from ffmpeg's", step.name);
}

#[test]
fn a_rotated_source_is_laddered_on_its_display_size() {
    let Some(fx) = Fixture::new("rotation", "1280x720", 2) else {
        return;
    };

    // Re-tag the source as rotated, as a phone would.
    let rotated = fx.out("rotated.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-display_rotation", "90", "-i"])
        .arg(&fx.source)
        .args(["-c", "copy"])
        .arg(&rotated)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let info = ferrite_av::probe(&rotated).expect("probe rotated");
    let video = info.primary_video().expect("video stream");
    assert_eq!(
        video.display_size(),
        (720, 1280),
        "rotation was not detected"
    );

    let steps = ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, Preset::Ultrafast);
    let top = &steps[0];
    assert!(
        top.spec.resolution.height > top.spec.resolution.width,
        "a portrait source was laddered as landscape: {}",
        top.spec.resolution
    );
}

#[test]
fn cancellation_stops_the_run_rather_than_finishing_it() {
    let Some(fx) = Fixture::new("cancel", "1280x720", 4) else {
        return;
    };

    struct Now(AtomicBool);
    impl CancelSignal for Now {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    let steps = ladder(&fx.source);
    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: fx.out(&format!("c-{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();

    let flag = Arc::new(Now(AtomicBool::new(true)));
    let err = transcode::run(&fx.source, &outputs, flag).expect_err("should have been cancelled");
    assert!(matches!(err, ferrite_av::AvError::Cancelled), "got {err}");
}

#[test]
fn a_truncated_rung_still_plays_and_is_caught_anyway() {
    // The exact failure the check step exists for: a dropped chunk leaves a
    // file that decodes without complaint.
    use ferrite_av::verify::{Rendition, verify};

    let Some(fx) = Fixture::new("truncated", "1280x720", 6) else {
        return;
    };

    let steps = ladder(&fx.source);
    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: fx.out(&format!("{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();
    let reports = transcode::run(&fx.source, &outputs, Arc::new(NeverCancel)).expect("transcode");

    let intact: Vec<ferrite_av::MediaInfo> = reports
        .iter()
        .map(|r| ferrite_av::probe(&r.path).expect("probe"))
        .collect();
    let named: Vec<Rendition<'_>> = steps
        .iter()
        .zip(&intact)
        .map(|(s, i)| Rendition {
            name: s.name,
            info: i,
        })
        .collect();
    assert!(
        verify(&named).is_ok(),
        "a fresh ladder should agree with itself"
    );

    // Cut the second rung short, as a lost chunk would.
    let short = fx.out("short.mp4");
    if !ffmpeg(&[
        "-i",
        reports[1].path.to_str().unwrap(),
        "-t",
        "3",
        "-c",
        "copy",
        short.to_str().unwrap(),
    ]) {
        return;
    }

    // It plays. That is why nothing else notices.
    let truncated = ferrite_av::probe(&short).expect("the truncated file still opens");

    let mut damaged = intact.clone();
    damaged[1] = truncated;
    let named: Vec<Rendition<'_>> = steps
        .iter()
        .zip(&damaged)
        .map(|(s, i)| Rendition {
            name: s.name,
            info: i,
        })
        .collect();

    let verdict = verify(&named);
    assert!(!verdict.is_ok(), "a short rung was published");
    assert!(
        verdict
            .findings
            .iter()
            .all(|f| f.rendition == steps[1].name),
        "blamed the wrong rung: {:?}",
        verdict.findings
    );
}

#[test]
fn an_empty_output_list_does_no_work() {
    let Some(fx) = Fixture::new("empty", "320x180", 1) else {
        return;
    };
    assert!(
        transcode::run(&fx.source, &[], Arc::new(NeverCancel))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_bad_spec_is_refused_before_the_file_is_opened() {
    let mut spec = ferrite_av::EncodeSpec::new(
        ferrite_av::VideoCodecName::H264,
        Resolution::new(641, 360),
        ferrite_av::Rational::new(30, 1),
    );
    spec.threads = 1;

    let outputs = [Output {
        path: PathBuf::from("/nonexistent/nope.mp4"),
        spec,
    }];
    let err = transcode::run(
        Path::new("/nonexistent/source.mp4"),
        &outputs,
        Arc::new(NeverCancel),
    )
    .expect_err("odd width must be refused");
    assert!(
        matches!(err, ferrite_av::AvError::InvalidSpec(_)),
        "got {err}"
    );
}
