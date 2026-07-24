//! Poster frame + sprite sheet + WebVTT storyboard for scrubbing previews.

use std::path::Path;

use ferrite_stream_core::{Artifact, ArtifactKind, MediaInfo, TranscodeError, TranscodeJob};
use tokio::process::Command;

const THUMB_W: u32 = 160;
const COLS: u32 = 10;
const MAX_THUMBS: u32 = 100;

/// Generate poster.jpg, sprite.jpg and thumbs.vtt into `dir`, returning them as
/// uploadable artifacts keyed under `{output_prefix}/thumbs/`.
pub async fn generate(
    job: &TranscodeJob,
    media: &MediaInfo,
    source: &str,
    dir: &Path,
) -> Result<Vec<Artifact>, TranscodeError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(TranscodeError::Io)?;
    let mut artifacts = Vec::new();

    // Poster: a frame from the mid-point (fast seek before input).
    let poster = dir.join("poster.jpg");
    let mid = (media.duration_secs / 2.0).max(0.0);
    run_ffmpeg(&[
        "-ss",
        &format!("{mid:.3}"),
        "-i",
        source,
        "-frames:v",
        "1",
        "-q:v",
        "3",
        &poster.to_string_lossy(),
    ])
    .await?;
    artifacts.push(artifact(
        job,
        ArtifactKind::Thumbnail,
        &poster,
        "thumbs/poster.jpg",
    ));

    // Duration unknown or tiny → poster only, no storyboard.
    if media.duration_secs < 1.0 || media.height == 0 {
        return Ok(artifacts);
    }

    let interval = ((media.duration_secs / MAX_THUMBS as f64).ceil() as u32).max(1);
    let count = ((media.duration_secs / interval as f64).ceil() as u32).max(1);
    let rows = count.div_ceil(COLS);
    let thumb_h = even(THUMB_W as f64 * media.height as f64 / media.width as f64);

    // Sprite: sampled frames tiled into one grid image.
    let sprite = dir.join("sprite.jpg");
    run_ffmpeg(&[
        "-i",
        source,
        "-vf",
        &format!("fps=1/{interval},scale={THUMB_W}:{thumb_h},tile={COLS}x{rows}"),
        "-frames:v",
        "1",
        "-q:v",
        "4",
        &sprite.to_string_lossy(),
    ])
    .await?;
    artifacts.push(artifact(
        job,
        ArtifactKind::Sprite,
        &sprite,
        "thumbs/sprite.jpg",
    ));

    // WebVTT mapping each interval to a region of the sprite.
    let vtt_path = dir.join("thumbs.vtt");
    let vtt = build_vtt(count, interval, media.duration_secs, thumb_h);
    tokio::fs::write(&vtt_path, vtt)
        .await
        .map_err(TranscodeError::Io)?;
    artifacts.push(artifact(
        job,
        ArtifactKind::Vtt,
        &vtt_path,
        "thumbs/thumbs.vtt",
    ));

    Ok(artifacts)
}

fn build_vtt(count: u32, interval: u32, duration: f64, thumb_h: u32) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for i in 0..count {
        let start = (i * interval) as f64;
        let end = ((i + 1) * interval) as f64;
        let end = end.min(duration);
        let x = (i % COLS) * THUMB_W;
        let y = (i / COLS) * thumb_h;
        out.push_str(&format!(
            "{} --> {}\nsprite.jpg#xywh={x},{y},{THUMB_W},{thumb_h}\n\n",
            timecode(start),
            timecode(end)
        ));
    }
    out
}

fn artifact(job: &TranscodeJob, kind: ArtifactKind, path: &Path, suffix: &str) -> Artifact {
    let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Artifact {
        kind,
        local_path: path.to_string_lossy().to_string(),
        key: format!("{}/{}", job.output_prefix, suffix),
        rendition: None,
        bytes,
    }
}

async fn run_ffmpeg(args: &[&str]) -> Result<(), TranscodeError> {
    let output = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-nostats"])
        .args(args)
        .output()
        .await
        .map_err(|e| TranscodeError::Ffmpeg(format!("failed to spawn ffmpeg: {e}")))?;
    if !output.status.success() {
        return Err(TranscodeError::Ffmpeg(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

/// `HH:MM:SS.mmm` WebVTT timestamp.
fn timecode(secs: f64) -> String {
    let ms = (secs * 1000.0).round() as u64;
    let (h, m, s, milli) = (
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60,
        ms % 1000,
    );
    format!("{h:02}:{m:02}:{s:02}.{milli:03}")
}

fn even(v: f64) -> u32 {
    let n = v.round() as u32;
    n - (n % 2)
}
