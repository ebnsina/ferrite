//! Packaging to CMAF. Skips unless the packager binary is present; run
//! scripts/fetch-packager.sh to get it.

#![cfg(feature = "ffmpeg")]

use ferrite_av::NeverCancel;
use ferrite_av::encoder::Preset;
use ferrite_av::package::{self, Input, Track};
use ferrite_av::transcode::{self, Output};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Fixture {
    dir: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Encode a small ladder, then package it. `None` if the tools are missing.
fn packaged(name: &str) -> Option<(Fixture, package::Packaged, Vec<String>)> {
    if !package::binary().is_file() && which_packager().is_none() {
        eprintln!("skipped: no packager — run scripts/fetch-packager.sh");
        return None;
    }

    let dir = std::env::temp_dir().join(format!("ferrite-package-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).ok()?;
    let fx = Fixture { dir: dir.clone() };

    let source = dir.join("source.mp4");
    let made = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg("testsrc2=size=640x360:rate=30:duration=6")
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=6"])
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
        .args(["-c:a", "aac", "-shortest"])
        .arg(&source)
        .status()
        .ok()?;
    if !made.success() {
        return None;
    }

    let info = ferrite_av::probe(&source).ok()?;
    let video = info.primary_video()?;
    let steps = ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, Preset::Ultrafast);

    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: dir.join(format!("{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();
    transcode::run(&source, &outputs, Arc::new(NeverCancel)).ok()?;

    let mut inputs: Vec<Input> = steps
        .iter()
        .zip(&outputs)
        .map(|(s, o)| Input {
            name: s.name.to_string(),
            path: o.path.clone(),
            kind: Track::Video,
        })
        .collect();
    inputs.push(Input {
        name: "audio".into(),
        path: source,
        kind: Track::Audio,
    });

    let out = package::run(&inputs, &dir.join("cmaf")).ok()?;
    let names = steps.iter().map(|s| s.name.to_string()).collect();
    Some((fx, out, names))
}

fn which_packager() -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join("packager"))
            .find(|p| p.is_file())
    })
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn one_segment_set_carries_both_manifests() {
    // CMAF is the whole reason: separate HLS and DASH segment sets roughly
    // double the stored bytes.
    let Some((_fx, out, names)) = packaged("cmaf") else {
        return;
    };

    assert!(out.hls.is_file(), "no HLS master");
    assert!(out.dash.is_file(), "no DASH manifest");

    let mpd = read(&out.dash);
    let master = read(&out.hls);

    for name in &names {
        let dir = out.hls.parent().unwrap().join(name);
        assert!(dir.join("init.mp4").is_file(), "{name} has no init segment");

        let segments: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".m4s"))
            .collect();
        assert!(!segments.is_empty(), "{name} has no segments");

        // Both manifests point at the same files.
        assert!(
            mpd.contains(&format!("{name}/init.mp4")),
            "MPD misses {name}"
        );
        assert!(
            master.contains(&format!("{name}/video.m3u8")),
            "master misses {name}"
        );
    }
}

#[test]
fn the_dash_manifest_describes_a_finished_file_not_a_live_stream() {
    let Some((_fx, out, _)) = packaged("static") else {
        return;
    };
    let mpd = read(&out.dash);

    assert!(
        mpd.contains(r#"type="static""#),
        "VOD was published as a live manifest"
    );
    assert!(
        mpd.contains("mediaPresentationDuration"),
        "no duration in the MPD"
    );
    assert!(
        !mpd.contains("timeShiftBufferDepth"),
        "a finished file has no time-shift buffer"
    );
}

#[test]
fn every_variant_playlist_is_vod_with_an_init_segment() {
    let Some((_fx, out, names)) = packaged("playlists") else {
        return;
    };
    let root = out.hls.parent().unwrap();

    for name in &names {
        let playlist = read(&root.join(name).join("video.m3u8"));
        assert!(
            playlist.contains("#EXT-X-PLAYLIST-TYPE:VOD"),
            "{name} is not VOD"
        );
        assert!(
            playlist.contains("#EXT-X-MAP:URI=\"init.mp4\""),
            "{name} has no init map"
        );
        assert!(playlist.contains("#EXT-X-ENDLIST"), "{name} never ends");
    }
}

#[test]
fn segment_boundaries_line_up_across_every_rung() {
    // A player switching rungs mid-stream needs the same cut points, or it
    // stutters. This is split rule 2 surviving all the way to the segments.
    let Some((_fx, out, names)) = packaged("aligned") else {
        return;
    };
    let root = out.hls.parent().unwrap();

    let counts: Vec<usize> = names
        .iter()
        .map(|name| {
            read(&root.join(name).join("video.m3u8"))
                .matches("#EXTINF:")
                .count()
        })
        .collect();
    assert!(
        counts.windows(2).all(|w| w[0] == w[1]),
        "segment counts differ: {counts:?}"
    );

    let durations: Vec<Vec<String>> = names
        .iter()
        .map(|name| {
            read(&root.join(name).join("video.m3u8"))
                .lines()
                .filter(|l| l.starts_with("#EXTINF:"))
                .map(str::to_string)
                .collect()
        })
        .collect();
    assert!(
        durations.windows(2).all(|w| w[0] == w[1]),
        "segment durations differ"
    );
}

#[test]
fn a_packaged_rendition_still_plays() {
    let Some((_fx, out, names)) = packaged("plays") else {
        return;
    };
    let playlist = out.hls.parent().unwrap().join(&names[0]).join("video.m3u8");

    // One variant, not the master: ffmpeg's HLS demuxer opens every variant at
    // once and interleaves them, which looks like corruption and is not.
    let decoded = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-allowed_extensions", "ALL", "-i"])
        .arg(&playlist)
        .args(["-f", "null", "-"])
        .output()
        .expect("run ffmpeg");

    let complaints = String::from_utf8_lossy(&decoded.stderr);
    assert!(decoded.status.success(), "{} did not decode", names[0]);
    assert!(
        complaints.trim().is_empty(),
        "decoding {} complained: {complaints}",
        names[0]
    );
}

#[test]
fn nothing_to_package_is_refused() {
    let err = package::run(&[], Path::new("/tmp/ferrite-nothing")).unwrap_err();
    assert!(matches!(err, ferrite_av::AvError::InvalidSpec(_)), "{err}");
}
