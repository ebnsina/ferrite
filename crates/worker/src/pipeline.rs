//! Single-job processing pipeline: download → probe → transcode → upload.
//!
//! Kept independent of the queue so it can be unit-tested and reused by a
//! future live path.

use std::path::PathBuf;

use ferrite_core::{Encoder, TranscodeJob};
use ferrite_storage::Storage;

use crate::cpu_encoder::CpuEncoder;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("storage error: {0}")]
    Storage(#[from] ferrite_storage::StorageError),
    #[error("transcode error: {0}")]
    Transcode(#[from] ferrite_core::TranscodeError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Process one job end to end. Returns the number of artifacts uploaded.
pub async fn process(
    job: &TranscodeJob,
    storage: &Storage,
    work_root: &str,
) -> Result<usize, PipelineError> {
    let job_dir = PathBuf::from(work_root).join(job.id.to_string());
    tokio::fs::create_dir_all(&job_dir).await?;

    // Ensure scratch is cleaned up regardless of outcome.
    let result = run(job, storage, &job_dir).await;
    if let Err(e) = tokio::fs::remove_dir_all(&job_dir).await {
        tracing::warn!(job = %job.id, error = %e, "failed to clean scratch dir");
    }
    result
}

async fn run(
    job: &TranscodeJob,
    storage: &Storage,
    job_dir: &PathBuf,
) -> Result<usize, PipelineError> {
    let source_path = job_dir.join("source.input");
    let source_str = source_path.to_string_lossy().to_string();

    tracing::info!(job = %job.id, key = %job.source_key, "downloading source");
    storage.get_file(&job.source_key, &source_str).await?;

    let output_dir = job_dir.join("output");
    let encoder = CpuEncoder::new(&output_dir);

    tracing::info!(job = %job.id, "probing source");
    let media = encoder.probe(&source_str).await?;
    tracing::info!(
        job = %job.id,
        width = media.width,
        height = media.height,
        duration = media.duration_secs,
        "probed"
    );

    // The encoder expects the source inside its output dir; copy it in.
    tokio::fs::create_dir_all(&output_dir).await?;
    tokio::fs::copy(&source_path, output_dir.join("source.input")).await?;

    let progress = |rendition: &str, pct: f32| {
        tracing::debug!(job = %job.id, rendition, pct, "progress");
    };

    tracing::info!(job = %job.id, renditions = job.ladder.renditions.len(), "transcoding");
    let artifacts = encoder.transcode(job, &media, &progress).await?;

    tracing::info!(job = %job.id, count = artifacts.len(), "uploading artifacts");
    for artifact in &artifacts {
        storage
            .put_file(&artifact.key, &artifact.local_path)
            .await?;
    }

    Ok(artifacts.len())
}
