//! Database access helpers (runtime-checked sqlx queries).
//!
//! Runtime queries (not the `query!` macros) so the crate builds without a live
//! database at compile time — friendlier for CI and fresh checkouts.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub plan: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ApiKeyRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub revoked_at: Option<DateTime<Utc>>,
}

pub async fn create_tenant(pool: &PgPool, name: &str) -> Result<Tenant, sqlx::Error> {
    sqlx::query_as::<_, Tenant>(
        "INSERT INTO tenants (name) VALUES ($1) RETURNING id, name, plan, created_at",
    )
    .bind(name)
    .fetch_one(pool)
    .await
}

pub async fn find_tenant(pool: &PgPool, id: Uuid) -> Result<Option<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>(
        "SELECT id, name, plan, created_at FROM tenants WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create_api_key(
    pool: &PgPool,
    tenant_id: Uuid,
    name: &str,
    key_hash: &str,
    prefix: &str,
) -> Result<Uuid, sqlx::Error> {
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO api_keys (tenant_id, name, key_hash, prefix)
         VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(tenant_id)
    .bind(name)
    .bind(key_hash)
    .bind(prefix)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Look up a non-revoked API key by its hash.
pub async fn find_active_api_key(
    pool: &PgPool,
    key_hash: &str,
) -> Result<Option<ApiKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyRow>(
        "SELECT id, tenant_id, revoked_at FROM api_keys
         WHERE key_hash = $1 AND revoked_at IS NULL",
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
}

/// Best-effort last-used timestamp; failures here must not fail the request.
pub async fn touch_api_key(pool: &PgPool, id: Uuid) {
    let _ = sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await;
}

// --- Assets ------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct Asset {
    pub id: Uuid,
    pub filename: String,
    pub original_key: String,
    pub bytes: Option<i64>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

pub async fn create_asset(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    filename: &str,
    original_key: &str,
) -> Result<Asset, sqlx::Error> {
    sqlx::query_as::<_, Asset>(
        "INSERT INTO assets (id, tenant_id, filename, original_key)
         VALUES ($1, $2, $3, $4)
         RETURNING id, filename, original_key, bytes, status, created_at",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(filename)
    .bind(original_key)
    .fetch_one(pool)
    .await
}

/// Tenant-scoped fetch — never returns another tenant's asset.
pub async fn find_asset(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<Asset>, sqlx::Error> {
    sqlx::query_as::<_, Asset>(
        "SELECT id, filename, original_key, bytes, status, created_at
         FROM assets WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_assets(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Asset>, sqlx::Error> {
    sqlx::query_as::<_, Asset>(
        "SELECT id, filename, original_key, bytes, status, created_at
         FROM assets WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// Mark an upload complete. Returns false if the asset does not belong to the tenant.
pub async fn mark_asset_ready(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    bytes: Option<i64>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE assets SET status = 'ready', bytes = COALESCE($3, bytes)
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(bytes)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}
