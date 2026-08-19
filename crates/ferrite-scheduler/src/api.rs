//! The internal API. Not public — between our services only.
//!
//! Four routes come from docs/04-api-and-cli.md. Two more are here because
//! `sched_db` cannot read `assets_db`: budgets have to be pushed in, and
//! something has to report a workflow finishing so the slot comes back.

use crate::engine::WorkflowEngine;
use crate::model::{Lane, NewWork, TenantId, WorkId, WorkItem, WorkState};
use crate::store::{Store, StoreError, Submitted};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// What the handlers share.
#[derive(Debug, Clone)]
pub struct ApiState {
    /// The queue.
    pub store: Store,
    /// Whatever runs the work.
    pub engine: Arc<dyn WorkflowEngine>,
    /// Total slots, so `/capacity` can report utilization.
    pub total_slots: u32,
}

/// The internal router.
pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/internal/work", post(submit).get(list))
        .route("/internal/work/{id}", get(fetch))
        .route("/internal/work/{id}/cancel", post(cancel))
        .route("/internal/work/{id}/finish", post(finish))
        .route("/internal/budgets/{tenant_id}", put(put_budget))
        .route("/internal/capacity", get(capacity))
        .route("/internal/health", get(health))
        .with_state(state)
}

/// An error the caller can act on.
#[derive(Debug, Serialize)]
pub struct ApiError {
    /// Machine-readable code.
    pub error: &'static str,
    /// Human-readable detail.
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.error {
            "not_found" => StatusCode::NOT_FOUND,
            "no_budget" => StatusCode::UNPROCESSABLE_ENTITY,
            "suspended" => StatusCode::FORBIDDEN,
            "bad_request" => StatusCode::BAD_REQUEST,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        let error = match e {
            StoreError::NoBudget(_) => "no_budget",
            StoreError::Suspended(_) => "suspended",
            StoreError::Corrupt { .. } | StoreError::Db(_) => "internal",
        };
        Self {
            error,
            message: e.to_string(),
        }
    }
}

type ApiResult<T> = Result<T, ApiError>;

/// `POST /internal/work`. Idempotent on `(tenant_id, dedupe_key)`.
async fn submit(
    State(state): State<ApiState>,
    Json(new): Json<NewWork>,
) -> ApiResult<(StatusCode, Json<WorkItem>)> {
    let submitted = state.store.submit(&new).await?;
    // 200 rather than 201 on a duplicate, so a retrying caller can tell whether
    // it created the item without having to compare timestamps.
    let status = match submitted {
        Submitted::Created(_) => StatusCode::CREATED,
        Submitted::Existing(_) => StatusCode::OK,
    };
    Ok((status, Json(submitted.item().clone())))
}

/// `GET /internal/work/{id}`.
async fn fetch(State(state): State<ApiState>, Path(id): Path<WorkId>) -> ApiResult<Json<WorkItem>> {
    state
        .store
        .get(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError {
            error: "not_found",
            message: format!("no work {id}"),
        })
}

/// Filters for `GET /internal/work`.
#[derive(Debug, Default, Deserialize)]
pub struct ListQuery {
    /// Only this tenant's work.
    pub tenant_id: Option<TenantId>,
    /// Only work in this state.
    pub state: Option<String>,
    /// How many rows at most.
    pub limit: Option<i64>,
}

/// `GET /internal/work`.
async fn list(
    State(state): State<ApiState>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<WorkItem>>> {
    let filter = q
        .state
        .as_deref()
        .map(str::parse::<WorkState>)
        .transpose()
        .map_err(|e| ApiError {
            error: "bad_request",
            message: e.to_string(),
        })?;

    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    Ok(Json(state.store.list(q.tenant_id, filter, limit).await?))
}

/// `POST /internal/work/{id}/cancel`. Reaches the running workflow.
async fn cancel(
    State(state): State<ApiState>,
    Path(id): Path<WorkId>,
) -> ApiResult<Json<WorkItem>> {
    let item = state.store.get(id).await?.ok_or_else(|| ApiError {
        error: "not_found",
        message: format!("no work {id}"),
    })?;

    if item.state.is_terminal() {
        return Ok(Json(item));
    }
    // Stop the workflow before the row changes: a slot released while the
    // encode is still burning cores is the failure this is guarding against.
    if let Some(workflow_id) = &item.workflow_id
        && let Err(e) = state.engine.cancel(workflow_id).await
    {
        return Err(ApiError {
            error: "internal",
            message: e.to_string(),
        });
    }

    state.store.finish(id, WorkState::Canceled, None).await?;
    fetch(State(state), Path(id)).await
}

/// Body for `POST /internal/work/{id}/finish`.
#[derive(Debug, Deserialize)]
pub struct FinishBody {
    /// `done` or `failed`.
    pub state: String,
    /// Why, when it failed.
    #[serde(default)]
    pub error: Option<String>,
    /// CPU seconds this task consumed.
    #[serde(default)]
    pub cpu_seconds: f64,
    /// Bytes it wrote.
    #[serde(default)]
    pub bytes_written: i64,
    /// Which machine ran it.
    #[serde(default)]
    pub machine: Option<String>,
}

/// `POST /internal/work/{id}/finish`. Releases the slot and records the cost.
async fn finish(
    State(state): State<ApiState>,
    Path(id): Path<WorkId>,
    Json(body): Json<FinishBody>,
) -> ApiResult<Json<WorkItem>> {
    let finished = body
        .state
        .parse::<WorkState>()
        .ok()
        .filter(|s| matches!(s, WorkState::Done | WorkState::Failed))
        .ok_or_else(|| ApiError {
            error: "bad_request",
            message: format!("finish state must be done or failed, got {:?}", body.state),
        })?;

    if body.cpu_seconds > 0.0 || body.bytes_written > 0 {
        state
            .store
            .record_cost(
                id,
                body.cpu_seconds,
                body.bytes_written,
                body.machine.as_deref().unwrap_or("unknown"),
            )
            .await?;
    }

    state
        .store
        .finish(id, finished, body.error.as_deref())
        .await?;
    fetch(State(state), Path(id)).await
}

/// Body for `PUT /internal/budgets/{tenant_id}`.
#[derive(Debug, Deserialize)]
pub struct BudgetBody {
    /// Ceiling on items holding a slot at once.
    pub max_concurrent_tasks: i32,
    /// Ceiling on admissions per minute.
    pub rate_limit_per_min: i32,
    /// Premium 5.0, free 1.0.
    pub fairness_weight: f32,
    /// Suspension stops admission without touching the queue.
    #[serde(default)]
    pub suspended: bool,
}

/// `PUT /internal/budgets/{tenant_id}`. Pushed from `ferrite-assets` on a plan
/// change, because `sched_db` cannot read `assets_db`.
async fn put_budget(
    State(state): State<ApiState>,
    Path(tenant_id): Path<TenantId>,
    Json(body): Json<BudgetBody>,
) -> ApiResult<StatusCode> {
    state
        .store
        .upsert_budget(
            tenant_id,
            body.max_concurrent_tasks,
            body.rate_limit_per_min,
            body.fairness_weight,
        )
        .await?;
    state.store.set_suspended(tenant_id, body.suspended).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// One lane's line in `ferrite capacity`.
#[derive(Debug, Serialize)]
pub struct LaneReport {
    /// Which lane.
    pub lane: Lane,
    /// Items pending.
    pub waiting: u32,
    /// How long the oldest has waited. Alert on age, not depth.
    pub oldest_wait_seconds: i64,
    /// Items holding a slot.
    pub running: u32,
}

/// `GET /internal/capacity`.
#[derive(Debug, Serialize)]
pub struct CapacityReport {
    /// Per-lane depths and ages.
    pub lanes: Vec<LaneReport>,
    /// Total slots across the fleet.
    pub total_slots: u32,
    /// Slots held right now.
    pub running: u32,
    /// Fraction of the fleet busy. Target is above 0.70.
    pub utilization: f64,
}

async fn capacity(State(state): State<ApiState>) -> ApiResult<Json<CapacityReport>> {
    let now = Utc::now();
    let stats = state.store.lane_stats().await?;

    let mut lanes: Vec<LaneReport> = Lane::ALL
        .into_iter()
        .map(|lane| {
            let s = stats.iter().find(|s| s.lane == lane);
            LaneReport {
                lane,
                waiting: s.map_or(0, |s| s.waiting),
                oldest_wait_seconds: s.map_or(0, |s| s.oldest_wait(now).num_seconds()),
                running: s.map_or(0, |s| s.running),
            }
        })
        .collect();
    lanes.sort_by_key(|l| l.lane);

    let running: u32 = lanes.iter().map(|l| l.running).sum();
    Ok(Json(CapacityReport {
        lanes,
        total_slots: state.total_slots,
        running,
        utilization: if state.total_slots == 0 {
            0.0
        } else {
            f64::from(running) / f64::from(state.total_slots)
        },
    }))
}

async fn health(State(state): State<ApiState>) -> ApiResult<Json<serde_json::Value>> {
    sqlx::query("SELECT 1")
        .execute(state.store.pool())
        .await
        .map_err(StoreError::from)?;
    Ok(Json(serde_json::json!({"status": "ok"})))
}
