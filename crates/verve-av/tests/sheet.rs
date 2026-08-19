//! Contact sheet and perceptual hashing against real files.

#![cfg(feature = "ffmpeg")]

use std::path::PathBuf;
use verve_av::phash;
use verve_av::sheet::{self, CELL, FRAMES, GRID};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Option<Self> {
        let dir = std::env::temp_dir().join(format!("verve-sheet-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Self { dir })
    }

    /// A source whose *brightness* keeps moving. dHash is luma-only by design —
    /// it has to survive colour grading — so a hue sweep would look static.
    fn moving(&self, name: &str, seconds: u32) -> Option<PathBuf> {
        let path = self.dir.join(format!("{name}.mp4"));
        let ok = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("color=c=black:s=640x360:r=30:d={seconds}"))
            .args([
                "-vf",
                "geq=lum='255*(0.5+0.5*sin(2*PI*(X/W+T/7)))':cb=128:cr=128",
            ])
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

fn dimensions(path: &std::path::Path) -> Option<(u32, u32)> {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.trim().split(',');
    Some((
        parts.next()?.trim().parse().ok()?,
        parts.next()?.trim().parse().ok()?,
    ))
}

#[test]
fn a_sheet_is_sixty_frames_in_a_ten_by_six_grid() {
    let Some(fx) = Fixture::new("grid") else {
        return;
    };
    let Some(source) = fx.moving("source", 120) else {
        return;
    };

    let out = fx.out("sheet.jpg");
    let sheet = sheet::build(&source, &out).expect("build sheet");

    assert_eq!(sheet.samples.len(), FRAMES as usize);
    assert!(out.is_file(), "no sheet written");

    let Some((w, h)) = dimensions(&out) else {
        return;
    };
    assert_eq!((w, h), (GRID.0 * CELL.0, GRID.1 * CELL.1));

    // A reviewer opens this; it must not be a multi-megabyte download.
    let bytes = std::fs::metadata(&out).unwrap().len();
    assert!(bytes < 1_500_000, "sheet is {bytes} bytes");
}

#[test]
fn samples_are_spread_across_the_whole_source() {
    let Some(fx) = Fixture::new("spread") else {
        return;
    };
    let Some(source) = fx.moving("source", 120) else {
        return;
    };

    let sheet = sheet::build(&source, &fx.out("sheet.jpg")).expect("build sheet");
    let times: Vec<u64> = sheet.samples.iter().map(|s| s.time_ms).collect();

    assert!(
        times.windows(2).all(|w| w[0] < w[1]),
        "samples are not in order"
    );
    assert!(
        times[0] < 5_000,
        "first sample at {}ms is too late",
        times[0]
    );
    assert!(*times.last().unwrap() > 110_000, "last sample is too early");

    // Evenly spaced, not clustered.
    let gaps: Vec<u64> = times.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(
        gaps.windows(2).all(|w| w[0] == w[1]),
        "uneven spacing: {gaps:?}"
    );
}

#[test]
fn different_frames_hash_differently() {
    let Some(fx) = Fixture::new("distinct") else {
        return;
    };
    let Some(source) = fx.moving("source", 120) else {
        return;
    };

    let sheet = sheet::build(&source, &fx.out("sheet.jpg")).expect("build sheet");
    let distinct: std::collections::HashSet<i64> = sheet.samples.iter().map(|s| s.phash).collect();

    assert!(
        distinct.len() > 5,
        "only {} distinct hashes in 60 frames",
        distinct.len()
    );
}

#[test]
fn a_colour_change_alone_does_not_move_the_hash() {
    // Deliberate: dHash compares luminance, so a re-grade or a colour shift
    // still matches. That is what makes it survive a re-encode.
    let Some(fx) = Fixture::new("hue") else {
        return;
    };
    let Some(source) = fx.moving("source", 30) else {
        return;
    };

    let regraded = fx.out("regraded.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(&source)
        .args([
            "-vf",
            "hue=H=2.1",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "60",
        ])
        .arg(&regraded)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let original = sheet::build(&source, &fx.out("a.jpg")).expect("original");
    let shifted = sheet::build(&regraded, &fx.out("b.jpg")).expect("regraded");
    assert!(
        shifted.blocklist_hit(&original.hashes()).is_some(),
        "a colour change defeated the hash"
    );
}

#[test]
fn a_reupload_is_caught_even_after_a_hard_re_encode() {
    // What the blocklist is for. Sampling by time does not land on the same
    // pictures, so a single frame matching is the bar — and it is enough.
    let Some(fx) = Fixture::new("reupload") else {
        return;
    };
    let Some(source) = fx.moving("source", 120) else {
        return;
    };

    let original = sheet::build(&source, &fx.out("original.jpg")).expect("original");
    let blocklist = original.hashes();

    // Re-uploaded at a third the size and badly compressed.
    let reencoded = fx.out("reencoded.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-i"])
        .arg(&source)
        .args([
            "-vf",
            "scale=214:120",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "38",
        ])
        .arg(&reencoded)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let suspect = sheet::build(&reencoded, &fx.out("suspect.jpg")).expect("suspect");
    let hit = suspect.blocklist_hit(&blocklist);
    assert!(
        hit.is_some(),
        "a re-encoded re-upload slipped past the blocklist"
    );

    let (sample, _, distance) = hit.unwrap();
    assert!(distance <= phash::MATCH_DISTANCE, "matched at {distance}");
    assert!(sample.time_ms > 0);
}

#[test]
fn unrelated_content_does_not_trip_the_blocklist() {
    // A false positive on a paying customer's legitimate video is worse than a
    // miss caught on report.
    let Some(fx) = Fixture::new("unrelated") else {
        return;
    };

    let blocked = fx.out("blocked.mp4");
    let innocent = fx.out("innocent.mp4");
    for (path, filter) in [(&blocked, "smptebars"), (&innocent, "testsrc2")] {
        let ok = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("{filter}=size=640x360:rate=30:duration=60"))
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
            .arg(path)
            .status()
            .is_ok_and(|s| s.success());
        if !ok {
            return;
        }
    }

    let blocklist = sheet::build(&blocked, &fx.out("b.jpg"))
        .expect("blocked")
        .hashes();
    let suspect = sheet::build(&innocent, &fx.out("i.jpg")).expect("innocent");

    assert!(
        suspect.blocklist_hit(&blocklist).is_none(),
        "unrelated content was flagged"
    );
}

#[test]
fn hashes_round_trip_through_a_signed_bigint() {
    let Some(fx) = Fixture::new("bigint") else {
        return;
    };
    let Some(source) = fx.moving("source", 30) else {
        return;
    };

    let sheet = sheet::build(&source, &fx.out("sheet.jpg")).expect("build sheet");
    for (stored, live) in sheet.samples.iter().map(|s| s.phash).zip(sheet.hashes()) {
        assert_eq!(
            stored as u64, live,
            "a hash changed sign on the way to storage"
        );
    }
}

#[test]
fn a_short_source_still_produces_a_sheet() {
    // A one-second clip has fewer distinct pictures than cells; empty cells are
    // grey rather than a failure.
    let Some(fx) = Fixture::new("short") else {
        return;
    };
    let Some(source) = fx.moving("source", 1) else {
        return;
    };

    let out = fx.out("sheet.jpg");
    let sheet = sheet::build(&source, &out).expect("build sheet");
    assert!(!sheet.samples.is_empty());
    assert!(out.is_file());
}

#[test]
fn a_file_that_is_not_video_is_an_error_not_a_panic() {
    let Some(fx) = Fixture::new("garbage") else {
        return;
    };
    let junk = fx.out("junk.mp4");
    std::fs::write(&junk, b"this is not a video").unwrap();

    assert!(sheet::build(&junk, &fx.out("sheet.jpg")).is_err());
}
