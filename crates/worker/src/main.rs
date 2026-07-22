//! Ferrite transcode worker: claims jobs from the queue and processes them.

mod cmaf;
mod config;
mod cpu_encoder;
mod db;
mod pipeline;
mod thumbnails;
mod webhooks;

use std::time::Duration;

use anyhow::Context;
use ferrite_queue::{JobQueue, RedisQueue};
use ferrite_storage::{Storage, StorageConfig};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
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

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await
        .with_context(|| format!("could not connect to Postgres at {}", settings.database_url))?;

    let queue = RedisQueue::connect(
        &settings.redis_url,
        &settings.queue_group,
        settings.max_inflight_per_tenant,
        Duration::from_secs(settings.reclaim_min_idle_secs),
    )
    .await
    .with_context(|| {
        format!(
            "could not connect to Redis at {} — is Redis running (docker compose up)?",
            settings.redis_url
        )
    })?;

    // Fair-dispatch scheduler: drains per-tenant pending lists into the work
    // stream round-robin. Safe to run on every worker (dispatch is atomic).
    if settings.run_scheduler {
        tokio::spawn(scheduler_loop(queue.clone()));
        tracing::info!("fair-dispatch scheduler enabled on this worker");
    }

    run_loop(settings, storage, queue, pool).await
}

/// Periodically refresh queue ownership of an in-progress entry (interval half
/// the reclaim idle) so a healthy long job isn't stolen. Aborted when done.
fn spawn_heartbeat(
    queue: RedisQueue,
    consumer: String,
    entry_id: String,
    reclaim_min_idle_secs: u64,
) -> tokio::task::JoinHandle<()> {
    let interval = Duration::from_secs((reclaim_min_idle_secs / 2).max(1));
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        loop {
            tick.tick().await;
            if let Err(e) = queue.heartbeat(&consumer, &entry_id).await {
                tracing::warn!(entry = %entry_id, error = %e, "heartbeat failed");
            }
        }
    })
}

/// Continuously dispatch queued jobs into the work stream, fairly across tenants.
/// Drains as fast as jobs are available, then idles briefly when empty.
async fn scheduler_loop(queue: RedisQueue) {
    loop {
        match queue.dispatch_tick().await {
            Ok(true) => continue, // dispatched one; try again immediately
            Ok(false) => tokio::time::sleep(Duration::from_millis(200)).await,
            Err(e) => {
                tracing::error!(error = %e, "scheduler dispatch failed; backing off");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Claim → process → ack. Failures are retried by redelivery; jobs that exceed
/// the attempt budget are dead-lettered so one poison input can't wedge a worker.
async fn run_loop(
    settings: Settings,
    storage: Storage,
    queue: RedisQueue,
    pool: PgPool,
) -> anyhow::Result<()> {
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
                        tracing::info!(job = %job.id, tenant = %job.tenant_id, attempt = claimed.delivery_count, "claimed job");

                        // Keep the entry owned while we work so no other worker
                        // reclaims a long-running (but healthy) job.
                        let heartbeat = spawn_heartbeat(
                            queue.clone(),
                            settings.consumer_name.clone(),
                            claimed.id.clone(),
                            settings.reclaim_min_idle_secs,
                        );

                        let outcome = pipeline::process(&pool, &job, &storage, &settings.work_dir).await;
                        heartbeat.abort();

                        match outcome {
                            Ok(count) => {
                                tracing::info!(job = %job.id, artifacts = count, "job completed");
                                if let Err(e) = db::mark_completed(&pool, job.id).await {
                                    tracing::error!(job = %job.id, error = %e, "failed to mark completed");
                                }
                                webhooks::deliver(&pool, job.tenant_id, "job.completed", job.id, job.asset_id).await;
                                if let Err(e) = queue.ack(&claimed).await {
                                    tracing::error!(job = %job.id, error = %e, "failed to ack job");
                                }
                            }
                            Err(e) => {
                                if let Err(db_err) = db::mark_failed(&pool, job.id, &e.to_string()).await {
                                    tracing::error!(job = %job.id, error = %db_err, "failed to mark failed");
                                }
                                webhooks::deliver(&pool, job.tenant_id, "job.failed", job.id, job.asset_id).await;
                                handle_failure(&queue, &claimed, settings.max_attempts, e).await;
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
    claimed: &ferrite_queue::ClaimedJob,
    max_attempts: usize,
    error: pipeline::PipelineError,
) {
    let job = &claimed.job;
    if claimed.delivery_count >= max_attempts {
        tracing::error!(
            job = %job.id,
            attempts = claimed.delivery_count,
            error = %error,
            "job exceeded retry budget; dead-lettering"
        );
        if let Err(e) = queue.dead_letter(claimed).await {
            tracing::error!(job = %job.id, error = %e, "failed to dead-letter job");
        }
    } else {
        // Not acking leaves the entry pending for redelivery/reclaim.
        tracing::warn!(
            job = %job.id,
            attempt = claimed.delivery_count,
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
