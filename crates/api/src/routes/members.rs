//! Team management: list members and invite new ones (owner only).
//!
//! Without email infra, an invite creates the member with a one-time temporary
//! password that the owner shares; the member logs in and can change it later.

use axum::extract::State;
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
    pub role: String,
    pub created_at: String,
}

impl From<Member> for MemberView {
    fn from(m: Member) -> Self {
        MemberView {
            id: m.id,
            email: m.email,
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
    db::create_user(state.db(), id, ctx.tenant_id, &email, &hash, &body.role).await?;

    let members = db::list_members(state.db(), ctx.tenant_id).await?;
    let member = members
        .into_iter()
        .find(|m| m.id == id)
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
