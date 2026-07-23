//! Team management: list, invite, re-role, and remove members (owner only).
//!
//! An invite creates the member with a one-time temporary password, emailed to
//! them (logged in dev) and also returned so the owner can share it manually.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::{self, TenantContext};
use crate::db::{self, Member};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const ROLES: [&str; 2] = ["admin", "member"];

#[derive(Serialize)]
pub struct MemberView {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub created_at: String,
}

impl From<Member> for MemberView {
    fn from(m: Member) -> Self {
        MemberView {
            id: m.id,
            email: m.email,
            name: m.name,
            role: m.role,
            created_at: m.created_at.to_rfc3339(),
        }
    }
}

/// `GET /v1/members` — list the workspace's members.
pub async fn list_members(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<MemberView>>> {
    let members = db::list_members(state.db(), ctx.tenant_id).await?;
    Ok(Json(members.into_iter().map(MemberView::from).collect()))
}

#[derive(Deserialize, Validate)]
pub struct InviteRequest {
    #[validate(email)]
    pub email: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct InviteResponse {
    pub member: MemberView,
    /// One-time temporary password to share with the invitee.
    pub temp_password: String,
}

/// `POST /v1/members` — invite a member (owner only).
pub async fn invite_member(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<InviteRequest>,
) -> ApiResult<Json<InviteResponse>> {
    if !ctx.is_owner() {
        return Err(ApiError::Forbidden);
    }
    body.validate().map_err(ApiError::Validation)?;
    if !ROLES.contains(&body.role.as_str()) {
        return Err(ApiError::BadRequest(format!("invalid role: {}", body.role)));
    }
    let email = body.email.trim().to_lowercase();
    if db::find_user_by_email(state.db(), &email).await?.is_some() {
        return Err(ApiError::Conflict("email already registered".into()));
    }

    let temp_password = temp_password();
    let hash = auth::hash_password(&temp_password)?;
    let id = Uuid::new_v4();
    db::create_user(
        state.db(),
        id,
        ctx.tenant_id,
        &email,
        &hash,
        &body.role,
        None,
    )
    .await?;

    // Email the invite (no-op log in dev); the temp password is also returned
    // so the owner can share it manually when mail isn't configured.
    let workspace = db::find_tenant(state.db(), ctx.tenant_id)
        .await?
        .map(|t| t.name)
        .unwrap_or_else(|| "your team".to_string());
    state
        .mailer()
        .send_invite(&email, &workspace, &temp_password)
        .await;

    let member = db::find_member(state.db(), ctx.tenant_id, id)
        .await?
        .map(MemberView::from)
        .ok_or(ApiError::NotFound)?;
    Ok(Json(InviteResponse {
        member,
        temp_password,
    }))
}

fn temp_password() -> String {
    let mut bytes = [0u8; 9];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[derive(Deserialize, Validate)]
pub struct UpdateMemberRequest {
    pub role: String,
}

/// `PATCH /v1/members/{id}` — change a member's role (owner only).
pub async fn update_member(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateMemberRequest>,
) -> ApiResult<Json<MemberView>> {
    if !ctx.is_owner() {
        return Err(ApiError::Forbidden);
    }
    if !ROLES.contains(&body.role.as_str()) {
        return Err(ApiError::BadRequest(format!("invalid role: {}", body.role)));
    }
    let member = db::find_member(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if member.role == "owner" {
        return Err(ApiError::BadRequest(
            "cannot change the owner's role".into(),
        ));
    }
    db::update_member_role(state.db(), ctx.tenant_id, id, &body.role).await?;
    let updated = db::find_member(state.db(), ctx.tenant_id, id)
        .await?
        .map(MemberView::from)
        .ok_or(ApiError::NotFound)?;
    Ok(Json(updated))
}

/// `DELETE /v1/members/{id}` — remove a member (owner only; not self/owner).
pub async fn remove_member(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    if !ctx.is_owner() {
        return Err(ApiError::Forbidden);
    }
    if ctx.user_id == Some(id) {
        return Err(ApiError::BadRequest("you can't remove yourself".into()));
    }
    let member = db::find_member(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if member.role == "owner" {
        return Err(ApiError::BadRequest("cannot remove the owner".into()));
    }
    db::delete_user(state.db(), ctx.tenant_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
