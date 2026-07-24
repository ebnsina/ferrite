//! Ferrite Stream API server entrypoint.

mod ai;
mod auth;
mod config;
mod cookies;
mod db;
mod email;
mod error;
mod metrics;
mod relay;
mod routes;
mod state;

use std::time::Duration;

use anyhow::Context;
use ferrite_stream_queue::RedisQueue;
use ferrite_stream_storage::{Storage, StorageConfig};
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
    tracing::info!(bind = %settings.bind_addr, "starting ferrite-stream-api");

    // Install the metrics recorder before anything records to it.
    let metrics_handle = metrics::install();

    let state = init_state(&settings, metrics_handle).await?;
    tokio::spawn(metrics::collect_loop(state.db().clone()));
    tokio::spawn(sweep_stale_jobs(
        state.db().clone(),
        settings.job_stale_secs as i64,
    ));
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

/// Periodically fail jobs stuck in a non-terminal state past the stale timeout.
async fn sweep_stale_jobs(pool: sqlx::PgPool, stale_secs: i64) {
    let mut tick = tokio::time::interval(Duration::from_secs(300));
    loop {
        tick.tick().await;
        match db::fail_stale_jobs(&pool, stale_secs).await {
            Ok(n) if n > 0 => tracing::warn!(count = n, "swept stale jobs to failed"),
            Ok(_) => {}
            Err(e) => tracing::error!(error = %e, "stale-job sweep failed"),
        }
    }
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,ferrite_stream_api=debug"));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Connect to every dependency, turning low-level failures into actionable
/// messages (e.g. "is `docker compose up` running?").
async fn init_state(
    settings: &Settings,
    metrics_handle: metrics_exporter_prometheus::PrometheusHandle,
) -> anyhow::Result<AppState> {
    // Connect as the non-superuser RLS role when configured; otherwise fall back
    // to the owner URL (RLS won't bind — a superuser bypasses it — but app-layer
    // tenant scoping still holds). Warn loudly so this isn't silently missed.
    let db_url = match &settings.api_database_url {
        Some(url) => url,
        None => {
            tracing::warn!(
                "FERRITE_API_DATABASE_URL unset — API is connecting as the DB owner, so \
                 Postgres RLS is NOT enforced (app-layer tenant scoping still applies). \
                 Set it to the ferrite_app role to enable defense-in-depth."
            );
            &settings.database_url
        }
    };
    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(db_url)
        .await
        .with_context(|| {
            format!(
                "could not connect to Postgres at {db_url} — is the database running (docker compose up)?"
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

    let mailer = crate::email::Mailer::from_settings(settings);

    Ok(AppState::new(
        db,
        storage,
        queue,
        settings.clone(),
        metrics_handle,
        mailer,
    ))
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
