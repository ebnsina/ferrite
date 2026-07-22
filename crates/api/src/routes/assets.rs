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
}

impl From<Asset> for AssetView {
    fn from(a: Asset) -> Self {
        AssetView {
            id: a.id,
            filename: a.filename,
            bytes: a.bytes,
            status: a.status,
            created_at: a.created_at.to_rfc3339(),
        }
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
        asset: asset.into(),
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
    Ok(Json(asset.into()))
}

/// `GET /v1/assets` — list the tenant's assets.
pub async fn list_assets(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<AssetView>>> {
    let assets = db::list_assets(state.db(), ctx.tenant_id).await?;
    Ok(Json(assets.into_iter().map(AssetView::from).collect()))
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
    Ok(Json(asset.into()))
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
