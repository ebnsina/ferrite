//! Usage metering + mock billing for the current month.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::auth::TenantContext;
use crate::db;
use crate::error::ApiResult;
use crate::state::AppState;

// Mock rates (USD). Real billing plugs in here later.
const RATE_PER_MINUTE: f64 = 0.015;
const RATE_PER_GB_MONTH: f64 = 0.023;

#[derive(Serialize)]
pub struct UsageView {
    pub minutes: f64,
    pub storage_bytes: i64,
    pub storage_gb: f64,
    pub cost: Cost,
}

#[derive(Serialize)]
pub struct Cost {
    pub currency: &'static str,
    pub transcode: f64,
    pub storage: f64,
    pub total: f64,
}

/// `GET /v1/usage` — this month's metered usage and estimated (mock) cost.
pub async fn get_usage(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<UsageView>> {
    let minutes = db::usage_minutes(state.db(), ctx.tenant_id).await?;
    let bytes = db::storage_bytes(state.db(), ctx.tenant_id).await?;
    let gb = bytes as f64 / 1_073_741_824.0;

    let transcode = round2(minutes * RATE_PER_MINUTE);
    let storage = round2(gb * RATE_PER_GB_MONTH);

    Ok(Json(UsageView {
        minutes: round2(minutes),
        storage_bytes: bytes,
        storage_gb: round2(gb),
        cost: Cost {
            currency: "USD",
            transcode,
            storage,
            total: round2(transcode + storage),
        },
    }))
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
