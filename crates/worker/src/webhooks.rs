//! Deliver signed job-event callbacks to a tenant's registered webhooks.
//!
//! Best-effort with a few retries; each request carries an HMAC-SHA256 signature
//! (`X-Ferrite-Signature: sha256=<hex>`) over the raw body so receivers can verify
//! authenticity. A durable delivery queue is a future hardening step.

use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use serde_json::json;
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;

type HmacSha256 = Hmac<Sha256>;
const MAX_ATTEMPTS: usize = 3;

/// Deliver `event` for a job to all subscribed webhooks. Never fails the pipeline.
pub async fn deliver(pool: &PgPool, tenant_id: Uuid, event: &str, job_id: Uuid, asset_id: Uuid) {
    let hooks = match db::webhooks_for_event(pool, tenant_id, event).await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(error = %e, "failed to load webhooks");
            return;
        }
    };
    if hooks.is_empty() {
        return;
    }

    let body = json!({
        "event": event,
        "job_id": job_id,
        "asset_id": asset_id,
    })
    .to_string();

    let client = reqwest::Client::new();
    for (url, secret) in hooks {
        let signature = sign(&secret, &body);
        deliver_one(&client, &url, &body, &signature, job_id).await;
    }
}

async fn deliver_one(
    client: &reqwest::Client,
    url: &str,
    body: &str,
    signature: &str,
    job_id: Uuid,
) {
    for attempt in 1..=MAX_ATTEMPTS {
        let result = client
            .post(url)
            .header("Content-Type", "application/json")
            .header("X-Ferrite-Signature", format!("sha256={signature}"))
            .body(body.to_string())
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(resp) if resp.status().is_success() => return,
            Ok(resp) => {
                tracing::warn!(job = %job_id, url, status = %resp.status(), attempt, "webhook non-2xx")
            }
            Err(e) => {
                tracing::warn!(job = %job_id, url, error = %e, attempt, "webhook delivery failed")
            }
        }
        tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
    }
}

fn sign(secret: &str, body: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
