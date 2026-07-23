//! Dashboard authentication: sign up (creates a workspace + owner) and log in.

use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::StatusCode;
use axum::response::{AppendHeaders, IntoResponse};
use axum::Json;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::{self, SESSION_TTL_SECS};
use crate::cookies;
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Set the HttpOnly session cookie + the readable CSRF cookie for a new session.
fn session_cookies(
    state: &AppState,
    token: &str,
) -> AppendHeaders<[(axum::http::HeaderName, String); 2]> {
    let secure = state.settings().cookie_secure();
    AppendHeaders([
        (
            SET_COOKIE,
            cookies::set_session(token, secure, SESSION_TTL_SECS),
        ),
        (
            SET_COOKIE,
            cookies::set_csrf(&cookies::random_token(), secure, SESSION_TTL_SECS),
        ),
    ])
}

#[derive(Serialize)]
pub struct UserView {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub superadmin: bool,
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
/// The JWT is delivered in an HttpOnly cookie (browsers) and echoed in the body
/// (non-browser clients); a CSRF cookie is set alongside.
pub async fn signup(
    State(state): State<AppState>,
    Json(body): Json<SignupRequest>,
) -> ApiResult<impl IntoResponse> {
    body.validate().map_err(ApiError::Validation)?;
    let email = body.email.trim().to_lowercase();

    if db::find_user_by_email(state.db(), &email).await?.is_some() {
        return Err(ApiError::Conflict("email already registered".into()));
    }

    let tenant = db::create_tenant(state.db(), body.workspace.trim()).await?;
    let user_id = Uuid::new_v4();
    let hash = auth::hash_password(&body.password)?;
    db::create_user(state.db(), user_id, tenant.id, &email, &hash, "owner", None).await?;

    let superadmin = auth::is_superadmin(&state.settings().superadmin_emails, &email);
    let token = auth::issue_session(
        &state.settings().auth_secret,
        user_id,
        tenant.id,
        "owner",
        superadmin,
    );
    let cookies = session_cookies(&state, &token);
    Ok((
        cookies,
        Json(AuthResponse {
            token,
            user: UserView {
                id: user_id,
                email,
                name: None,
                role: "owner".into(),
                superadmin,
            },
            tenant: TenantView {
                id: tenant.id,
                name: tenant.name,
            },
        }),
    ))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// `POST /v1/auth/login` — verify credentials, return a session (HttpOnly cookie
/// + CSRF cookie; JWT also echoed in the body for non-browser clients).
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> ApiResult<impl IntoResponse> {
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

    let superadmin = auth::is_superadmin(&state.settings().superadmin_emails, &email);
    let token = auth::issue_session(
        &state.settings().auth_secret,
        user.id,
        user.tenant_id,
        &user.role,
        superadmin,
    );
    let cookies = session_cookies(&state, &token);
    Ok((
        cookies,
        Json(AuthResponse {
            token,
            user: UserView {
                id: user.id,
                email,
                name: user.name,
                role: user.role,
                superadmin,
            },
            tenant: TenantView {
                id: tenant.id,
                name: tenant.name,
            },
        }),
    ))
}

/// `POST /v1/auth/logout` — expire the session + CSRF cookies. Unauthenticated
/// and idempotent (clearing cookies changes no server state).
pub async fn logout(State(state): State<AppState>) -> impl IntoResponse {
    let secure = state.settings().cookie_secure();
    (
        AppendHeaders([
            (SET_COOKIE, cookies::clear(cookies::SESSION_COOKIE, secure)),
            (SET_COOKIE, cookies::clear(cookies::CSRF_COOKIE, secure)),
        ]),
        StatusCode::NO_CONTENT,
    )
}

// --- Password reset ----------------------------------------------------------

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// `POST /v1/auth/forgot-password` — email a reset link if the account exists.
/// Always returns 204 so callers can't probe which emails are registered.
pub async fn forgot_password(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordRequest>,
) -> ApiResult<StatusCode> {
    let email = body.email.trim().to_lowercase();
    if let Some(user) = db::find_user_by_email(state.db(), &email).await? {
        let token = reset_token();
        let expires = chrono::Utc::now() + chrono::Duration::hours(1);
        db::create_password_reset(state.db(), &token, user.id, expires).await?;
        state.mailer().send_password_reset(&email, &token).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, Validate)]
pub struct ResetPasswordRequest {
    pub token: String,
    #[validate(length(min = 8, message = "password must be at least 8 characters"))]
    pub new_password: String,
}

/// `POST /v1/auth/reset-password` — set a new password using a valid token.
pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordRequest>,
) -> ApiResult<StatusCode> {
    body.validate().map_err(ApiError::Validation)?;
    let user_id = db::find_valid_reset(state.db(), &body.token)
        .await?
        .ok_or_else(|| ApiError::BadRequest("invalid or expired reset link".into()))?;
    let hash = auth::hash_password(&body.new_password)?;
    db::update_user_password(state.db(), user_id, &hash).await?;
    db::mark_reset_used(state.db(), &body.token).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn reset_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
