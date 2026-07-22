//! Transcode job submission and status.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use ferrite_core::{Ladder, TranscodeJob};
use ferrite_queue::JobQueue;
use futures::Stream;
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
    /// HLS master playlist proxy URL; present once the job is completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playback_url: Option<String>,
    /// MPEG-DASH manifest proxy URL; present once the job is completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash_url: Option<String>,
    /// Poster image proxy URL; present once the job is completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    /// Public WebVTT storyboard (scrubbing thumbnails); present once completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storyboard_url: Option<String>,
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
            dash_url: None,
            poster_url: None,
            storyboard_url: None,
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
            dash: true,
            thumbnails: true,
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

    Ok(Json(view_with_urls(&state, ctx.tenant_id, job)))
}

/// Build a JobView, adding tokenized playback proxy URLs once completed.
/// Outputs are private in storage; the proxy authorizes delivery via a
/// short-lived token (see [`super::playback`]).
fn view_with_urls(state: &AppState, tenant_id: Uuid, job: Job) -> JobView {
    let completed = job.state == "completed";
    let job_id = job.id;
    let mut view = JobView::from(job);
    if completed {
        let s = state.settings();
        let exp = super::playback::now_unix() + super::playback::TOKEN_TTL_SECS;
        let token = super::playback::sign_token(&s.playback_secret, tenant_id, job_id, exp);
        let base = &s.public_url;
        view.playback_url = Some(format!(
            "{base}/playback/{job_id}/master.m3u8?token={token}"
        ));
        view.dash_url = Some(format!(
            "{base}/playback/{job_id}/dash/manifest.mpd?token={token}"
        ));
        view.poster_url = Some(format!(
            "{base}/playback/{job_id}/thumbs/poster.jpg?token={token}"
        ));
        view.storyboard_url = Some(format!(
            "{base}/playback/{job_id}/thumbs/thumbs.vtt?token={token}"
        ));
    }
    view
}

const SSE_POLL: Duration = Duration::from_millis(1000);
const SSE_MAX_TICKS: u32 = 30 * 60; // ~30 min safety cap

/// `GET /v1/jobs/:id/events` — Server-Sent Events stream of job status until it
/// reaches a terminal state. One long-lived connection replaces client polling.
pub async fn job_events(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Verify ownership before opening the stream.
    db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let tenant_id = ctx.tenant_id;
    let stream = async_stream::stream! {
        let mut ticker = tokio::time::interval(SSE_POLL);
        for _ in 0..SSE_MAX_TICKS {
            ticker.tick().await;
            match db::find_job(state.db(), tenant_id, id).await {
                Ok(Some(job)) => {
                    let terminal = job.state == "completed" || job.state == "failed";
                    let view = view_with_urls(&state, tenant_id, job);
                    if let Ok(event) = Event::default().json_data(view) {
                        yield Ok(event);
                    }
                    if terminal { break; }
                }
                _ => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
