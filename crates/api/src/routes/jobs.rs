//! Transcode job submission and status.

use std::time::Duration;

use axum::extract::{Path, State};
use axum::Json;
use ferrite_core::{Ladder, TranscodeJob};
use ferrite_queue::JobQueue;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PLAYBACK_URL_TTL: Duration = Duration::from_secs(60 * 60);

use crate::auth::TenantContext;
use crate::db::{self, Job};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct JobView {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub state: String,
    pub progress: f32,
    pub error: Option<String>,
    pub queued_at: String,
    pub finished_at: Option<String>,
    /// Presigned HLS master playlist URL; present once the job is completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playback_url: Option<String>,
}

impl From<Job> for JobView {
    fn from(j: Job) -> Self {
        JobView {
            id: j.id,
            asset_id: j.asset_id,
            state: j.state,
            progress: j.progress,
            error: j.error,
            queued_at: j.queued_at.to_rfc3339(),
            finished_at: j.finished_at.map(|t| t.to_rfc3339()),
            playback_url: None,
        }
    }
}

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub asset_id: Uuid,
    /// Optional; a repeated key returns the same job instead of a duplicate.
    pub idempotency_key: Option<String>,
}

/// `POST /v1/jobs` — submit a transcode job for a ready asset.
pub async fn create_job(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<CreateJobRequest>,
) -> ApiResult<Json<JobView>> {
    let job = submit_job(
        &state,
        ctx.tenant_id,
        body.asset_id,
        body.idempotency_key.as_deref(),
    )
    .await?;
    Ok(Json(job.into()))
}

/// Validate a ready asset, create its job, and enqueue it (idempotency-safe).
async fn submit_job(
    state: &AppState,
    tenant_id: Uuid,
    asset_id: Uuid,
    idempotency_key: Option<&str>,
) -> ApiResult<Job> {
    let asset = db::find_asset(state.db(), tenant_id, asset_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if asset.status != "ready" {
        return Err(ApiError::Conflict(format!(
            "asset is not ready for transcoding (status: {})",
            asset.status
        )));
    }

    let job_id = Uuid::new_v4();
    let output_prefix = format!("{tenant_id}/outputs/{job_id}");
    let (job, created) = db::create_job(
        state.db(),
        tenant_id,
        job_id,
        asset_id,
        &output_prefix,
        idempotency_key,
    )
    .await?;

    if created {
        let transcode = TranscodeJob {
            id: job.id,
            tenant_id,
            asset_id,
            source_key: asset.original_key,
            output_prefix,
            ladder: Ladder::default_abr(),
            hls: true,
            dash: false,
            thumbnails: false,
        };
        state.queue().enqueue(&transcode).await?;
        tracing::info!(job = %job.id, tenant = %tenant_id, "job enqueued");
    }
    Ok(job)
}

const MAX_BATCH: usize = 500;

#[derive(Deserialize)]
pub struct BatchJobRequest {
    pub asset_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct BatchResult {
    pub submitted: Vec<JobView>,
    pub skipped: Vec<SkippedItem>,
}

#[derive(Serialize)]
pub struct SkippedItem {
    pub asset_id: Uuid,
    pub reason: String,
}

/// `POST /v1/jobs/batch` — submit many assets at once. Partial success: each
/// asset is reported as submitted or skipped; the fair queue spreads the load.
pub async fn create_jobs_batch(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<BatchJobRequest>,
) -> ApiResult<Json<BatchResult>> {
    if body.asset_ids.is_empty() {
        return Err(ApiError::BadRequest("asset_ids is empty".into()));
    }
    if body.asset_ids.len() > MAX_BATCH {
        return Err(ApiError::BadRequest(format!(
            "batch too large ({}, max {MAX_BATCH})",
            body.asset_ids.len()
        )));
    }

    let mut submitted = Vec::new();
    let mut skipped = Vec::new();
    for asset_id in body.asset_ids {
        match submit_job(&state, ctx.tenant_id, asset_id, None).await {
            Ok(job) => submitted.push(job.into()),
            Err(e) => skipped.push(SkippedItem {
                asset_id,
                reason: e.to_string(),
            }),
        }
    }
    Ok(Json(BatchResult { submitted, skipped }))
}

/// `GET /v1/jobs` — list the tenant's jobs.
pub async fn list_jobs(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<JobView>>> {
    let jobs = db::list_jobs(state.db(), ctx.tenant_id).await?;
    Ok(Json(jobs.into_iter().map(JobView::from).collect()))
}

/// `GET /v1/jobs/:id` — fetch a single job's status.
pub async fn get_job(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<JobView>> {
    let job = db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let playback_key =
        (job.state == "completed").then(|| format!("{}/master.m3u8", job.output_prefix));
    let mut view = JobView::from(job);
    if let Some(key) = playback_key {
        view.playback_url = Some(state.storage().presign_get(&key, PLAYBACK_URL_TTL).await?);
    }
    Ok(Json(view))
}
