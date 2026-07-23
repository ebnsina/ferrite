//! Platform superadmin endpoints (cross-tenant). Gated on the superadmin claim.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::auth::TenantContext;
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

fn require_super(ctx: &TenantContext) -> ApiResult<()> {
    if ctx.superadmin {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

#[derive(Serialize)]
pub struct Overview {
    pub tenants: i64,
    pub users: i64,
    pub assets: i64,
    pub jobs: i64,
    pub waitlist: i64,
}

/// `GET /v1/admin/overview` — platform-wide counts.
pub async fn overview(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Overview>> {
    require_super(&ctx)?;
    let (tenants, users, assets, jobs, waitlist) = db::platform_overview(state.db()).await?;
    Ok(Json(Overview {
        tenants,
        users,
        assets,
        jobs,
        waitlist,
    }))
}

#[derive(Serialize)]
pub struct WaitlistRow {
    pub id: i64,
    pub name: String,
    pub email: String,
    pub whatsapp: Option<String>,
    pub country: Option<String>,
    pub use_case: Option<String>,
    pub volume: Option<String>,
    pub plan: Option<String>,
    pub payment: Option<String>,
    pub created_at: String,
}

/// `GET /v1/admin/waitlist` — all early-access signups.
pub async fn waitlist(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<WaitlistRow>>> {
    require_super(&ctx)?;
    let rows = db::list_waitlist(state.db()).await?;
    Ok(Json(
        rows.into_iter()
            .map(|w| WaitlistRow {
                id: w.id,
                name: w.name,
                email: w.email,
                whatsapp: w.whatsapp,
                country: w.country,
                use_case: w.use_case,
                volume: w.volume,
                plan: w.plan,
                payment: w.payment,
                created_at: w.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}
