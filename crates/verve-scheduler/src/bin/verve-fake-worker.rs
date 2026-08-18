//! A worker for `verve.fake`. Not video — the point of Stage 1 is to prove
//! scheduling on work whose only job is to finish.
//!
//! It closes the loop: the scheduler starts a workflow, this runs it, and the
//! completion is reported back to `/internal/work/{id}/finish` so the slot
//! comes back exactly once.

// The activity and workflow macros generate types we do not control.
#![allow(missing_debug_implementations)]

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions};
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};
use temporalio_sdk::workflows::{workflow, workflow_methods};
use temporalio_sdk::{
    ActivityOptions, Runtime, Worker, WorkerOptions, WorkflowContext, WorkflowContextView,
    WorkflowResult,
};
use verve_scheduler::temporal::TemporalEngine;

#[derive(Debug, Parser)]
#[command(name = "verve-fake-worker", about = "runs verve.fake work")]
struct Args {
    /// Temporal frontend address.
    #[arg(long, default_value = "http://localhost:7253")]
    address: String,
    /// Namespace.
    #[arg(long, default_value = "default")]
    namespace: String,
    /// Task queue to poll.
    #[arg(long, default_value = "verve")]
    task_queue: String,
    /// Where the scheduler's internal API listens.
    #[arg(long, default_value = "http://127.0.0.1:8081")]
    scheduler: String,
}

/// The opaque `spec` the scheduler passed through, as this worker reads it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FakeSpec {
    /// How many steps to run.
    #[serde(default = "one")]
    pub steps: u32,
    /// Milliseconds each step takes.
    #[serde(default)]
    pub step_ms: u64,
    /// Fail on purpose, to exercise the failure path.
    #[serde(default)]
    pub fail: bool,
}

fn one() -> u32 {
    1
}

/// Tells the scheduler a task finished. The only side effect in the process.
#[derive(Debug)]
pub struct Reporter {
    http: reqwest::Client,
    scheduler: String,
}

#[activities]
impl Reporter {
    /// One unit of fake work.
    #[activity]
    pub async fn step(_ctx: ActivityContext, step_ms: u64) -> Result<u32, ActivityError> {
        tokio::time::sleep(Duration::from_millis(step_ms)).await;
        Ok(1)
    }

    /// Report the outcome so the slot comes back.
    #[activity]
    pub async fn report(
        self: Arc<Self>,
        _ctx: ActivityContext,
        outcome: (String, bool),
    ) -> Result<(), ActivityError> {
        let (workflow_id, ok) = outcome;
        let Some(work_id) = TemporalEngine::work_id_from_workflow_id(&workflow_id) else {
            // Nothing to report against; better to say so than to retry forever.
            return Ok(());
        };

        let body = serde_json::json!({
            "state": if ok { "done" } else { "failed" },
            "error": (!ok).then_some("fake work asked to fail"),
            "cpu_seconds": 0.01,
            "machine": "fake-worker",
        });

        self.http
            .post(format!("{}/internal/work/{work_id}/finish", self.scheduler))
            .json(&body)
            .send()
            .await
            .map_err(|e| ActivityError::from(anyhow::anyhow!("reporting finish: {e}")))?;
        Ok(())
    }
}

/// The workflow the scheduler starts for `WorkKind::Fake`.
#[workflow]
#[derive(Debug)]
pub struct FakeWorkflow {
    spec: FakeSpec,
}

#[workflow_methods]
impl FakeWorkflow {
    #[init]
    pub fn new(_ctx: &WorkflowContextView, spec: FakeSpec) -> Self {
        Self { spec }
    }

    #[run(name = "verve.fake")]
    pub async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<u32> {
        let spec = ctx.state(|s| s.spec.clone());
        let opts = ActivityOptions::start_to_close_timeout(Duration::from_secs(30));

        let mut done = 0;
        for _ in 0..spec.steps {
            done += ctx
                .execute_activity(Reporter::step, spec.step_ms, opts.clone())
                .await?;
        }

        let workflow_id = ctx.workflow_id().to_string();
        ctx.execute_activity(Reporter::report, (workflow_id, !spec.fail), opts)
            .await?;

        Ok(done)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,temporalio=warn".into()),
        )
        .init();

    let args = Args::parse();
    let runtime = Runtime::new_assume_tokio(Default::default())?;

    let connection = Connection::connect(
        ConnectionOptions::new(args.address.parse::<url::Url>().context("bad --address")?)
            .identity("verve-fake-worker".to_string())
            .build(),
    )
    .await
    .with_context(|| format!("cannot reach Temporal at {}", args.address))?;
    let client = Client::new(connection, ClientOptions::new(args.namespace).build())?;

    let options = WorkerOptions::new(&args.task_queue)
        .register_activities(Reporter {
            http: reqwest::Client::new(),
            scheduler: args.scheduler.clone(),
        })
        .register_workflow::<FakeWorkflow>()?
        .build();

    tracing::info!(task_queue = %args.task_queue, scheduler = %args.scheduler, "fake worker ready");
    let mut worker = Worker::new(&runtime, client, options)?;
    worker.run().await?;
    Ok(())
}
