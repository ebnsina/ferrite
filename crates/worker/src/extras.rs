//! Extra deliverables beyond the adaptive package: a progressive MP4 download
//! (optionally watermarked) and an audio-only track. Best-effort — a failure
//! here logs but never fails the transcode.

use std::path::Path;

use ferrite_stream_core::{Artifact, ArtifactKind, TranscodeJob, Watermark};
use ferrite_stream_storage::Storage;
use tokio::process::Command;

/// Generate the requested extra outputs and return their artifacts.
pub async fn generate(
    job: &TranscodeJob,
    source: &str,
    dir: &Path,
    storage: &Storage,
) -> Vec<Artifact> {
    let mut out = Vec::new();

    if job.mp4 {
        match mp4(job, source, dir, storage).await {
            Ok(a) => out.push(a),
            Err(e) => tracing::warn!(job = %job.id, error = %e, "mp4 download failed"),
        }
    }
    if job.audio {
        match audio(job, source, dir).await {
            Ok(a) => out.push(a),
            Err(e) => tracing::warn!(job = %job.id, error = %e, "audio-only export failed"),
        }
    }
    out
}

async fn mp4(
    job: &TranscodeJob,
    source: &str,
    dir: &Path,
    storage: &Storage,
) -> Result<Artifact, String> {
    let out = dir.join("download.mp4");
    let out_str = out.to_string_lossy().to_string();

    let mut args: Vec<String> = ["-y", "-loglevel", "error", "-i", source]
        .into_iter()
        .map(String::from)
        .collect();

    // Cap to 1080p, never upscale.
    let base_scale = "scale=-2:'min(1080,ih)'";

    if let Some(wm) = &job.watermark {
        let logo = dir.join("logo.input");
        let logo_str = logo.to_string_lossy().to_string();
        if storage.get_file(&wm.logo_key, &logo_str).await.is_ok() {
            args.push("-i".into());
            args.push(logo_str);
            args.push("-filter_complex".into());
            args.push(watermark_filter(wm, base_scale));
        } else {
            tracing::warn!(job = %job.id, "watermark logo missing; skipping overlay");
            args.push("-vf".into());
            args.push(base_scale.into());
        }
    } else {
        args.push("-vf".into());
        args.push(base_scale.into());
    }

    args.extend(
        [
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "21",
            "-c:a",
            "aac",
            "-b:a",
            "160k",
            "-movflags",
            "+faststart",
        ]
        .map(String::from),
    );
    args.push(out_str.clone());

    run(&args).await?;
    let bytes = tokio::fs::metadata(&out)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(Artifact {
        kind: ArtifactKind::DashSegment, // generic binary artifact
        local_path: out_str,
        key: format!("{}/download.mp4", job.output_prefix),
        rendition: None,
        bytes,
    })
}

/// filter_complex: fade the logo to the given opacity, scale the base, overlay.
fn watermark_filter(wm: &Watermark, base_scale: &str) -> String {
    let opacity = wm.opacity.clamp(0.0, 1.0);
    let m = 24; // margin in px
    let pos = match wm.position.as_str() {
        "tl" => format!("{m}:{m}"),
        "tr" => format!("W-w-{m}:{m}"),
        "bl" => format!("{m}:H-h-{m}"),
        _ => format!("W-w-{m}:H-h-{m}"), // br (default)
    };
    format!(
        "[1:v]format=rgba,colorchannelmixer=aa={opacity}[wm];\
         [0:v]{base_scale}[base];[base][wm]overlay={pos}"
    )
}

async fn audio(job: &TranscodeJob, source: &str, dir: &Path) -> Result<Artifact, String> {
    let out = dir.join("audio.m4a");
    let out_str = out.to_string_lossy().to_string();
    let args = [
        "-y",
        "-loglevel",
        "error",
        "-i",
        source,
        "-vn",
        "-c:a",
        "aac",
        "-b:a",
        "192k",
        "-movflags",
        "+faststart",
        &out_str,
    ]
    .map(String::from);
    run(&args).await?;
    let bytes = tokio::fs::metadata(&out)
        .await
        .map(|m| m.len())
        .unwrap_or(0);
    Ok(Artifact {
        kind: ArtifactKind::DashSegment,
        local_path: out_str,
        key: format!("{}/audio.m4a", job.output_prefix),
        rendition: None,
        bytes,
    })
}

async fn run(args: &[String]) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args(args)
        .status()
        .await
        .map_err(|e| format!("spawn ffmpeg: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited with {status}"))
    }
}
