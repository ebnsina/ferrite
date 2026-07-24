//! Sign + store content provenance for a produced asset. No-op when the
//! signing secret is unset.

use std::path::Path;

use ferrite_stream_core::provenance::{self, Manifest};
use sqlx::PgPool;
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn record(
    pool: &PgPool,
    secret: Option<&str>,
    tenant_id: Uuid,
    asset_id: Uuid,
    filename: &str,
    operation: &str,
    source_asset_id: Option<Uuid>,
    path: &Path,
) {
    let Some(secret) = secret else { return };
    let Ok(bytes) = tokio::fs::read(path).await else {
        tracing::warn!(asset = %asset_id, "provenance: could not read file to hash");
        return;
    };
    let manifest = Manifest {
        version: 1,
        tool: "Ferrite Stream".to_string(),
        tenant_id,
        asset_id,
        filename: filename.to_string(),
        sha256: provenance::sha256_hex(&bytes),
        operation: operation.to_string(),
        source_asset_id,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let json = manifest.to_json();
    let sig = provenance::sign(secret, &json);
    if let Err(e) = crate::db::insert_provenance(pool, asset_id, tenant_id, &json, &sig).await {
        tracing::warn!(asset = %asset_id, error = %e, "provenance: failed to store");
    } else {
        tracing::info!(asset = %asset_id, operation, "provenance signed");
    }
}
