//! Tenant view (`/me`) and API-key management. Tenant creation happens only via
//! signup (`/v1/auth/signup`) — there is no open bootstrap endpoint.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::{self, TenantContext};
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct TenantView {
    pub id: Uuid,
    pub name: String,
    pub plan: String,
}

#[derive(Deserialize, Validate)]
pub struct CreateApiKeyRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub id: Uuid,
    pub prefix: String,
    pub api_key: String,
}

/// `POST /v1/api-keys` — issue a new key for the caller's tenant.
pub async fn create_api_key(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<CreateApiKeyRequest>,
) -> ApiResult<Json<CreateApiKeyResponse>> {
    body.validate().map_err(ApiError::Validation)?;

    let key = auth::generate_key();
    let id = db::create_api_key(
        state.db(),
        ctx.tenant_id,
        &body.name,
        &key.hash,
        &key.prefix,
    )
    .await?;

    Ok(Json(CreateApiKeyResponse {
        id,
        prefix: key.prefix,
        api_key: key.plaintext,
    }))
}

#[derive(Serialize)]
pub struct ApiKeyView {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub last_used_at: Option<String>,
    pub revoked: bool,
    pub created_at: String,
}

/// `GET /v1/api-keys` — list the tenant's keys (active and revoked).
pub async fn list_api_keys(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<ApiKeyView>>> {
    let keys = db::list_api_keys(state.db(), ctx.tenant_id).await?;
    let views = keys
        .into_iter()
        .map(|k| ApiKeyView {
            id: k.id,
            name: k.name,
            prefix: k.prefix,
            last_used_at: k.last_used_at.map(|t| t.to_rfc3339()),
            revoked: k.revoked_at.is_some(),
            created_at: k.created_at.to_rfc3339(),
        })
        .collect();
    Ok(Json(views))
}

/// `DELETE /v1/api-keys/{id}` — revoke a key (owner only).
pub async fn revoke_api_key(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    if !ctx.is_owner() {
        return Err(ApiError::Forbidden);
    }
    let revoked = db::revoke_api_key(state.db(), ctx.tenant_id, id).await?;
    if !revoked {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /v1/me` — return the authenticated tenant (verifies auth works).
pub async fn me(State(state): State<AppState>, ctx: TenantContext) -> ApiResult<Json<TenantView>> {
    let tenant = db::find_tenant(state.db(), ctx.tenant_id)
        .await?
        .ok_or(ApiError::NotFound)?;

    Ok(Json(TenantView {
        id: tenant.id,
        name: tenant.name,
        plan: tenant.plan,
    }))
}
