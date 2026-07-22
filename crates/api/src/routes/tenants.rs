//! Tenant bootstrap and API-key management.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::auth::{self, TenantContext};
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Deserialize, Validate)]
pub struct CreateTenantRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
}

#[derive(Serialize)]
pub struct TenantView {
    pub id: uuid::Uuid,
    pub name: String,
    pub plan: String,
}

#[derive(Serialize)]
pub struct CreateTenantResponse {
    pub tenant: TenantView,
    /// Shown exactly once — store it now, it cannot be retrieved again.
    pub api_key: String,
}

/// `POST /v1/tenants` — bootstrap a tenant and its first API key.
///
/// NOTE: open in dev for convenience. Before production this must be gated
/// behind an admin credential or invite flow.
pub async fn create_tenant(
    State(state): State<AppState>,
    Json(body): Json<CreateTenantRequest>,
) -> ApiResult<Json<CreateTenantResponse>> {
    body.validate().map_err(ApiError::Validation)?;

    let tenant = db::create_tenant(state.db(), &body.name).await?;
    let key = auth::generate_key();
    db::create_api_key(state.db(), tenant.id, "default", &key.hash, &key.prefix).await?;

    Ok(Json(CreateTenantResponse {
        tenant: TenantView {
            id: tenant.id,
            name: tenant.name,
            plan: tenant.plan,
        },
        api_key: key.plaintext,
    }))
}

#[derive(Deserialize, Validate)]
pub struct CreateApiKeyRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
}

#[derive(Serialize)]
pub struct CreateApiKeyResponse {
    pub id: uuid::Uuid,
    pub prefix: String,
    pub api_key: String,
}

/// `POST /v1/api-keys` — issue an additional key for the caller's tenant.
pub async fn create_api_key(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<CreateApiKeyRequest>,
) -> ApiResult<Json<CreateApiKeyResponse>> {
    body.validate().map_err(ApiError::Validation)?;

    let key = auth::generate_key();
    let id = db::create_api_key(state.db(), ctx.tenant_id, &body.name, &key.hash, &key.prefix)
        .await?;

    Ok(Json(CreateApiKeyResponse {
        id,
        prefix: key.prefix,
        api_key: key.plaintext,
    }))
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
