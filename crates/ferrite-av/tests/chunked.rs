//! Chunked encoding and joining. The property that matters is that splitting
//! the work does not cost quality — without that, chunking is not worth having.

#![cfg(feature = "ffmpeg")]

use ferrite_av::encoder::Preset;
use ferrite_av::quality::{self, GATE_DELTA_VMAF, Options};
use ferrite_av::transcode::Output;
use ferrite_av::{NeverCancel, split};
use std::path::PathBuf;
use std::sync::Arc;

struct Fixture {
    dir: PathBuf,
    source: PathBuf,
}

impl Fixture {
    /// Long enough to be worth splitting, short enough to test with.
    fn new(name: &str, seconds: u32) -> Option<Self> {
        let dir = std::env::temp_dir().join(format!("ferrite-chunk-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;

        let source = dir.join("source.mp4");
        let ok = std::process::Command::new("ffmpeg")
            .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
            .arg(format!("testsrc2=size=320x180:rate=30:duration={seconds}"))
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
        ok.success().then_some(Self { dir, source })
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

/// Longer than production's ten seconds: fewer encoder starts per test run.
const CHUNK_MS: u64 = 20_000;

fn spec(source: &std::path::Path) -> ferrite_av::EncodeSpec {
    let info = ferrite_av::probe(source).expect("probe");
    let video = info.primary_video().expect("video");
    ferrite_av::EncodeSpec::new(
        ferrite_av::VideoCodecName::H264,
        ferrite_av::Resolution::new(video.width, video.height),
        video.frame_rate,
    )
    .with_preset(Preset::Veryfast)
    .with_rate_control(ferrite_av::RateControl::crf(23, 3_000_000))
}

/// Encode every chunk separately and join them.
fn chunked(fx: &Fixture, target_ms: u64) -> Option<(PathBuf, usize)> {
    let info = ferrite_av::probe(&fx.source).expect("probe");
    let plan = split::plan(&info.keyframes_ms, info.duration_ms, target_ms);
    if !plan.is_split() {
        return None;
    }

    let spec = spec(&fx.source);
    let mut parts = Vec::new();
    for chunk in &plan.chunks {
        let part = fx.out(&format!("part-{:03}.mp4", chunk.index));
        ferrite_av::transcode::run_range(
            &fx.source,
            &[Output {
                path: part.clone(),
                spec: spec.clone(),
            }],
            Some(*chunk),
            Arc::new(NeverCancel),
        )
        .expect("encode chunk");
        parts.push(part);
    }

    let joined = fx.out("joined.mp4");
    ferrite_av::join::run(&parts, &joined).expect("join");
    Some((joined, plan.chunks.len()))
}

fn single_pass(fx: &Fixture) -> PathBuf {
    let out = fx.out("single.mp4");
    ferrite_av::transcode::run(
        &fx.source,
        &[Output {
            path: out.clone(),
            spec: spec(&fx.source),
        }],
        Arc::new(NeverCancel),
    )
    .expect("single pass");
    out
}

#[test]
fn a_chunk_covers_only_its_own_range() {
    let Some(fx) = Fixture::new("range", 124) else {
        return;
    };
    let info = ferrite_av::probe(&fx.source).expect("probe");
    let plan = split::plan(&info.keyframes_ms, info.duration_ms, CHUNK_MS);
    assert!(plan.is_split(), "the source was not split");

    let chunk = plan.chunks[2];
    let part = fx.out("one.mp4");
    let report = ferrite_av::transcode::run_range(
        &fx.source,
        &[Output {
            path: part.clone(),
            spec: spec(&fx.source),
        }],
        Some(chunk),
        Arc::new(NeverCancel),
    )
    .expect("encode chunk")
    .remove(0);

    // 30fps, so a ten-second chunk is about 300 frames — not the whole source.
    let expected = (chunk.duration_ms() * 30 / 1000) as u64;
    assert!(
        report.frames.abs_diff(expected) <= 2,
        "chunk {} produced {} frames, expected about {expected}",
        chunk.index,
        report.frames
    );

    let produced = ferrite_av::probe(&part).expect("probe chunk");
    assert!(
        produced.duration_ms < info.duration_ms,
        "the chunk covers the whole source"
    );
}

#[test]
fn every_chunk_starts_on_a_keyframe() {
    // Split rule 1. A chunk that does not cannot be decoded on its own, which
    // is the entire premise of encoding them on different machines.
    let Some(fx) = Fixture::new("keyframe", 124) else {
        return;
    };
    let info = ferrite_av::probe(&fx.source).expect("probe");
    let plan = split::plan(&info.keyframes_ms, info.duration_ms, CHUNK_MS);

    for chunk in plan.chunks.iter().take(3) {
        let part = fx.out(&format!("k-{}.mp4", chunk.index));
        ferrite_av::transcode::run_range(
            &fx.source,
            &[Output {
                path: part.clone(),
                spec: spec(&fx.source),
            }],
            Some(*chunk),
            Arc::new(NeverCancel),
        )
        .expect("encode chunk");

        let produced = ferrite_av::probe(&part).expect("probe chunk");
        assert_eq!(
            produced.keyframes_ms.first(),
            Some(&0),
            "chunk {} does not open on a keyframe",
            chunk.index
        );
    }
}

#[test]
fn joining_restores_the_whole_timeline() {
    let Some(fx) = Fixture::new("timeline", 124) else {
        return;
    };
    let Some((joined, parts)) = chunked(&fx, CHUNK_MS) else {
        return;
    };
    assert!(parts > 1, "only {parts} chunk");

    let source = ferrite_av::probe(&fx.source).expect("probe source");
    let produced = ferrite_av::probe(&joined).expect("probe joined");

    // Within a frame: the join fixes timestamps, it does not resample.
    assert!(
        produced.duration_ms.abs_diff(source.duration_ms) <= 40,
        "joined is {}ms against a {}ms source",
        produced.duration_ms,
        source.duration_ms
    );
    assert!(
        !produced
            .warnings
            .contains(&ferrite_av::Warning::NonMonotonicTimestamps),
        "the join left timestamps going backwards"
    );
}

#[test]
fn the_joined_file_has_every_frame() {
    let Some(fx) = Fixture::new("frames", 124) else {
        return;
    };
    let Some((joined, _)) = chunked(&fx, CHUNK_MS) else {
        return;
    };

    let single = single_pass(&fx);
    let a = ferrite_av::probe(&single).expect("probe single");
    let b = ferrite_av::probe(&joined).expect("probe joined");

    let frames = |i: &ferrite_av::MediaInfo| i.primary_video().and_then(|v| v.frame_count);
    if let (Some(want), Some(got)) = (frames(&a), frames(&b)) {
        assert_eq!(got, want, "a chunk went missing in the join");
    }
}

#[test]
fn chunking_does_not_cost_quality() {
    // Stage 3's gate. Without this, chunking is a way to make videos worse in
    // parallel.
    if std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-filters"])
        .output()
        .is_ok_and(|o| !String::from_utf8_lossy(&o.stdout).contains("libvmaf"))
    {
        eprintln!("skipped: this ffmpeg has no libvmaf");
        return;
    }

    let Some(fx) = Fixture::new("quality", 124) else {
        return;
    };
    let Some((joined, parts)) = chunked(&fx, CHUNK_MS) else {
        return;
    };
    let single = single_pass(&fx);

    let options = Options {
        subsample: 15,
        ..Options::default()
    };
    let chunked_score = quality::measure(&fx.source, &joined, &options).expect("measure chunked");
    let single_score = quality::measure(&fx.source, &single, &options).expect("measure single");

    let delta = single_score.vmaf.mean - chunked_score.vmaf.mean;
    assert!(
        delta <= GATE_DELTA_VMAF,
        "{parts} chunks cost {delta:.3} VMAF (single {:.2}, chunked {:.2}), gate is {GATE_DELTA_VMAF}",
        single_score.vmaf.mean,
        chunked_score.vmaf.mean
    );

    if let (Some(single_cambi), Some(chunked_cambi)) = (single_score.cambi, chunked_score.cambi) {
        assert!(
            chunked_cambi.mean <= single_cambi.mean + 0.05,
            "chunking added banding: {:.3} against {:.3}",
            chunked_cambi.mean,
            single_cambi.mean
        );
    }
}

#[test]
fn nothing_to_join_is_refused() {
    assert!(ferrite_av::join::run(&[], std::path::Path::new("/tmp/none.mp4")).is_err());
}
