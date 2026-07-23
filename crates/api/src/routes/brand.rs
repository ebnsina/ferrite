//! Tenant brand logo — uploaded once, reused as the watermark source.

use std::time::Duration;

use axum::extract::State;
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use crate::auth::TenantContext;
use crate::error::ApiResult;
use crate::state::AppState;

const UPLOAD_TTL: Duration = Duration::from_secs(15 * 60);
const VIEW_TTL: Duration = Duration::from_secs(60 * 60);

/// Object-storage key of a tenant's brand logo.
pub fn logo_key(tenant_id: Uuid) -> String {
    format!("{tenant_id}/brand/logo.png")
}

#[derive(Serialize)]
pub struct LogoUpload {
    pub upload_url: String,
    pub expires_in_secs: u64,
}

/// `POST /v1/brand/logo` — presigned URL to upload/replace the brand logo.
pub async fn upload_logo(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<LogoUpload>> {
    let url = state
        .storage()
        .presign_put(&logo_key(ctx.tenant_id), UPLOAD_TTL)
        .await?;
    Ok(Json(LogoUpload {
        upload_url: url,
        expires_in_secs: UPLOAD_TTL.as_secs(),
    }))
}

#[derive(Serialize)]
pub struct BrandView {
    /// Presigned URL of the current logo, or null if none uploaded.
    pub logo_url: Option<String>,
}

/// `GET /v1/brand` — the current brand logo (if any).
pub async fn get_brand(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<BrandView>> {
    let key = logo_key(ctx.tenant_id);
    let logo_url = if state.storage().exists(&key).await {
        state.storage().presign_get(&key, VIEW_TTL).await.ok()
    } else {
        None
    };
    Ok(Json(BrandView { logo_url }))
}
