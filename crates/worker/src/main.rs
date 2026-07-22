//! Ferrite transcode worker: claims jobs from the queue and processes them.

mod config;
mod cpu_encoder;
mod pipeline;

use std::time::Duration;

use anyhow::Context;
use ferrite_queue::{JobQueue, RedisQueue};
use ferrite_storage::{Storage, StorageConfig};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Settings;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let settings = Settings::load().context("failed to load worker configuration")?;
    tracing::info!(consumer = %settings.consumer_name, "starting ferrite-worker");

    let storage = Storage::connect(StorageConfig {
        bucket: settings.s3_bucket.clone(),
        endpoint_url: settings.s3_endpoint_url.clone(),
        force_path_style: settings.s3_force_path_style,
    })
    .await
    .context("failed to initialize object storage client")?;

    let queue = RedisQueue::connect(&settings.redis_url, &settings.queue_group)
        .await
        .with_context(|| {
            format!(
                "could not connect to Redis at {} — is Redis running (docker compose up)?",
                settings.redis_url
            )
        })?;

    run_loop(settings, storage, queue).await
}

/// Claim → process → ack. Failures are retried by redelivery; jobs that exceed
/// the attempt budget are dead-lettered so one poison input can't wedge a worker.
async fn run_loop(settings: Settings, storage: Storage, queue: RedisQueue) -> anyhow::Result<()> {
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);

    loop {
        tokio::select! {
            _ = &mut shutdown => {
                tracing::info!("shutdown signal received, stopping claim loop");
                return Ok(());
            }
            claimed = queue.claim(&settings.consumer_name, Duration::from_secs(5)) => {
                match claimed {
                    Ok(Some(claimed)) => {
                        let job = claimed.job.clone();
                        tracing::info!(job = %job.id, tenant = %job.tenant_id, "claimed job");

                        match pipeline::process(&job, &storage, &settings.work_dir).await {
                            Ok(count) => {
                                tracing::info!(job = %job.id, artifacts = count, "job completed");
                                if let Err(e) = queue.ack(&claimed.id).await {
                                    tracing::error!(job = %job.id, error = %e, "failed to ack job");
                                }
                            }
                            Err(e) => {
                                handle_failure(&queue, &claimed.id, &job, claimed.delivery_count, settings.max_attempts, e).await;
                            }
                        }
                    }
                    Ok(None) => { /* idle: no jobs within the block window */ }
                    Err(e) => {
                        tracing::error!(error = %e, "queue claim failed; backing off");
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }
    }
}

async fn handle_failure(
    queue: &RedisQueue,
    entry_id: &str,
    job: &ferrite_core::TranscodeJob,
    delivery_count: usize,
    max_attempts: usize,
    error: pipeline::PipelineError,
) {
    if delivery_count >= max_attempts {
        tracing::error!(
            job = %job.id,
            attempts = delivery_count,
            error = %error,
            "job exceeded retry budget; dead-lettering"
        );
        if let Err(e) = queue.dead_letter(entry_id, job).await {
            tracing::error!(job = %job.id, error = %e, "failed to dead-letter job");
        }
    } else {
        // Not acking leaves the entry pending for redelivery/reclaim.
        tracing::warn!(
            job = %job.id,
            attempt = delivery_count,
            error = %error,
            "job failed; will be retried"
        );
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ferrite_worker=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
