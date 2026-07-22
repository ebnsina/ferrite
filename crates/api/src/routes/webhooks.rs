//! Webhook management. Job events (`job.completed`, `job.failed`) are delivered
//! to registered URLs by the worker, signed with the webhook's secret.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::TenantContext;
use crate::db::{self, Webhook};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

const EVENTS: [&str; 2] = ["job.completed", "job.failed"];

#[derive(Serialize)]
pub struct WebhookView {
    pub id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub created_at: String,
}

impl From<Webhook> for WebhookView {
    fn from(w: Webhook) -> Self {
        WebhookView {
            id: w.id,
            url: w.url,
            events: w.events,
            created_at: w.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, Validate)]
pub struct CreateWebhookRequest {
    #[validate(url)]
    pub url: String,
    /// Subscribed events; empty means all known events.
    #[serde(default)]
    pub events: Vec<String>,
}

#[derive(Serialize)]
pub struct CreateWebhookResponse {
    #[serde(flatten)]
    pub webhook: WebhookView,
    /// Shown once — used to verify the `X-Ferrite-Signature` HMAC.
    pub secret: String,
}

/// `POST /v1/webhooks` — register a callback URL.
pub async fn create_webhook(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<CreateWebhookRequest>,
) -> ApiResult<Json<CreateWebhookResponse>> {
    body.validate().map_err(ApiError::Validation)?;

    let events = if body.events.is_empty() {
        EVENTS.iter().map(|e| e.to_string()).collect()
    } else {
        for e in &body.events {
            if !EVENTS.contains(&e.as_str()) {
                return Err(ApiError::BadRequest(format!("unknown event: {e}")));
            }
        }
        body.events
    };

    let secret = generate_secret();
    let webhook =
        db::create_webhook(state.db(), ctx.tenant_id, &body.url, &secret, &events).await?;
    Ok(Json(CreateWebhookResponse {
        webhook: webhook.into(),
        secret,
    }))
}

/// `GET /v1/webhooks` — list registered webhooks (secrets omitted).
pub async fn list_webhooks(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<WebhookView>>> {
    let hooks = db::list_webhooks(state.db(), ctx.tenant_id).await?;
    Ok(Json(hooks.into_iter().map(WebhookView::from).collect()))
}

/// `DELETE /v1/webhooks/:id`.
pub async fn delete_webhook(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    if db::delete_webhook(state.db(), ctx.tenant_id, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

fn generate_secret() -> String {
    let mut bytes = [0u8; 24];
    rand::rng().fill_bytes(&mut bytes);
    format!("whsec_{}", hex::encode(bytes))
}
