//! Dashboard authentication: sign up (creates a workspace + owner) and log in.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth;
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct UserView {
    pub id: Uuid,
    pub email: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct TenantView {
    pub id: Uuid,
    pub name: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserView,
    pub tenant: TenantView,
}

#[derive(Deserialize, Validate)]
pub struct SignupRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub password: String,
    #[validate(length(min = 1, max = 100))]
    pub workspace: String,
}

/// `POST /v1/auth/signup` — create a workspace and its owner, return a session.
pub async fn signup(
    State(state): State<AppState>,
    Json(body): Json<SignupRequest>,
) -> ApiResult<Json<AuthResponse>> {
    body.validate().map_err(ApiError::Validation)?;
    let email = body.email.trim().to_lowercase();

    if db::find_user_by_email(state.db(), &email).await?.is_some() {
        return Err(ApiError::Conflict("email already registered".into()));
    }

    let tenant = db::create_tenant(state.db(), body.workspace.trim()).await?;
    let user_id = Uuid::new_v4();
    let hash = auth::hash_password(&body.password)?;
    db::create_user(state.db(), user_id, tenant.id, &email, &hash, "owner").await?;

    let token = auth::issue_session(&state.settings().auth_secret, user_id, tenant.id, "owner");
    Ok(Json(AuthResponse {
        token,
        user: UserView {
            id: user_id,
            email,
            role: "owner".into(),
        },
        tenant: TenantView {
            id: tenant.id,
            name: tenant.name,
        },
    }))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// `POST /v1/auth/login` — verify credentials, return a session.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Json<AuthResponse>> {
    let email = body.email.trim().to_lowercase();
    let user = db::find_user_by_email(state.db(), &email)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    if !auth::verify_password(&body.password, &user.password_hash) {
        return Err(ApiError::Unauthorized);
    }
    let tenant = db::find_tenant(state.db(), user.tenant_id)
        .await?
        .ok_or(ApiError::Unauthorized)?;

    let token = auth::issue_session(
        &state.settings().auth_secret,
        user.id,
        user.tenant_id,
        &user.role,
    );
    Ok(Json(AuthResponse {
        token,
        user: UserView {
            id: user.id,
            email,
            role: user.role,
        },
        tenant: TenantView {
            id: tenant.id,
            name: tenant.name,
        },
    }))
}
