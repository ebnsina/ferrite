//! The scheduler process: an admission loop and the internal API beside it.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use ferrite_scheduler::admission::{Config, Scheduler};
use ferrite_scheduler::api::{ApiState, router};
use ferrite_scheduler::engine::{RecordingEngine, WorkflowEngine};
use ferrite_scheduler::store::Store;

#[derive(Debug, Parser)]
#[command(name = "ferrite-scheduler", version, about = "admission control")]
struct Args {
    /// sched_db connection string.
    #[arg(long, env = "SCHED_DATABASE_URL")]
    database_url: String,

    /// Where the internal API listens. Private interface only.
    #[arg(long, env = "FERRITE_SCHEDULER_BIND", default_value = "127.0.0.1:8081")]
    bind: String,

    /// Total slots across every worker.
    #[arg(long, env = "FERRITE_TOTAL_SLOTS", default_value_t = 64)]
    total_slots: u32,

    /// Gap between admission ticks, in milliseconds.
    #[arg(long, default_value_t = 250)]
    tick_ms: u64,

    /// Run the API and the loop without starting any real workflow.
    #[arg(long)]
    dry_run: bool,

    /// Temporal frontend address. Ignored with --dry-run.
    #[cfg(feature = "temporal")]
    #[arg(
        long,
        env = "TEMPORAL_ADDRESS",
        default_value = "http://localhost:7253"
    )]
    temporal_address: String,

    /// Temporal namespace.
    #[cfg(feature = "temporal")]
    #[arg(long, default_value = "default")]
    temporal_namespace: String,

    /// Task queue workers poll.
    #[cfg(feature = "temporal")]
    #[arg(long, default_value = "ferrite")]
    task_queue: String,
}

#[cfg(feature = "temporal")]
async fn build_engine(args: &Args) -> Result<Arc<dyn WorkflowEngine>> {
    use ferrite_scheduler::temporal::{TemporalConfig, TemporalEngine};

    let engine = TemporalEngine::connect(TemporalConfig {
        address: args.temporal_address.clone(),
        namespace: args.temporal_namespace.clone(),
        task_queue: args.task_queue.clone(),
    })
    .await
    .context("connecting to Temporal")?;
    tracing::info!(address = %args.temporal_address, "workflow engine connected");
    Ok(Arc::new(engine))
}

#[cfg(not(feature = "temporal"))]
async fn build_engine(_args: &Args) -> Result<Arc<dyn WorkflowEngine>> {
    anyhow::bail!("built without the `temporal` feature; pass --dry-run or rebuild")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _telemetry = ferrite_telemetry::init(
        ferrite_telemetry::Config::from_env("ferrite-scheduler").context("telemetry config")?,
    )
    .context("telemetry")?;

    let store = Store::connect(&args.database_url)
        .await
        .with_context(|| format!("connecting to {}", args.database_url))?;

    let engine: Arc<dyn WorkflowEngine> = if args.dry_run {
        tracing::warn!("--dry-run: nothing will actually be started");
        Arc::new(RecordingEngine::new())
    } else {
        build_engine(&args).await?
    };

    let config = Config {
        total_slots: args.total_slots,
        tick: Duration::from_millis(args.tick_ms),
        ..Config::default()
    };

    let api = router(ApiState {
        store: store.clone(),
        engine: engine.clone(),
        total_slots: args.total_slots,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let scheduler = Scheduler::new(store, engine, config);
    let loop_task = tokio::spawn(scheduler.run(shutdown_rx));

    let listener = tokio::net::TcpListener::bind(&args.bind)
        .await
        .with_context(|| format!("binding {}", args.bind))?;
    tracing::info!(bind = %args.bind, "internal API listening");

    axum::serve(listener, api)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            let _ = shutdown_tx.send(true);
        })
        .await
        .context("serving")?;

    loop_task.await.context("admission loop panicked")??;
    Ok(())
}
