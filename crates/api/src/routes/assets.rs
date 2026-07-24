//! Asset upload and listing.
//!
//! Uploads go **directly** to object storage via a presigned PUT URL — the API
//! only issues the URL and records metadata, never proxying bytes.

use std::time::Duration;

use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use ferrite_stream_core::{Clip, Ladder, ShortsSpec, TranscodeJob};
use ferrite_stream_queue::JobQueue;

use crate::auth::TenantContext;
use crate::db::{self, Asset};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const UPLOAD_URL_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Serialize)]
pub struct AssetView {
    pub id: Uuid,
    pub filename: String,
    pub bytes: Option<i64>,
    pub status: String,
    pub created_at: String,
    /// Signed, embeddable derived-media URLs (present once the asset is ready).
    pub thumbnail_url: Option<String>,
    pub preview_url: Option<String>,
    /// Presigned GET for the original file — lets the browser play the source
    /// (or a clip) directly, before any transcode. Only set on the detail view.
    #[serde(default)]
    pub source_url: Option<String>,
}

/// Build a view, attaching signed thumbnail/preview URLs for ready assets.
fn asset_view(state: &AppState, tenant: Uuid, a: Asset) -> AssetView {
    let (thumbnail_url, preview_url) = if a.status == "ready" {
        let (t, p) = super::media::signed_urls(state, tenant, a.id);
        (Some(t), Some(p))
    } else {
        (None, None)
    };
    AssetView {
        id: a.id,
        filename: a.filename,
        bytes: a.bytes,
        status: a.status,
        created_at: a.created_at.to_rfc3339(),
        thumbnail_url,
        preview_url,
        source_url: None,
    }
}

#[derive(Deserialize, Validate)]
pub struct CreateAssetRequest {
    #[validate(length(min = 1, max = 255))]
    pub filename: String,
}

#[derive(Serialize)]
pub struct CreateAssetResponse {
    pub asset: AssetView,
    /// Presigned URL the client PUTs the file bytes to.
    pub upload_url: String,
    pub expires_in_secs: u64,
}

/// `POST /v1/assets` — register an asset and get a presigned upload URL.
pub async fn create_asset(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<CreateAssetRequest>,
) -> ApiResult<Json<CreateAssetResponse>> {
    body.validate().map_err(ApiError::Validation)?;

    let asset_id = Uuid::new_v4();
    let key = source_key(ctx.tenant_id, asset_id, &body.filename);

    let asset = db::create_asset(state.db(), ctx.tenant_id, asset_id, &body.filename, &key).await?;
    let upload_url = state.storage().presign_put(&key, UPLOAD_URL_TTL).await?;

    Ok(Json(CreateAssetResponse {
        asset: asset_view(&state, ctx.tenant_id, asset),
        upload_url,
        expires_in_secs: UPLOAD_URL_TTL.as_secs(),
    }))
}

#[derive(Deserialize)]
pub struct CompleteAssetRequest {
    pub bytes: Option<i64>,
}

/// `POST /v1/assets/:id/complete` — mark the upload finished.
pub async fn complete_asset(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
    Json(body): Json<CompleteAssetRequest>,
) -> ApiResult<Json<AssetView>> {
    let updated = db::mark_asset_ready(state.db(), ctx.tenant_id, id, body.bytes).await?;
    if !updated {
        return Err(ApiError::NotFound);
    }
    let asset = db::find_asset(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(asset_view(&state, ctx.tenant_id, asset)))
}

/// `GET /v1/assets` — list the tenant's assets.
pub async fn list_assets(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<AssetView>>> {
    let assets = db::list_assets(state.db(), ctx.tenant_id).await?;
    Ok(Json(
        assets
            .into_iter()
            .map(|a| asset_view(&state, ctx.tenant_id, a))
            .collect(),
    ))
}

/// `GET /v1/assets/:id` — fetch a single asset.
pub async fn get_asset(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AssetView>> {
    let asset = db::find_asset(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let ready = asset.status == "ready";
    let original_key = asset.original_key.clone();
    let mut view = asset_view(&state, ctx.tenant_id, asset);
    // Direct-play URL for the original file so a ready video (or a fresh clip)
    // is watchable immediately, without waiting on a transcode.
    if ready {
        view.source_url = state
            .storage()
            .presign_get(&original_key, Duration::from_secs(60 * 60))
            .await
            .ok();
    }
    Ok(Json(view))
}

#[derive(Deserialize, Validate)]
pub struct ClipRequest {
    /// Start/end offsets in seconds.
    pub start: f64,
    pub end: f64,
    #[validate(length(max = 255))]
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Serialize)]
pub struct ClipResponse {
    pub asset: AssetView,
    pub job_id: Uuid,
}

/// `POST /v1/assets/{id}/clip` — trim `[start, end]` of a source into a new asset.
pub async fn clip_asset(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
    Json(body): Json<ClipRequest>,
) -> ApiResult<Json<ClipResponse>> {
    body.validate().map_err(ApiError::Validation)?;
    if !(body.start >= 0.0 && body.end > body.start) {
        return Err(ApiError::BadRequest("invalid time range".into()));
    }

    let source = db::find_asset(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if source.status != "ready" {
        return Err(ApiError::BadRequest("source asset is not ready".into()));
    }

    let stem = source
        .filename
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(&source.filename);
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{stem} clip.mp4"));

    let dest_id = Uuid::new_v4();
    let dest_key = source_key(ctx.tenant_id, dest_id, &name);
    let dest =
        db::create_processing_asset(state.db(), ctx.tenant_id, dest_id, &name, &dest_key).await?;

    let job_id = Uuid::new_v4();
    db::create_clip_job(state.db(), ctx.tenant_id, job_id, id, dest_id).await?;

    let transcode = TranscodeJob {
        id: job_id,
        tenant_id: ctx.tenant_id,
        asset_id: id,
        source_key: source.original_key,
        output_prefix: String::new(),
        ladder: Ladder::default_abr(),
        hls: false,
        dash: false,
        thumbnails: false,
        encrypt: false,
        encryption_key: None,
        clip: Some(Clip {
            start_secs: body.start,
            end_secs: body.end,
            dest_asset_id: dest_id,
            dest_key,
        }),
        shorts: None,
        mp4: false,
        audio: false,
        captions: false,
        watermark: None,
    };
    state.queue().enqueue(&transcode).await?;
    tracing::info!(job = %job_id, source = %id, dest = %dest_id, "clip enqueued");

    Ok(Json(ClipResponse {
        asset: asset_view(&state, ctx.tenant_id, dest),
        job_id,
    }))
}

#[derive(Deserialize)]
pub struct ShortsRequest {
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Serialize)]
pub struct ShortsResponse {
    pub job_id: Uuid,
}

/// `POST /v1/assets/{id}/shorts` — generate AI vertical shorts from a source.
/// Produced shorts land as new assets once the job completes.
pub async fn shorts_asset(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
    Json(body): Json<ShortsRequest>,
) -> ApiResult<Json<ShortsResponse>> {
    let source = db::find_asset(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if source.status != "ready" {
        return Err(ApiError::BadRequest("source asset is not ready".into()));
    }
    let count = body.count.unwrap_or(3).clamp(1, 10);

    let job_id = Uuid::new_v4();
    db::create_shorts_job(state.db(), ctx.tenant_id, job_id, id).await?;

    let transcode = TranscodeJob {
        id: job_id,
        tenant_id: ctx.tenant_id,
        asset_id: id,
        source_key: source.original_key,
        output_prefix: format!("{}/outputs/{job_id}", ctx.tenant_id),
        ladder: Ladder::default_abr(),
        hls: false,
        dash: false,
        thumbnails: false,
        encrypt: false,
        encryption_key: None,
        clip: None,
        shorts: Some(ShortsSpec { count }),
        mp4: false,
        audio: false,
        captions: false,
        watermark: None,
    };
    state.queue().enqueue(&transcode).await?;
    tracing::info!(job = %job_id, source = %id, count, "shorts enqueued");

    Ok(Json(ShortsResponse { job_id }))
}

/// Object-storage key for a source upload. Filename is sanitized to a basename
/// so a client can't smuggle path traversal into the key.
fn source_key(tenant_id: Uuid, asset_id: Uuid, filename: &str) -> String {
    let safe = filename
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("upload");
    format!("{tenant_id}/sources/{asset_id}/{safe}")
}
