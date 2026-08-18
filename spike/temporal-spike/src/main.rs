//! Throwaway. Answers one question before the design rests on it: does the
//! pre-1.0 Temporal Rust SDK hold 1,000 steps split into child workflows?
//!
//! Shape is deliberately the pipeline's: a parent that fans chunks out to child
//! workflows, each running activities, results joined back. No video involved.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use temporalio_client::{
    Client, ClientOptions, Connection, ConnectionOptions, WorkflowGetResultOptions,
    WorkflowStartOptions,
};
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};
use temporalio_sdk::workflows::{join_all, workflow, workflow_methods};
use temporalio_sdk::{
    ActivityOptions, ChildWorkflowOptions, Runtime, Worker, WorkerOptions, WorkflowContext,
    WorkflowContextView, WorkflowResult,
};

const TASK_QUEUE: &str = "verve-spike";

#[derive(Debug, Parser)]
#[command(about = "Temporal SDK load spike: N steps across C child workflows")]
struct Args {
    /// Temporal frontend address.
    #[arg(long, default_value = "http://localhost:7233")]
    address: String,
    /// Namespace.
    #[arg(long, default_value = "default")]
    namespace: String,
    /// Total steps. The pipeline's shape: a 10-min video is ~60 chunks × rungs.
    #[arg(long, default_value_t = 1000)]
    steps: u32,
    /// Steps per child workflow.
    #[arg(long, default_value_t = 50)]
    chunk: u32,
    /// Workflow id suffix, so repeat runs do not collide.
    #[arg(long, default_value = "1")]
    run_id: String,
}

// ---------------------------------------------------------------- activities

/// Stands in for encode-a-chunk. Counts calls so double execution is visible.
#[derive(Debug, Default)]
pub struct SpikeActivities {
    pub calls: AtomicU64,
}

#[activities]
impl SpikeActivities {
    /// Cheap and deterministic: the point is the engine, not the work.
    #[activity]
    pub async fn step(
        self: Arc<Self>,
        _ctx: ActivityContext,
        n: u32,
    ) -> Result<u64, ActivityError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(u64::from(n) * 2)
    }
}

// ----------------------------------------------------------------- workflows

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChildInput {
    pub first: u32,
    pub count: u32,
}

/// One chunk's worth of steps. The unit a straggler re-issue would replace.
#[workflow]
#[derive(Debug)]
pub struct ChunkWorkflow {
    input: ChildInput,
}

#[workflow_methods]
impl ChunkWorkflow {
    #[init]
    pub fn new(_ctx: &WorkflowContextView, input: ChildInput) -> Self {
        Self { input }
    }

    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<u64> {
        let input = ctx.state(|s| s.input.clone());
        let opts = ActivityOptions::start_to_close_timeout(Duration::from_secs(30));
        let mut total = 0u64;
        for i in 0..input.count {
            total += ctx
                .execute_activity(SpikeActivities::step, input.first + i, opts.clone())
                .await?;
        }
        Ok(total)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FanOutInput {
    pub steps: u32,
    pub chunk: u32,
}

/// The parent. 1,000 steps in one workflow blows the history limit; splitting
/// into children is the pattern the pipeline needs, so it is what we test.
#[workflow]
#[derive(Debug)]
pub struct FanOutWorkflow {
    input: FanOutInput,
}

#[workflow_methods]
impl FanOutWorkflow {
    #[init]
    pub fn new(_ctx: &WorkflowContextView, input: FanOutInput) -> Self {
        Self { input }
    }

    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<u64> {
        let input = ctx.state(|s| s.input.clone());
        let chunk = input.chunk.max(1);
        let children = input.steps.div_ceil(chunk);

        let mut started = Vec::with_capacity(children as usize);
        for c in 0..children {
            let first = c * chunk;
            let count = chunk.min(input.steps - first);
            started.push(
                ctx.start_child_workflow(
                    ChunkWorkflow::run,
                    ChildInput { first, count },
                    ChildWorkflowOptions::workflow_id(format!("{}-chunk-{c}", ctx.workflow_id())),
                )
                .await?,
            );
        }

        // join_all, not futures::join_all: replay must poll in the same order.
        let results = join_all(started.into_iter().map(|s| s.result())).await;

        let mut total = 0u64;
        for r in results {
            total += r?;
        }
        Ok(total)
    }
}

// ---------------------------------------------------------------------- main

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,temporalio=warn".into()),
        )
        .init();

    let args = Args::parse();
    let expected: u64 = (0..args.steps).map(|n| u64::from(n) * 2).sum();

    let runtime = Runtime::new_assume_tokio(Default::default())?;
    let connection = Connection::connect(
        ConnectionOptions::new(args.address.parse::<url::Url>().context("bad --address")?)
            .identity("verve-spike".to_string())
            .build(),
    )
    .await
    .with_context(|| format!("cannot reach Temporal at {}", args.address))?;
    let client = Client::new(
        connection,
        ClientOptions::new(args.namespace.clone()).build(),
    )?;

    let worker_options = WorkerOptions::new(TASK_QUEUE)
        .register_activities(SpikeActivities::default())
        .register_workflow::<FanOutWorkflow>()?
        .register_workflow::<ChunkWorkflow>()?
        .build();
    let mut worker = Worker::new(&runtime, client.clone(), worker_options)?;

    // The worker future is !Send, so it lives on a LocalSet rather than a task.
    let local = tokio::task::LocalSet::new();
    local.spawn_local(async move {
        if let Err(e) = worker.run().await {
            tracing::error!("worker stopped: {e}");
        }
    });

    let workflow_id = format!("verve-spike-{}", args.run_id);
    tracing::info!(
        steps = args.steps,
        chunk = args.chunk,
        children = args.steps.div_ceil(args.chunk),
        %workflow_id,
        "starting"
    );

    let started = Instant::now();
    let total = local
        .run_until(async {
            let handle = client
                .start_workflow(
                    FanOutWorkflow::run,
                    FanOutInput {
                        steps: args.steps,
                        chunk: args.chunk,
                    },
                    WorkflowStartOptions::new(TASK_QUEUE.to_string(), workflow_id.clone()).build(),
                )
                .await
                .context("start_workflow failed")?;

            handle
                .get_result(WorkflowGetResultOptions::builder().build())
                .await
                .context("workflow did not complete")
        })
        .await?;
    let elapsed = started.elapsed();

    println!();
    println!("steps      {}", args.steps);
    println!("children   {}", args.steps.div_ceil(args.chunk));
    println!("elapsed    {:.2}s", elapsed.as_secs_f64());
    println!(
        "throughput {:.0} steps/s",
        f64::from(args.steps) / elapsed.as_secs_f64()
    );
    println!("total      {total} (expected {expected})");

    // Exactly-once is the whole reason we are on an engine at all.
    anyhow::ensure!(
        total == expected,
        "sum mismatch: steps ran twice or not at all"
    );
    println!("\nPASS");
    Ok(())
}
