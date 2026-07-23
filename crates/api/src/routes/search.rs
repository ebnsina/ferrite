//! In-video search: full-text over indexed transcript segments. Each hit
//! deep-links to the exact moment. Semantic (vector) search plugs in here later.

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::TenantContext;
use crate::db;
use crate::error::ApiResult;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize)]
pub struct SearchHitView {
    pub asset_id: Uuid,
    pub filename: String,
    pub job_id: Uuid,
    pub start_secs: f64,
    pub snippet: String,
}

/// `GET /v1/search?q=` — search across the tenant's video transcripts.
pub async fn search(
    State(state): State<AppState>,
    ctx: TenantContext,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<Vec<SearchHitView>>> {
    let term = q.q.trim();
    if term.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let hits = db::search_transcripts(state.db(), ctx.tenant_id, term).await?;
    Ok(Json(
        hits.into_iter()
            .map(|h| SearchHitView {
                asset_id: h.asset_id,
                filename: h.filename,
                job_id: h.job_id,
                start_secs: h.start_secs as f64,
                snippet: h.snippet,
            })
            .collect(),
    ))
}
