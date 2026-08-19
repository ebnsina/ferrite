//! Audio normalisation: one track, encoded once, playable everywhere.

#![cfg(feature = "ffmpeg")]

use ferrite_av::audio::{self, AudioSpec};
use std::path::PathBuf;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Option<Self> {
        let dir = std::env::temp_dir().join(format!("ferrite-audio-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        Some(Self { dir })
    }

    /// A source with `codec` audio, or `None` if this build cannot make one.
    fn with_audio(&self, codec: &str, extra: &[&str]) -> Option<PathBuf> {
        let path = self.dir.join(format!("src-{codec}.mp4"));
        let ok = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg("testsrc2=size=320x180:rate=30:duration=3")
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=3"])
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-pix_fmt",
                "yuv420p",
            ])
            .args(["-c:a", codec])
            .args(extra)
            .args(["-shortest"])
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

fn probe_audio(path: &std::path::Path) -> Option<(String, u32, u16)> {
    let info = ferrite_av::probe(path).ok()?;
    let track = info.audio.first()?;
    Some((track.codec.clone(), track.sample_rate, track.channels))
}

#[test]
fn every_source_codec_comes_out_as_aac_stereo() {
    // The gap this closes: Shaka Packager refuses an MP3 soundtrack in an MP4
    // outright, so passing source audio through fails on real uploads.
    let Some(fx) = Fixture::new("codecs") else {
        return;
    };

    for (codec, extra) in [("aac", &[][..]), ("mp3", &[]), ("ac3", &[])] {
        let Some(source) = fx.with_audio(codec, extra) else {
            continue;
        };
        let out = fx.out(&format!("{codec}.m4a"));

        let report = audio::encode(&source, &out, &AudioSpec::default())
            .unwrap_or_else(|e| panic!("{codec}: {e}"))
            .unwrap_or_else(|| panic!("{codec}: no audio found"));

        assert_eq!(report.codec, "aac");
        assert!(report.bytes > 0, "{codec} produced no bytes");

        let (out_codec, rate, channels) =
            probe_audio(&out).unwrap_or_else(|| panic!("{codec}: output has no audio"));
        assert_eq!(out_codec, "aac", "{codec} did not normalise");
        assert_eq!(rate, 48_000);
        assert_eq!(channels, 2);
    }
}

#[test]
fn a_silent_source_produces_no_track_rather_than_an_empty_one() {
    let Some(fx) = Fixture::new("silent") else {
        return;
    };
    let silent = fx.out("silent.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=size=320x180:rate=30:duration=2")
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-an"])
        .arg(&silent)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let report = audio::encode(&silent, &fx.out("out.m4a"), &AudioSpec::default()).expect("encode");
    assert!(report.is_none(), "a silent source produced a track");
}

#[test]
fn surround_is_downmixed_to_stereo() {
    let Some(fx) = Fixture::new("surround") else {
        return;
    };
    let path = fx.out("surround.mp4");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=size=320x180:rate=30:duration=2")
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"])
        .args(["-af", "pan=5.1|c0=c0|c1=c0|c2=c0|c3=c0|c4=c0|c5=c0"])
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&path)
        .status()
        .is_ok_and(|s| s.success());
    if !ok {
        return;
    }

    let out = fx.out("stereo.m4a");
    audio::encode(&path, &out, &AudioSpec::default())
        .expect("encode")
        .expect("track");
    let (_, _, channels) = probe_audio(&out).expect("output audio");
    assert_eq!(channels, 2, "surround was not downmixed");
}

#[test]
fn the_track_lasts_as_long_as_the_source() {
    // Priming samples and a wrong time base both show up as a short track.
    let Some(fx) = Fixture::new("duration") else {
        return;
    };
    let Some(source) = fx.with_audio("aac", &[]) else {
        return;
    };

    let out = fx.out("out.m4a");
    audio::encode(&source, &out, &AudioSpec::default())
        .expect("encode")
        .expect("track");

    let produced = ferrite_av::probe(&out).expect("probe");
    assert!(
        (2_800..=3_200).contains(&produced.duration_ms),
        "3s of audio came out {}ms",
        produced.duration_ms
    );
}

#[test]
fn a_source_that_is_not_media_is_an_error() {
    let Some(fx) = Fixture::new("junk") else {
        return;
    };
    let junk = fx.out("junk.mp4");
    std::fs::write(&junk, b"not media").unwrap();
    assert!(audio::encode(&junk, &fx.out("out.m4a"), &AudioSpec::default()).is_err());
}
