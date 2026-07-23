//! Clip/trim: cut a time range from a source asset into a new MP4 asset.
//! Re-encodes (H.264/AAC + faststart) so the cut is frame-accurate and the
//! result is a clean, transcodable source.

use std::path::Path;

use ferrite_core::{Clip, TranscodeJob};
use ferrite_storage::Storage;
use sqlx::PgPool;
use tokio::process::Command;

use crate::db;
use crate::pipeline::PipelineError;

pub async fn run(
    pool: &PgPool,
    job: &TranscodeJob,
    clip: &Clip,
    storage: &Storage,
    job_dir: &Path,
    provenance: Option<&str>,
) -> Result<usize, PipelineError> {
    db::mark_started(pool, job.id, "transcoding").await?;

    let source = job_dir.join("source.input");
    let source_str = source.to_string_lossy().to_string();
    tracing::info!(job = %job.id, key = %job.source_key, "clip: downloading source");
    storage.get_file(&job.source_key, &source_str).await?;

    let out = job_dir.join("clip.mp4");
    let out_str = out.to_string_lossy().to_string();
    let duration = (clip.end_secs - clip.start_secs).max(0.1);

    db::set_progress(pool, job.id, 0.4).await.ok();
    // -ss before -i is a fast seek; re-encoding makes it frame-accurate. -t is
    // the clip duration relative to the seek point.
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-ss",
            &format!("{:.3}", clip.start_secs),
            "-i",
            &source_str,
            "-t",
            &format!("{:.3}", duration),
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-c:a",
            "aac",
            "-movflags",
            "+faststart",
            &out_str,
        ])
        .status()
        .await?;

    if !status.success() {
        return Err(PipelineError::Clip(format!("ffmpeg exited with {status}")));
    }

    db::set_state(pool, job.id, "uploading").await.ok();
    tracing::info!(job = %job.id, key = %clip.dest_key, "clip: uploading result");
    storage.put_file(&clip.dest_key, &out_str).await?;

    let bytes = tokio::fs::metadata(&out).await.map(|m| m.len() as i64).ok();
    db::mark_asset_ready(pool, clip.dest_asset_id, bytes).await?;

    let filename = clip.dest_key.rsplit('/').next().unwrap_or("clip.mp4");
    crate::provenance::record(
        pool,
        provenance,
        job.tenant_id,
        clip.dest_asset_id,
        filename,
        "clip",
        Some(job.asset_id),
        &out,
    )
    .await;

    Ok(1)
}
