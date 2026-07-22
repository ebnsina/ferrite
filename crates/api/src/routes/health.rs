//! Liveness and readiness probes.
//!
//! - `GET /health` — liveness: the process is up. No dependencies touched.
//! - `GET /ready`  — readiness: dependencies (Postgres, Redis) are reachable.
//!   Returns 503 with per-dependency detail when something is down.

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let db_ok = sqlx::query("SELECT 1").execute(state.db()).await.is_ok();

    // A cheap round-trip proves Redis connectivity.
    let redis_ok = state.queue().ping().await.is_ok();

    let all_ok = db_ok && redis_ok;
    let status = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = json!({
        "status": if all_ok { "ready" } else { "degraded" },
        "checks": {
            "database": if db_ok { "up" } else { "down" },
            "redis": if redis_ok { "up" } else { "down" },
        }
    });

    (status, Json(body))
}
