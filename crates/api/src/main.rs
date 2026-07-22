//! Ferrite API server entrypoint.

mod auth;
mod config;
mod db;
mod error;
mod routes;
mod state;

use std::time::Duration;

use anyhow::Context;
use ferrite_queue::RedisQueue;
use ferrite_storage::{Storage, StorageConfig};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Settings;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present; real environment always wins over file values.
    let _ = dotenvy::dotenv();
    init_tracing();

    let settings = Settings::load().context("failed to load configuration")?;
    tracing::info!(bind = %settings.bind_addr, "starting ferrite-api");

    let state = init_state(&settings).await?;
    let app = routes::build(state);

    let listener = tokio::net::TcpListener::bind(&settings.bind_addr)
        .await
        .with_context(|| format!("failed to bind {}", settings.bind_addr))?;

    tracing::info!(addr = %settings.bind_addr, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")?;

    tracing::info!("shutdown complete");
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ferrite_api=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Connect to every dependency, turning low-level failures into actionable
/// messages (e.g. "is `docker compose up` running?").
async fn init_state(settings: &Settings) -> anyhow::Result<AppState> {
    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&settings.database_url)
        .await
        .with_context(|| {
            format!(
                "could not connect to Postgres at {} — is the database running (docker compose up)?",
                settings.database_url
            )
        })?;

    let storage = Storage::connect(StorageConfig {
        bucket: settings.s3_bucket.clone(),
        endpoint_url: settings.s3_endpoint_url.clone(),
        force_path_style: settings.s3_force_path_style,
    })
    .await
    .context("failed to initialize object storage client")?;

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

    Ok(AppState::new(db, storage, queue, settings.clone()))
}

/// Resolve on SIGINT (Ctrl-C) or SIGTERM so in-flight requests can drain.
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

    tracing::info!("shutdown signal received");
}
