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
    sqlx::query_as::<_, Tenant>("SELECT id, name, plan, created_at FROM tenants WHERE id = $1")
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

// --- Jobs --------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub state: String,
    pub progress: f32,
    pub error: Option<String>,
    pub output_prefix: String,
    pub queued_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    /// True once an AES-128 key exists — encrypted jobs skip DASH output.
    pub encrypted: bool,
}

/// Idempotent create: a reused `idempotency_key` returns the existing job with
/// `created = false`, so the caller skips re-enqueuing.
pub async fn create_job(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    asset_id: Uuid,
    output_prefix: &str,
    idempotency_key: Option<&str>,
) -> Result<(Job, bool), sqlx::Error> {
    let inserted = sqlx::query_as::<_, Job>(
        "INSERT INTO jobs (id, tenant_id, asset_id, output_prefix, idempotency_key)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
         RETURNING id, asset_id, state, progress, error, output_prefix, queued_at, finished_at,
         (encryption_key IS NOT NULL) AS encrypted",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(asset_id)
    .bind(output_prefix)
    .bind(idempotency_key)
    .fetch_optional(pool)
    .await?;

    if let Some(job) = inserted {
        return Ok((job, true));
    }

    let existing = sqlx::query_as::<_, Job>(
        "SELECT id, asset_id, state, progress, error, output_prefix, queued_at, finished_at,
         (encryption_key IS NOT NULL) AS encrypted
         FROM jobs WHERE tenant_id = $1 AND idempotency_key = $2",
    )
    .bind(tenant_id)
    .bind(idempotency_key)
    .fetch_one(pool)
    .await?;
    Ok((existing, false))
}

/// Fetch a job's AES-128 key (tenant-scoped). Used by the playback key endpoint.
pub async fn get_encryption_key(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<Vec<u8>>, sqlx::Error> {
    let row: Option<(Option<Vec<u8>>,)> =
        sqlx::query_as("SELECT encryption_key FROM jobs WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|r| r.0))
}

pub async fn find_job(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<Job>, sqlx::Error> {
    sqlx::query_as::<_, Job>(
        "SELECT id, asset_id, state, progress, error, output_prefix, queued_at, finished_at,
         (encryption_key IS NOT NULL) AS encrypted
         FROM jobs WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_jobs(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Job>, sqlx::Error> {
    sqlx::query_as::<_, Job>(
        "SELECT id, asset_id, state, progress, error, output_prefix, queued_at, finished_at,
         (encryption_key IS NOT NULL) AS encrypted
         FROM jobs WHERE tenant_id = $1 ORDER BY queued_at DESC LIMIT 200",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}
