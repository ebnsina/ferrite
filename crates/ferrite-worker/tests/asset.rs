//! Asset mode end to end: source in, playable directory out.

#![cfg(feature = "ffmpeg")]

use ferrite_worker::asset::{self, AssetError, Request};
use std::path::PathBuf;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Option<Self> {
        // Packaging needs the vendored binary; without it there is no asset.
        if !ferrite_av::package::binary().is_file() {
            eprintln!("skipped: no packager — run scripts/fetch-packager.sh");
            return None;
        }
        let dir = std::env::temp_dir().join(format!("ferrite-asset-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Self { dir })
    }

    fn source(&self, size: &str, seconds: u32, audio: bool) -> Option<PathBuf> {
        let path = self.dir.join("source.mp4");
        let mut command = std::process::Command::new("ffmpeg");
        command
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=size={size}:rate=30:duration={seconds}"));
        if audio {
            command
                .args(["-f", "lavfi", "-i"])
                .arg(format!("sine=frequency=440:duration={seconds}"));
        }
        command.args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-g",
            "60",
            "-pix_fmt",
            "yuv420p",
        ]);
        if audio {
            command.args(["-c:a", "aac", "-shortest"]);
        }
        command.arg(&path).status().ok()?.success().then_some(path)
    }

    fn out(&self) -> PathBuf {
        self.dir.join("asset")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_source_becomes_something_a_player_can_load() {
    let Some(fx) = Fixture::new("publish") else {
        return;
    };
    let Some(source) = fx.source("1280x720", 5, true) else {
        return;
    };

    let published = asset::run(&source, &fx.out(), &Request::default()).expect("publish");

    assert!(published.hls.is_file(), "no HLS master");
    assert!(published.dash.is_file(), "no DASH manifest");
    assert!(
        published.renditions.len() >= 3,
        "{:?}",
        published.renditions
    );
    assert!(published.audio, "a source with sound published silent");

    // Every rendition the master advertises must actually be on disk.
    let master = std::fs::read_to_string(&published.hls).expect("read master");
    for r in &published.renditions {
        assert!(
            master.contains(&format!("{}/video.m3u8", r.name)),
            "{} missing",
            r.name
        );
        assert!(
            published
                .hls
                .parent()
                .unwrap()
                .join(&r.name)
                .join("init.mp4")
                .is_file()
        );
    }
    assert!(
        master.contains("audio/audio.m3u8"),
        "no audio in the master"
    );
}

#[test]
fn nothing_is_published_without_something_to_review() {
    let Some(fx) = Fixture::new("review") else {
        return;
    };
    let Some(source) = fx.source("640x360", 4, false) else {
        return;
    };

    let published = asset::run(&source, &fx.out(), &Request::default()).expect("publish");
    assert_eq!(
        published.hashes.len(),
        60,
        "the blocklist got nothing to match on"
    );
    assert!(
        published.contact_sheet.is_file(),
        "no contact sheet for a reviewer"
    );
    assert_eq!(published.thumbnails, 60, "no scrub previews");
}

#[test]
fn a_silent_source_publishes_without_an_audio_track() {
    let Some(fx) = Fixture::new("silent") else {
        return;
    };
    let Some(source) = fx.source("640x360", 4, false) else {
        return;
    };

    let published = asset::run(&source, &fx.out(), &Request::default()).expect("publish");
    assert!(!published.audio, "silence produced a track");

    let master = std::fs::read_to_string(&published.hls).expect("read master");
    assert!(
        !master.contains("audio/audio.m3u8"),
        "an empty audio track was advertised"
    );
}

#[test]
fn the_fast_path_publishes_one_rung() {
    // The promise is time to first play, so the fast path encodes one rung and
    // the rest follow.
    let Some(fx) = Fixture::new("fast") else {
        return;
    };
    let Some(source) = fx.source("1280x720", 4, false) else {
        return;
    };

    let published = asset::run(
        &source,
        &fx.out(),
        &Request {
            fast_only: true,
            ..Request::default()
        },
    )
    .expect("publish");
    assert_eq!(published.renditions.len(), 1, "{:?}", published.renditions);
}

#[test]
fn every_rendition_agrees_before_anything_is_published() {
    let Some(fx) = Fixture::new("agree") else {
        return;
    };
    let Some(source) = fx.source("1280x720", 5, false) else {
        return;
    };

    let published = asset::run(&source, &fx.out(), &Request::default()).expect("publish");
    let frames: Vec<u64> = published.renditions.iter().map(|r| r.frames).collect();
    assert!(
        frames.windows(2).all(|w| w[0] == w[1]),
        "frame counts differ: {frames:?}"
    );

    // Smaller pictures cost fewer bytes, or the rungs are not what they say.
    let bytes: Vec<u64> = published.renditions.iter().map(|r| r.bytes).collect();
    assert!(
        bytes.windows(2).all(|w| w[0] > w[1]),
        "rung sizes not descending: {bytes:?}"
    );
}

#[test]
fn a_source_with_no_video_is_refused_clearly() {
    let Some(fx) = Fixture::new("novideo") else {
        return;
    };
    let junk = fx.dir.join("junk.mp4");
    std::fs::write(&junk, b"not a video").unwrap();

    let err = asset::run(&junk, &fx.out(), &Request::default()).unwrap_err();
    assert!(
        matches!(err, AssetError::Av(_) | AssetError::NoVideo(_)),
        "{err}"
    );
}
