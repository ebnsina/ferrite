//! Transcode job submission and status.

use axum::extract::{Path, State};
use axum::Json;
use ferrite_core::{Ladder, TranscodeJob};
use ferrite_queue::JobQueue;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    // Asset must belong to the tenant and have finished uploading.
    let asset = db::find_asset(state.db(), ctx.tenant_id, body.asset_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if asset.status != "ready" {
        return Err(ApiError::Conflict(format!(
            "asset is not ready for transcoding (status: {})",
            asset.status
        )));
    }

    let job_id = Uuid::new_v4();
    let output_prefix = format!("{}/outputs/{}", ctx.tenant_id, job_id);

    let (job, created) = db::create_job(
        state.db(),
        ctx.tenant_id,
        job_id,
        body.asset_id,
        &output_prefix,
        body.idempotency_key.as_deref(),
    )
    .await?;

    // Only enqueue when this call actually created the job (idempotency-safe).
    if created {
        let transcode = TranscodeJob {
            id: job.id,
            tenant_id: ctx.tenant_id,
            asset_id: body.asset_id,
            source_key: asset.original_key,
            output_prefix,
            ladder: Ladder::default_abr(),
            hls: true,
            dash: false,
            thumbnails: false,
        };
        state.queue().enqueue(&transcode).await?;
        tracing::info!(job = %job.id, tenant = %ctx.tenant_id, "job enqueued");
    }

    Ok(Json(job.into()))
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
    Ok(Json(job.into()))
}
