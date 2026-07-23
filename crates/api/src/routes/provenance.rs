//! Content provenance: serve an asset's signed manifest and verify it —
//! signature validity (authenticity) + content hash match (tamper-evidence).

use axum::extract::{Path, State};
use axum::Json;
use ferrite_core::provenance;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::auth::TenantContext;
use crate::db;
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ProvenanceView {
    pub manifest: Value,
    pub signature: String,
    pub algorithm: &'static str,
    pub public_key: String,
    /// The signature is valid for this deployment's key.
    pub signature_valid: bool,
    /// The current stored file still matches the signed hash (not tampered).
    pub content_matches: bool,
    /// Both checks passed.
    pub verified: bool,
}

/// `GET /v1/assets/{id}/provenance` — the asset's content credentials, verified.
pub async fn get_asset_provenance(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ProvenanceView>> {
    let (manifest_json, signature) = db::get_provenance(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let secret = state.settings().provenance_secret.as_deref();
    let signature_valid = secret
        .map(|s| provenance::verify(s, &manifest_json, &signature))
        .unwrap_or(false);
    let public_key = secret.map(provenance::public_key_hex).unwrap_or_default();

    // Re-hash the current stored file and compare to the signed hash.
    let manifest: Value = serde_json::from_str(&manifest_json).unwrap_or(Value::Null);
    let signed_hash = manifest
        .get("sha256")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let content_matches = match db::find_asset(state.db(), ctx.tenant_id, id).await? {
        Some(a) => match state.storage().get_bytes(&a.original_key).await {
            Ok(bytes) => provenance::sha256_hex(&bytes) == signed_hash,
            Err(_) => false,
        },
        None => false,
    };

    Ok(Json(ProvenanceView {
        manifest,
        signature,
        algorithm: "ed25519",
        public_key,
        signature_valid,
        content_matches,
        verified: signature_valid && content_matches,
    }))
}

#[derive(Serialize)]
pub struct PublicKeyView {
    pub public_key: String,
    pub algorithm: &'static str,
}

/// `GET /v1/provenance/key` — the deployment's public verification key.
pub async fn public_key(State(state): State<AppState>) -> Json<PublicKeyView> {
    let public_key = state
        .settings()
        .provenance_secret
        .as_deref()
        .map(provenance::public_key_hex)
        .unwrap_or_default();
    Json(PublicKeyView {
        public_key,
        algorithm: "ed25519",
    })
}
