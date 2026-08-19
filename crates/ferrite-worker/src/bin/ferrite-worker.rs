//! The machine that converts video.
//!
//! Pulls chunks off the queue, encodes every rung from one decode, and joins
//! the pieces when they are all in. Temporal remembers where the work got to,
//! so a machine dying loses a chunk rather than an asset.

// The activity and workflow macros generate types we do not control.
#![allow(missing_debug_implementations)]

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use ferrite_worker::work::{AssetJob, EncodeChunk, JoinRung, straggler_budget};
use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions};
use temporalio_common::RetryPolicy;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};
use temporalio_sdk::workflows::{join_all, workflow, workflow_methods};
use temporalio_sdk::{
    ActivityOptions, Runtime, Worker, WorkerOptions, WorkflowContext, WorkflowContextView,
    WorkflowResult,
};

#[derive(Debug, Parser)]
#[command(name = "ferrite-worker", version, about = "encodes chunks")]
struct Args {
    /// Temporal frontend address.
    #[arg(
        long,
        env = "TEMPORAL_ADDRESS",
        default_value = "http://localhost:7253"
    )]
    address: String,
    /// Namespace.
    #[arg(long, default_value = "default")]
    namespace: String,
    /// Task queue to poll.
    #[arg(long, env = "FERRITE_TASK_QUEUE", default_value = "ferrite")]
    task_queue: String,
    /// Encoder threads per chunk. Encoders stop scaling past about sixteen.
    #[arg(long, default_value_t = 8)]
    threads: u16,
    /// The scheduler's internal API, for reporting completion.
    #[arg(
        long,
        env = "FERRITE_SCHEDULER",
        default_value = "http://127.0.0.1:8081"
    )]
    scheduler: String,
}

/// This machine's name, recorded against what it cost.
fn gethostname() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|h| !h.is_empty())
        .unwrap_or_else(|| "unknown".into())
}

/// What a chunk cost, so the scheduler can bill it.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkDone {
    /// Which slice.
    pub index: u32,
    /// Frames encoded across every rung.
    pub frames: u64,
    /// Compressed bytes across every rung.
    pub bytes: u64,
    /// Wall clock, for straggler detection.
    pub seconds: f64,
}

/// The video work itself. Everything here is a side effect, which is exactly
/// why it lives in an activity rather than a workflow.
#[derive(Debug)]
pub struct Encoder {
    threads: u16,
    scheduler: String,
}

/// The work id inside a workflow id, by the convention the scheduler names them
/// with: `ferrite-<kind>-<uuid>`. Duplicated rather than depended on, because a
/// worker knowing the scheduler's types would be the wrong direction.
fn work_id_of(workflow_id: &str) -> Option<String> {
    let parts: Vec<&str> = workflow_id.split('-').collect();
    (parts.len() >= 5).then(|| parts[parts.len() - 5..].join("-"))
}

#[activities]
impl Encoder {
    /// Encode one chunk across every rung, from one decode.
    #[activity]
    pub async fn encode_chunk(
        self: Arc<Self>,
        _ctx: ActivityContext,
        job: EncodeChunk,
    ) -> Result<ChunkDone, ActivityError> {
        let threads = self.threads;
        // Blocking, CPU-bound and long: it must not sit on an async worker.
        let done = tokio::task::spawn_blocking(move || {
            let started = std::time::Instant::now();
            let outputs: Vec<ferrite_av::transcode::Output> = job
                .outputs
                .iter()
                .map(|o| {
                    let mut spec = o.spec.clone();
                    spec.threads = threads;
                    ferrite_av::transcode::Output {
                        path: o.path.clone(),
                        spec,
                    }
                })
                .collect();

            let reports = ferrite_av::transcode::run_range(
                &job.input,
                &outputs,
                Some(job.chunk),
                Arc::new(ferrite_av::NeverCancel),
            )?;

            Ok::<_, ferrite_av::AvError>(ChunkDone {
                index: job.chunk.index,
                frames: reports.iter().map(|r| r.frames).sum(),
                bytes: reports.iter().map(|r| r.bytes).sum(),
                seconds: started.elapsed().as_secs_f64(),
            })
        })
        .await
        .map_err(|e| ActivityError::from(anyhow::anyhow!("encode panicked: {e}")))?
        .map_err(|e| ActivityError::from(anyhow::anyhow!("{e}")))?;

        Ok(done)
    }

    /// Encode the audio track. One per asset, never chunked and never per rung.
    #[activity]
    pub async fn encode_audio(_ctx: ActivityContext, job: AssetJob) -> Result<bool, ActivityError> {
        tokio::task::spawn_blocking(move || ferrite_worker::asset::encode_audio(&job))
            .await
            .map_err(|e| ActivityError::from(anyhow::anyhow!("audio panicked: {e}")))?
            .map_err(|e| ActivityError::from(anyhow::anyhow!("{e}")))
    }

    /// Package whatever has landed. Called as each rung finishes, so the
    /// manifest grows instead of appearing all at once.
    #[activity]
    pub async fn publish(_ctx: ActivityContext, job: AssetJob) -> Result<usize, ActivityError> {
        tokio::task::spawn_blocking(move || {
            ferrite_worker::asset::publish(&job).map(|p| p.renditions.len())
        })
        .await
        .map_err(|e| ActivityError::from(anyhow::anyhow!("publish panicked: {e}")))?
        .map_err(|e| ActivityError::from(anyhow::anyhow!("{e}")))
    }

    /// Tell the scheduler the asset is done, so the slot comes back and the
    /// work is billed. Without this a finished asset holds capacity forever.
    #[activity]
    pub async fn report_done(
        self: Arc<Self>,
        _ctx: ActivityContext,
        outcome: (String, u64, f64),
    ) -> Result<(), ActivityError> {
        let (workflow_id, bytes, cpu_seconds) = outcome;
        let Some(work_id) = work_id_of(&workflow_id) else {
            return Ok(());
        };

        let body = serde_json::json!({
            "state": "done",
            "cpu_seconds": cpu_seconds,
            "bytes_written": bytes,
            "machine": gethostname(),
        });
        let status = std::process::Command::new("curl")
            .args(["-sS", "-o", "/dev/null", "-w", "%{http_code}", "-X", "POST"])
            .args(["-H", "content-type: application/json", "-d"])
            .arg(body.to_string())
            .arg(format!("{}/internal/work/{work_id}/finish", self.scheduler))
            .output()
            .map_err(|e| ActivityError::from(anyhow::anyhow!("reporting done: {e}")))?;

        let code = String::from_utf8_lossy(&status.stdout).trim().to_string();
        if !code.starts_with('2') {
            return Err(ActivityError::from(anyhow::anyhow!(
                "scheduler returned {code} for work {work_id}"
            )));
        }
        Ok(())
    }

    /// Concatenate one rung's pieces. No re-encode.
    #[activity]
    pub async fn join_rung(_ctx: ActivityContext, job: JoinRung) -> Result<u64, ActivityError> {
        let bytes = tokio::task::spawn_blocking(move || {
            let joined = ferrite_av::join::run(&job.parts, &job.output)?;
            // Leftovers quietly double the storage bill, and after the join
            // there is nothing left to re-issue them for.
            for part in &job.parts {
                let _ = std::fs::remove_file(part);
            }
            Ok::<_, ferrite_av::AvError>(joined.bytes)
        })
        .await
        .map_err(|e| ActivityError::from(anyhow::anyhow!("join panicked: {e}")))?
        .map_err(|e| ActivityError::from(anyhow::anyhow!("{e}")))?;
        Ok(bytes)
    }
}

/// Fan chunks out, then join each rung once they are all in.
///
/// Named for the work kind the scheduler dispatches: it starts workflows by
/// name without linking these types, so the two have to agree.
///
/// The plan travels with the job rather than being recomputed here: a workflow
/// must replay identically, and a resplit could cut somewhere else.
#[workflow]
#[derive(Debug)]
pub struct AssetWorkflow {
    job: AssetJob,
}

#[workflow_methods]
impl AssetWorkflow {
    #[init]
    pub fn new(_ctx: &WorkflowContextView, job: AssetJob) -> Self {
        Self { job }
    }

    #[run(name = "ferrite.asset_quality")]
    pub async fn run(ctx: &mut WorkflowContext<Self>) -> WorkflowResult<u64> {
        let job = ctx.state(|s| s.job.clone());
        let (fast, quality) = job.two_paths();

        // A chunk that stops making progress is re-issued rather than waited
        // for: the fast path is only as fast as its slowest chunk. The retry
        // lands wherever there is capacity, which is usually another machine.
        let chunk_options = |chunk: &ferrite_av::split::Chunk, rungs: usize| {
            ActivityOptions::with_start_to_close_timeout(straggler_budget(chunk, rungs))
                .retry_policy(
                    RetryPolicy::builder()
                        .initial_interval(Duration::from_secs(1))
                        .backoff_coefficient(1.0)
                        .maximum_interval(Duration::from_secs(5))
                        // Bounded: a chunk that fails everywhere is broken, not
                        // unlucky, and should fail the asset rather than loop.
                        .maximum_attempts(4)
                        .build(),
                )
                .build()
        };
        let join = ActivityOptions::start_to_close_timeout(Duration::from_secs(600));
        let short = ActivityOptions::start_to_close_timeout(Duration::from_secs(300));

        // Audio first and alongside: one track, and the fast rung is unwatchable
        // without it.
        let audio = ctx.execute_activity(Encoder::encode_audio, job.clone(), short.clone());

        // The fast path: one mid rung across every free machine, so the asset
        // becomes playable without waiting for the whole ladder.
        let mut frames = 0u64;
        let mut cpu_seconds = 0.0;
        let mut bytes = 0u64;

        for done in join_all(
            fast.chunks()
                .into_iter()
                .map(|job| {
                    let options = chunk_options(&job.chunk, job.outputs.len());
                    ctx.execute_activity(Encoder::encode_chunk, job, options)
                })
                .collect::<Vec<_>>(),
        )
        .await
        {
            let done = done?;
            frames += done.frames;
            cpu_seconds += done.seconds;
        }

        for joined in join_all(
            fast.rungs
                .iter()
                .map(|rung| {
                    ctx.execute_activity(Encoder::join_rung, fast.join_rung(rung), join.clone())
                })
                .collect::<Vec<_>>(),
        )
        .await
        {
            bytes += joined?;
        }

        audio.await?;
        // Playable from here. Everything below only adds quality.
        ctx.execute_activity(Encoder::publish, fast.clone(), short.clone())
            .await?;

        if quality.rungs.is_empty() {
            ctx.execute_activity(
                Encoder::report_done,
                (ctx.workflow_id().to_string(), bytes, cpu_seconds),
                short,
            )
            .await?;
            return Ok(frames);
        }

        // The quality path: the rest of the ladder, published as it lands.
        for done in join_all(
            quality
                .chunks()
                .into_iter()
                .map(|job| {
                    let options = chunk_options(&job.chunk, job.outputs.len());
                    ctx.execute_activity(Encoder::encode_chunk, job, options)
                })
                .collect::<Vec<_>>(),
        )
        .await
        {
            let done = done?;
            frames += done.frames;
            cpu_seconds += done.seconds;
        }

        for rung in &quality.rungs {
            ctx.execute_activity(Encoder::join_rung, quality.join_rung(rung), join.clone())
                .await?;
            // Republished per rung: the manifest grows rather than appearing
            // all at once, and a viewer gets the better rung as soon as it is
            // there.
            ctx.execute_activity(Encoder::publish, job.clone(), short.clone())
                .await?;
        }

        ctx.execute_activity(
            Encoder::report_done,
            (ctx.workflow_id().to_string(), bytes, cpu_seconds),
            short,
        )
        .await?;
        Ok(frames)
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
            .identity("ferrite-worker".to_string())
            .build(),
    )
    .await
    .with_context(|| format!("cannot reach Temporal at {}", args.address))?;
    let client = Client::new(connection, ClientOptions::new(args.namespace).build())?;

    let options = WorkerOptions::new(&args.task_queue)
        .register_activities(Encoder {
            threads: args.threads,
            scheduler: args.scheduler.clone(),
        })
        .register_workflow::<AssetWorkflow>()?
        .build();

    tracing::info!(
        task_queue = %args.task_queue,
        threads = args.threads,
        "worker ready"
    );
    let mut worker = Worker::new(&runtime, client, options)?;
    worker.run().await?;
    Ok(())
}
