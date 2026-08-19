//! Job mode against real files: one in, one out, and nothing published
//! unreviewed.

#![cfg(feature = "ffmpeg")]

use ferrite_worker::job::{self, JobError, Request};
use std::path::PathBuf;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Option<Self> {
        let dir = std::env::temp_dir().join(format!("ferrite-job-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Self { dir })
    }

    fn source(&self, name: &str, args: &[&str]) -> Option<PathBuf> {
        let path = self.dir.join(format!("{name}.mp4"));
        let ok = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y"])
            .args(args)
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
            .arg(&path)
            .status()
            .ok()?;
        ok.success().then_some(path)
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

#[test]
fn one_file_in_one_file_out() {
    let Some(fx) = Fixture::new("basic") else {
        return;
    };
    let Some(source) = fx.source(
        "source",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=30:duration=3",
        ],
    ) else {
        return;
    };

    let out = fx.out("output.mp4");
    let done = job::run(
        &source,
        &out,
        &Request {
            height: Some(480),
            ..Request::default()
        },
    )
    .expect("job");

    assert_eq!((done.width, done.height), (854, 480));
    assert!(done.frames > 0);
    assert!(done.bytes > 0);
    assert!(out.is_file());

    // Reading it back is the check that it is a real file, not just bytes.
    let produced = ferrite_av::probe(&out).expect("output opens");
    let video = produced.primary_video().expect("video stream");
    assert_eq!((video.width, video.height), (854, 480));
}

#[test]
fn nothing_is_published_without_something_to_review() {
    // Job mode has no mezzanine to hang the sampling pass off, and the
    // alternative is publishing files nobody has looked at.
    let Some(fx) = Fixture::new("review") else {
        return;
    };
    let Some(source) = fx.source(
        "source",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:duration=3",
        ],
    ) else {
        return;
    };

    let done = job::run(&source, &fx.out("out.mp4"), &Request::default()).expect("job");
    assert_eq!(
        done.hashes.len(),
        60,
        "the blocklist got nothing to match on"
    );
    assert!(
        done.contact_sheet.is_file(),
        "no contact sheet for a reviewer"
    );
}

#[test]
fn a_request_above_the_source_does_not_upscale() {
    // A customer asking for 1080p from a 360p file gets 360p, not a bigger
    // file carrying the same detail.
    let Some(fx) = Fixture::new("upscale") else {
        return;
    };
    let Some(source) = fx.source(
        "small",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:duration=2",
        ],
    ) else {
        return;
    };

    let done = job::run(
        &source,
        &fx.out("out.mp4"),
        &Request {
            height: Some(1080),
            ..Request::default()
        },
    )
    .expect("job");
    assert_eq!((done.width, done.height), (640, 360));
}

#[test]
fn a_rotated_source_comes_out_upright() {
    let Some(fx) = Fixture::new("rotated") else {
        return;
    };
    let Some(source) = fx.source(
        "source",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=30:duration=2",
        ],
    ) else {
        return;
    };

    let tagged = fx.out("rotated.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-display_rotation", "90", "-i"])
        .arg(&source)
        .args(["-c", "copy"])
        .arg(&tagged)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let done = job::run(&tagged, &fx.out("out.mp4"), &Request::default()).expect("job");
    assert!(
        done.height > done.width,
        "portrait source came out landscape"
    );
    assert!(
        done.mezzanine,
        "rotation is a problem a normalising pass fixes"
    );
}

#[test]
fn a_silent_source_is_not_charged_for_a_mezzanine() {
    let Some(fx) = Fixture::new("silent") else {
        return;
    };
    let Some(source) = fx.source(
        "source",
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x360:rate=30:duration=2",
        ],
    ) else {
        return;
    };

    let done = job::run(&source, &fx.out("out.mp4"), &Request::default()).expect("job");
    assert!(
        !done.mezzanine,
        "silence is not a defect a mezzanine repairs"
    );
}

#[test]
fn a_variable_frame_rate_source_comes_out_at_a_fixed_rate() {
    let Some(fx) = Fixture::new("vfr") else {
        return;
    };
    let path = fx.out("vfr.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=size=640x360:rate=30:duration=4")
        .args(["-vf", "setpts=N/(30+random(0)*20)/TB", "-fps_mode", "vfr"])
        .args(["-c:v", "libx264", "-preset", "ultrafast"])
        .arg(&path)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let done = job::run(&path, &fx.out("out.mp4"), &Request::default()).expect("job");
    assert!(done.mezzanine, "variable frame rate needs normalising");

    let produced = ferrite_av::probe(&done.path).expect("output opens");
    assert!(
        !produced
            .warnings
            .contains(&ferrite_av::Warning::VariableFrameRate),
        "the output is still variable rate"
    );
}

#[test]
fn a_source_with_no_video_is_refused_clearly() {
    let Some(fx) = Fixture::new("novideo") else {
        return;
    };
    let audio = fx.out("audio.m4a");
    let ok = std::process::Command::new("ffmpeg")
        .args([
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
        ])
        .args(["-c:a", "aac"])
        .arg(&audio)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let err = job::run(&audio, &fx.out("out.mp4"), &Request::default()).unwrap_err();
    assert!(
        matches!(err, JobError::NoVideo(_) | JobError::Av(_)),
        "{err}"
    );
}

#[test]
fn a_corrupt_source_errors_rather_than_hanging() {
    let Some(fx) = Fixture::new("corrupt") else {
        return;
    };
    let junk = fx.out("junk.mp4");
    std::fs::write(&junk, b"not a video at all").unwrap();

    assert!(job::run(&junk, &fx.out("out.mp4"), &Request::default()).is_err());
}
