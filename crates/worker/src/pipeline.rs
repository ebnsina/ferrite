//! Single-job pipeline: download → probe → transcode → upload, persisting state.

use std::path::PathBuf;

use ferrite_core::{Encoder, TranscodeJob};
use ferrite_storage::Storage;
use sqlx::PgPool;

use crate::cmaf;
use crate::cpu_encoder::CpuEncoder;
use crate::db;
use crate::encoding::EncodeParams;
use crate::thumbnails;

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("storage error: {0}")]
    Storage(#[from] ferrite_storage::StorageError),
    #[error("transcode error: {0}")]
    Transcode(#[from] ferrite_core::TranscodeError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("db error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("clip error: {0}")]
    Clip(String),
}

/// Process one job end to end. Returns the number of artifacts uploaded.
pub async fn process(
    pool: &PgPool,
    job: &TranscodeJob,
    storage: &Storage,
    work_root: &str,
    encode: EncodeParams,
) -> Result<usize, PipelineError> {
    let job_dir = PathBuf::from(work_root).join(job.id.to_string());
    tokio::fs::create_dir_all(&job_dir).await?;

    let result = run(pool, job, storage, &job_dir, encode).await;
    if let Err(e) = tokio::fs::remove_dir_all(&job_dir).await {
        tracing::warn!(job = %job.id, error = %e, "failed to clean scratch dir");
    }
    result
}

async fn run(
    pool: &PgPool,
    job: &TranscodeJob,
    storage: &Storage,
    job_dir: &PathBuf,
    encode: EncodeParams,
) -> Result<usize, PipelineError> {
    // Clip jobs trim the source into a new asset instead of transcoding.
    if let Some(clip) = job.clip.clone() {
        let res = crate::clip::run(pool, job, &clip, storage, job_dir).await;
        if res.is_err() {
            let _ = db::set_asset_status(pool, clip.dest_asset_id, "error").await;
        }
        return res;
    }

    let source_path = job_dir.join("source.input");
    let source_str = source_path.to_string_lossy().to_string();

    db::mark_started(pool, job.id, "probing").await?;
    tracing::info!(job = %job.id, key = %job.source_key, "downloading source");
    storage.get_file(&job.source_key, &source_str).await?;

    let output_dir = job_dir.join("output");
    let encoder = CpuEncoder::new(&output_dir, encode);
    let media = encoder.probe(&source_str).await?;
    tracing::info!(job = %job.id, w = media.width, h = media.height, "probed");

    // Cap the ladder to the source resolution — never upscale.
    let mut job = job.clone();
    job.ladder = job.ladder.cap_to_source(&media);

    // For encrypted HLS: mint a per-job AES-128 key, persist it for the key
    // endpoint, and hand it to the encoder. The key never touches object storage.
    if job.encrypt {
        let mut key = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::rng(), &mut key);
        db::set_encryption_key(pool, job.id, &key).await?;
        job.encryption_key = Some(hex::encode(key));
    }
    let job = &job;

    db::set_state(pool, job.id, "transcoding").await?;
    tracing::info!(job = %job.id, renditions = job.ladder.renditions.len(), encrypt = job.encrypt, "transcoding");

    // Encrypted jobs use TS HLS + AES-128 (fMP4 encryption is CENC — future work);
    // everything else uses a single CMAF pass shared by HLS + DASH.
    let mut artifacts = if job.encrypt {
        // The encoder reads the source from inside its output dir.
        tokio::fs::create_dir_all(&output_dir).await?;
        tokio::fs::copy(&source_path, output_dir.join("source.input")).await?;

        // Bridge the encoder's sync progress callback to throttled DB writes.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<f32>();
        let progress_task = {
            let pool = pool.clone();
            let job_id = job.id;
            tokio::spawn(async move {
                let mut last = 0.0;
                while let Some(p) = rx.recv().await {
                    if p - last >= 0.02 || p >= 1.0 {
                        let _ = db::set_progress(&pool, job_id, p).await;
                        last = p;
                    }
                }
            })
        };
        let progress = move |_rendition: &str, pct: f32| {
            let _ = tx.send(pct);
        };
        let a = encoder.transcode(job, &media, &progress).await?;
        drop(progress);
        let _ = progress_task.await;
        a
    } else {
        cmaf::generate(job, &media, &source_str, &output_dir, encode).await?
    };

    // Optional poster + sprite + VTT storyboard. Non-essential: failure logs
    // but does not fail the transcode.
    if job.thumbnails {
        db::set_state(pool, job.id, "packaging").await?;
        let thumb_dir = output_dir.join("thumbs");
        match thumbnails::generate(job, &media, &source_str, &thumb_dir).await {
            Ok(mut thumbs) => artifacts.append(&mut thumbs),
            Err(e) => tracing::warn!(job = %job.id, error = %e, "thumbnail generation failed"),
        }
    }

    db::set_state(pool, job.id, "uploading").await?;
    tracing::info!(job = %job.id, count = artifacts.len(), "uploading artifacts");
    for artifact in &artifacts {
        storage
            .put_file(&artifact.key, &artifact.local_path)
            .await?;
    }

    // CMAF's ffmpeg emits master.m3u8 + manifest.mpd; the TS (encrypted) path
    // needs the hand-built HLS master.
    if job.encrypt {
        upload_master_playlist(job, &media, job_dir, storage).await?;
    }
    record_renditions(pool, job).await?;

    // Meter usage: source minutes × rendition count (billed output minutes).
    let minutes = (media.duration_secs / 60.0) * job.ladder.renditions.len().max(1) as f64;
    if let Err(e) = db::record_usage(pool, job.tenant_id, minutes).await {
        tracing::warn!(job = %job.id, error = %e, "failed to record usage");
    }

    Ok(artifacts.len())
}

/// Build and upload the HLS master playlist that ties the renditions into an
/// adaptive ladder, at `{output_prefix}/master.m3u8`.
async fn upload_master_playlist(
    job: &TranscodeJob,
    media: &ferrite_core::MediaInfo,
    job_dir: &PathBuf,
    storage: &Storage,
) -> Result<(), PipelineError> {
    let mut m3u8 = String::from("#EXTM3U\n#EXT-X-VERSION:3\n");
    for r in &job.ladder.renditions {
        let mut width = (r.height as f64 * media.width as f64 / media.height as f64).round() as u32;
        width -= width % 2; // H.264 needs even dimensions
        let bandwidth = (r.bitrate_kbps + 128) * 1000; // video + ~audio
        m3u8.push_str(&format!(
            "#EXT-X-STREAM-INF:BANDWIDTH={bandwidth},RESOLUTION={width}x{}\n{}/index.m3u8\n",
            r.height, r.name
        ));
    }

    let path = job_dir.join("master.m3u8");
    tokio::fs::write(&path, m3u8).await?;
    let key = format!("{}/master.m3u8", job.output_prefix);
    storage.put_file(&key, &path.to_string_lossy()).await?;
    Ok(())
}

/// One DB row per HLS rendition produced.
async fn record_renditions(pool: &PgPool, job: &TranscodeJob) -> Result<(), sqlx::Error> {
    // Playlists live under the master; the exact child path differs between the
    // TS and CMAF layouts, so rows point at the master as the entrypoint.
    let master = format!("{}/master.m3u8", job.output_prefix);
    for r in &job.ladder.renditions {
        let prefix = format!("{}/{}", job.output_prefix, r.name);
        db::insert_rendition(
            pool,
            job.id,
            "hls",
            &r.name,
            r.height as i32,
            r.bitrate_kbps as i32,
            &master,
            &prefix,
        )
        .await?;
    }
    Ok(())
}
