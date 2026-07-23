//! Playback analytics: a public beacon the embed player posts to, plus a
//! tenant-scoped summary and embed-code generation.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::TenantContext;
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::routes::playback;
use crate::state::AppState;

/// Embed playback tokens live long — an embed shouldn't rot after a few hours.
const EMBED_TOKEN_TTL_SECS: u64 = 365 * 24 * 60 * 60;

#[derive(Deserialize)]
pub struct Beacon {
    pub job: Uuid,
    pub token: String,
    pub session: String,
    pub kind: String,
    #[serde(default)]
    pub position: f64,
    #[serde(default)]
    pub watched: f64,
}

/// `POST /playback/beacon` — public; the player attributes the event via its
/// signed playback token (no auth header, works from a cross-origin iframe).
pub async fn beacon(State(state): State<AppState>, Json(b): Json<Beacon>) -> StatusCode {
    let Some((tenant, job)) = playback::verify_token(&state.settings().playback_secret, &b.token)
    else {
        return StatusCode::FORBIDDEN;
    };
    if job != b.job || !matches!(b.kind.as_str(), "view" | "heartbeat" | "ended") {
        return StatusCode::BAD_REQUEST;
    }
    // Clamp so a hostile embed can't inflate watch-time arbitrarily per beacon.
    let watched = b.watched.clamp(0.0, 60.0);
    match db::insert_playback_event(
        state.db(),
        tenant,
        job,
        &b.session,
        &b.kind,
        b.position,
        watched,
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[derive(Serialize)]
pub struct AnalyticsView {
    pub views: i64,
    pub watch_seconds: f64,
    pub avg_view_seconds: f64,
    pub completions: i64,
    pub completion_rate: f64,
}

/// `GET /v1/jobs/{id}/analytics` — playback summary for the job's owner.
pub async fn job_analytics(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<AnalyticsView>> {
    db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let (views, watch_seconds, completions) =
        db::job_analytics(state.db(), ctx.tenant_id, id).await?;
    let avg = if views > 0 {
        watch_seconds / views as f64
    } else {
        0.0
    };
    let rate = if views > 0 {
        completions as f64 / views as f64
    } else {
        0.0
    };
    Ok(Json(AnalyticsView {
        views,
        watch_seconds,
        avg_view_seconds: avg,
        completions,
        completion_rate: rate,
    }))
}

#[derive(Serialize)]
pub struct EmbedView {
    pub embed_url: String,
    pub iframe: String,
}

/// `GET /v1/jobs/{id}/embed` — a long-lived embed URL + ready-to-paste iframe.
pub async fn job_embed(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<EmbedView>> {
    let job = db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if job.state != "completed" {
        return Err(ApiError::Conflict("job is not completed yet".into()));
    }
    let s = state.settings();
    let exp = playback::now_unix() + EMBED_TOKEN_TTL_SECS;
    let token = playback::sign_token(&s.playback_secret, ctx.tenant_id, id, exp);
    let base = s.app_base_url.trim_end_matches('/');
    let cc = if job.has_captions { "&cc=1" } else { "" };
    let embed_url = format!("{base}/embed/{id}?token={token}{cc}");
    let iframe = format!(
        "<iframe src=\"{embed_url}\" width=\"360\" height=\"640\" frameborder=\"0\" \
         allow=\"autoplay; fullscreen; picture-in-picture\" allowfullscreen></iframe>"
    );
    Ok(Json(EmbedView { embed_url, iframe }))
}
