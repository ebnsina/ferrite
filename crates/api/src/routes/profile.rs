//! Self-service profile: view, rename, and change password. Session users only
//! (API keys have no user identity).

use axum::extract::State;
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
pub struct ProfileView {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
}

impl From<db::Profile> for ProfileView {
    fn from(p: db::Profile) -> Self {
        ProfileView {
            id: p.id,
            email: p.email,
            name: p.name,
            role: p.role,
        }
    }
}

fn user_id(ctx: &TenantContext) -> ApiResult<Uuid> {
    ctx.user_id.ok_or(ApiError::Forbidden)
}

/// `GET /v1/profile` — the caller's own profile.
pub async fn get_profile(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<ProfileView>> {
    let id = user_id(&ctx)?;
    let profile = db::find_profile(state.db(), id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(profile.into()))
}

#[derive(Deserialize, Validate)]
pub struct UpdateProfileRequest {
    #[validate(length(min = 1, max = 80, message = "name must be 1–80 characters"))]
    pub name: String,
}

/// `PATCH /v1/profile` — update the display name.
pub async fn update_profile(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<UpdateProfileRequest>,
) -> ApiResult<Json<ProfileView>> {
    let id = user_id(&ctx)?;
    body.validate().map_err(ApiError::Validation)?;
    db::update_user_name(state.db(), id, body.name.trim()).await?;
    let profile = db::find_profile(state.db(), id)
        .await?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(profile.into()))
}

#[derive(Deserialize, Validate)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub new_password: String,
}

/// `POST /v1/profile/password` — change password after verifying the current one.
pub async fn change_password(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<ChangePasswordRequest>,
) -> ApiResult<StatusCode> {
    let id = user_id(&ctx)?;
    body.validate().map_err(ApiError::Validation)?;

    let profile = db::find_profile(state.db(), id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if !auth::verify_password(&body.current_password, &profile.password_hash) {
        return Err(ApiError::BadRequest("current password is incorrect".into()));
    }
    let hash = auth::hash_password(&body.new_password)?;
    db::update_user_password(state.db(), id, &hash).await?;
    Ok(StatusCode::NO_CONTENT)
}
