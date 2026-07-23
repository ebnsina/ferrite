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

// --- Users / members ---------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct UserAuth {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub password_hash: String,
    pub role: String,
    pub name: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Member {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

/// A user's own profile (self-scoped view).
#[derive(Debug, sqlx::FromRow)]
pub struct Profile {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub password_hash: String,
}

pub async fn create_user(
    pool: &PgPool,
    id: Uuid,
    tenant_id: Uuid,
    email: &str,
    password_hash: &str,
    role: &str,
    name: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users (id, tenant_id, email, password_hash, role, name)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(email)
    .bind(password_hash)
    .bind(role)
    .bind(name)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<UserAuth>, sqlx::Error> {
    sqlx::query_as::<_, UserAuth>(
        "SELECT id, tenant_id, password_hash, role, name FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

/// A user's own profile by id — includes the password hash for verification.
pub async fn find_profile(pool: &PgPool, id: Uuid) -> Result<Option<Profile>, sqlx::Error> {
    sqlx::query_as::<_, Profile>(
        "SELECT id, email, name, role, password_hash FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn update_user_name(pool: &PgPool, id: Uuid, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET name = $2 WHERE id = $1")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_user_password(
    pool: &PgPool,
    id: Uuid,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET password_hash = $2 WHERE id = $1")
        .bind(id)
        .bind(password_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_members(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Member>, sqlx::Error> {
    sqlx::query_as::<_, Member>(
        "SELECT id, email, name, role, created_at FROM users
         WHERE tenant_id = $1 ORDER BY created_at",
    )
    .bind(tenant_id)
    .fetch_all(pool)
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

#[derive(Debug, sqlx::FromRow)]
pub struct ApiKeyListItem {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

pub async fn list_api_keys(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<ApiKeyListItem>, sqlx::Error> {
    sqlx::query_as::<_, ApiKeyListItem>(
        "SELECT id, name, prefix, last_used_at, revoked_at, created_at FROM api_keys
         WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// Revoke a key (tenant-scoped). Returns true if a row was affected.
pub async fn revoke_api_key(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE api_keys SET revoked_at = now()
         WHERE id = $1 AND tenant_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

// --- Member management -------------------------------------------------------

/// A member within a tenant (for owner-only role/removal checks).
pub async fn find_member(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<Member>, sqlx::Error> {
    sqlx::query_as::<_, Member>(
        "SELECT id, email, name, role, created_at FROM users WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_member_role(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    role: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE users SET role = $3 WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(tenant_id)
        .bind(role)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_user(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM users WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(tenant_id)
        .execute(pool)
        .await?;
    Ok(())
}

// --- Password resets ---------------------------------------------------------

pub async fn create_password_reset(
    pool: &PgPool,
    token: &str,
    user_id: Uuid,
    expires_at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO password_resets (token, user_id, expires_at) VALUES ($1, $2, $3)")
        .bind(token)
        .bind(user_id)
        .bind(expires_at)
        .execute(pool)
        .await?;
    Ok(())
}

/// Return the user id for a valid (unused, unexpired) reset token.
pub async fn find_valid_reset(pool: &PgPool, token: &str) -> Result<Option<Uuid>, sqlx::Error> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM password_resets
         WHERE token = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn mark_reset_used(pool: &PgPool, token: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE password_resets SET used_at = now() WHERE token = $1")
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
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

/// Create an asset that the worker will produce (e.g. a clip). Starts in
/// `processing` — no client upload is expected.
pub async fn create_processing_asset(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    filename: &str,
    original_key: &str,
) -> Result<Asset, sqlx::Error> {
    sqlx::query_as::<_, Asset>(
        "INSERT INTO assets (id, tenant_id, filename, original_key, status)
         VALUES ($1, $2, $3, $4, 'processing')
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

/// Mark a produced asset as failed (tenant-scoped).
pub async fn set_asset_error(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE assets SET status = 'error' WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(tenant_id)
        .execute(pool)
        .await?;
    Ok(())
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

/// Stored provenance (manifest JSON + hex signature) for an asset, tenant-scoped.
pub async fn get_provenance(
    pool: &PgPool,
    tenant_id: Uuid,
    asset_id: Uuid,
) -> Result<Option<(String, String)>, sqlx::Error> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT manifest, signature FROM provenance WHERE asset_id = $1 AND tenant_id = $2",
    )
    .bind(asset_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

/// Moderation result (flagged, categories JSON) for an asset, tenant-scoped.
pub async fn get_moderation(
    pool: &PgPool,
    tenant_id: Uuid,
    asset_id: Uuid,
) -> Result<Option<(bool, serde_json::Value)>, sqlx::Error> {
    sqlx::query_as::<_, (bool, serde_json::Value)>(
        "SELECT flagged, categories FROM moderation WHERE asset_id = $1 AND tenant_id = $2",
    )
    .bind(asset_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
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
    pub has_mp4: bool,
    pub has_audio: bool,
    pub has_captions: bool,
}

/// Create a clip job row (kind='clip') linking source → produced asset.
pub async fn create_clip_job(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    source_asset_id: Uuid,
    dest_asset_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO jobs (id, tenant_id, asset_id, output_prefix, kind, dest_asset_id)
         VALUES ($1, $2, $3, '', 'clip', $4)",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(source_asset_id)
    .bind(dest_asset_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Create an AI-shorts job row (kind='shorts'); produced shorts become assets.
pub async fn create_shorts_job(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    source_asset_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO jobs (id, tenant_id, asset_id, output_prefix, kind)
         VALUES ($1, $2, $3, '', 'shorts')",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(source_asset_id)
    .execute(pool)
    .await?;
    Ok(())
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
    has_mp4: bool,
    has_audio: bool,
    has_captions: bool,
) -> Result<(Job, bool), sqlx::Error> {
    let inserted = sqlx::query_as::<_, Job>(
        "INSERT INTO jobs (id, tenant_id, asset_id, output_prefix, idempotency_key, has_mp4, has_audio, has_captions)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (tenant_id, idempotency_key) DO NOTHING
         RETURNING id, asset_id, state, progress, error, output_prefix, queued_at, finished_at,
         (encryption_key IS NOT NULL) AS encrypted, has_mp4, has_audio, has_captions",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(asset_id)
    .bind(output_prefix)
    .bind(idempotency_key)
    .bind(has_mp4)
    .bind(has_audio)
    .bind(has_captions)
    .fetch_optional(pool)
    .await?;

    if let Some(job) = inserted {
        return Ok((job, true));
    }

    let existing = sqlx::query_as::<_, Job>(
        "SELECT id, asset_id, state, progress, error, output_prefix, queued_at, finished_at,
         (encryption_key IS NOT NULL) AS encrypted, has_mp4, has_audio, has_captions
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
         (encryption_key IS NOT NULL) AS encrypted, has_mp4, has_audio, has_captions
         FROM jobs WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

// --- Simulcast targets -------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct SimulcastTarget {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub stream_key: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

pub async fn create_target(
    pool: &PgPool,
    tenant_id: Uuid,
    live_stream_id: Uuid,
    name: &str,
    url: &str,
    stream_key: &str,
) -> Result<SimulcastTarget, sqlx::Error> {
    sqlx::query_as::<_, SimulcastTarget>(
        "INSERT INTO simulcast_targets (tenant_id, live_stream_id, name, url, stream_key)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id, name, url, stream_key, enabled, created_at",
    )
    .bind(tenant_id)
    .bind(live_stream_id)
    .bind(name)
    .bind(url)
    .bind(stream_key)
    .fetch_one(pool)
    .await
}

pub async fn list_targets(
    pool: &PgPool,
    tenant_id: Uuid,
    live_stream_id: Uuid,
) -> Result<Vec<SimulcastTarget>, sqlx::Error> {
    sqlx::query_as::<_, SimulcastTarget>(
        "SELECT id, name, url, stream_key, enabled, created_at FROM simulcast_targets
         WHERE live_stream_id = $1 AND tenant_id = $2 ORDER BY created_at",
    )
    .bind(live_stream_id)
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_target(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("DELETE FROM simulcast_targets WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(tenant_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

/// Enabled (url, stream_key) destinations for a publishing stream key.
pub async fn enabled_targets_by_stream_key(
    pool: &PgPool,
    stream_key: &str,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT t.url, t.stream_key FROM simulcast_targets t
         JOIN live_streams l ON l.id = t.live_stream_id
         WHERE l.stream_key = $1 AND t.enabled = true",
    )
    .bind(stream_key)
    .fetch_all(pool)
    .await
}

// --- In-video search ---------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct SearchHit {
    pub asset_id: Uuid,
    pub filename: String,
    pub job_id: Uuid,
    pub start_secs: f32,
    pub snippet: String,
}

/// A job's transcript cues in order (for translation / rebuilding VTT).
pub async fn transcript_for_job(
    pool: &PgPool,
    job_id: Uuid,
) -> Result<Vec<(f32, f32, String)>, sqlx::Error> {
    sqlx::query_as::<_, (f32, f32, String)>(
        "SELECT start_secs, end_secs, text FROM transcript_segments
         WHERE job_id = $1 ORDER BY start_secs",
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
}

pub async fn insert_caption_track(
    pool: &PgPool,
    tenant_id: Uuid,
    job_id: Uuid,
    lang: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO caption_tracks (tenant_id, job_id, lang) VALUES ($1, $2, $3)
         ON CONFLICT (job_id, lang) DO NOTHING",
    )
    .bind(tenant_id)
    .bind(job_id)
    .bind(lang)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_caption_langs(pool: &PgPool, job_id: Uuid) -> Result<Vec<String>, sqlx::Error> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT lang FROM caption_tracks WHERE job_id = $1 ORDER BY lang")
            .bind(job_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Full-text search over transcript segments (tenant-scoped), ranked by relevance.
pub async fn search_transcripts(
    pool: &PgPool,
    tenant_id: Uuid,
    query: &str,
) -> Result<Vec<SearchHit>, sqlx::Error> {
    sqlx::query_as::<_, SearchHit>(
        "SELECT s.asset_id, a.filename, s.job_id, s.start_secs, s.text AS snippet
         FROM transcript_segments s
         JOIN assets a ON a.id = s.asset_id
         WHERE s.tenant_id = $1 AND s.tsv @@ plainto_tsquery('english', $2)
         ORDER BY ts_rank(s.tsv, plainto_tsquery('english', $2)) DESC
         LIMIT 40",
    )
    .bind(tenant_id)
    .bind(query)
    .fetch_all(pool)
    .await
}

// --- Playback analytics ------------------------------------------------------

pub async fn insert_playback_event(
    pool: &PgPool,
    tenant_id: Uuid,
    job_id: Uuid,
    session: &str,
    kind: &str,
    position: f64,
    watched: f64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO playback_events (tenant_id, job_id, session, kind, position, watched)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(tenant_id)
    .bind(job_id)
    .bind(session)
    .bind(kind)
    .bind(position as f32)
    .bind(watched as f32)
    .execute(pool)
    .await?;
    Ok(())
}

/// (views, watch_seconds, completions) for a job, tenant-scoped.
pub async fn job_analytics(
    pool: &PgPool,
    tenant_id: Uuid,
    job_id: Uuid,
) -> Result<(i64, f64, i64), sqlx::Error> {
    let row: (i64, f64, i64) = sqlx::query_as(
        "SELECT
           count(DISTINCT session) FILTER (WHERE kind = 'view'),
           COALESCE(sum(watched), 0)::float8,
           count(DISTINCT session) FILTER (WHERE kind = 'ended')
         FROM playback_events WHERE job_id = $1 AND tenant_id = $2",
    )
    .bind(job_id)
    .bind(tenant_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

// --- Webhooks ----------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct Webhook {
    pub id: Uuid,
    pub url: String,
    pub events: Vec<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn create_webhook(
    pool: &PgPool,
    tenant_id: Uuid,
    url: &str,
    secret: &str,
    events: &[String],
) -> Result<Webhook, sqlx::Error> {
    sqlx::query_as::<_, Webhook>(
        "INSERT INTO webhooks (tenant_id, url, secret, events)
         VALUES ($1, $2, $3, $4) RETURNING id, url, events, created_at",
    )
    .bind(tenant_id)
    .bind(url)
    .bind(secret)
    .bind(events)
    .fetch_one(pool)
    .await
}

pub async fn list_webhooks(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Webhook>, sqlx::Error> {
    sqlx::query_as::<_, Webhook>(
        "SELECT id, url, events, created_at FROM webhooks
         WHERE tenant_id = $1 ORDER BY created_at DESC",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// Delete a webhook; false if it does not belong to the tenant.
pub async fn delete_webhook(pool: &PgPool, tenant_id: Uuid, id: Uuid) -> Result<bool, sqlx::Error> {
    let r = sqlx::query("DELETE FROM webhooks WHERE id = $1 AND tenant_id = $2")
        .bind(id)
        .bind(tenant_id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

/// Fail jobs stuck in a non-terminal state longer than `stale_secs` (orphaned
/// enqueues or hung transcodes). Returns how many were swept.
pub async fn fail_stale_jobs(pool: &PgPool, stale_secs: i64) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "UPDATE jobs SET state = 'failed', error = 'job timed out', finished_at = now()
         WHERE state NOT IN ('completed', 'failed')
           AND queued_at < now() - make_interval(secs => $1)",
    )
    .bind(stale_secs as f64)
    .execute(pool)
    .await?;
    Ok(r.rows_affected())
}

// --- Usage / billing ---------------------------------------------------------

/// Transcoded minutes accrued this month for the tenant.
pub async fn usage_minutes(pool: &PgPool, tenant_id: Uuid) -> Result<f64, sqlx::Error> {
    let row: Option<(f64,)> = sqlx::query_as(
        "SELECT minutes FROM usage
         WHERE tenant_id = $1 AND period = date_trunc('month', now())::date",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0).unwrap_or(0.0))
}

/// Total source bytes stored for the tenant (ready assets).
pub async fn storage_bytes(pool: &PgPool, tenant_id: Uuid) -> Result<i64, sqlx::Error> {
    let row: (i64,) =
        sqlx::query_as("SELECT COALESCE(SUM(bytes), 0)::bigint FROM assets WHERE tenant_id = $1")
            .bind(tenant_id)
            .fetch_one(pool)
            .await?;
    Ok(row.0)
}

// --- Live streams ------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct LiveStream {
    pub id: Uuid,
    pub name: String,
    pub stream_key: String,
    pub created_at: DateTime<Utc>,
}

pub async fn create_live_stream(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
    name: &str,
    stream_key: &str,
) -> Result<LiveStream, sqlx::Error> {
    sqlx::query_as::<_, LiveStream>(
        "INSERT INTO live_streams (id, tenant_id, name, stream_key)
         VALUES ($1, $2, $3, $4) RETURNING id, name, stream_key, created_at",
    )
    .bind(id)
    .bind(tenant_id)
    .bind(name)
    .bind(stream_key)
    .fetch_one(pool)
    .await
}

pub async fn list_live_streams(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<LiveStream>, sqlx::Error> {
    sqlx::query_as::<_, LiveStream>(
        "SELECT id, name, stream_key, created_at FROM live_streams
         WHERE tenant_id = $1 ORDER BY created_at DESC LIMIT 200",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

/// Look up a live stream by its (secret) key — used by the ingest DVR callback.
pub async fn find_live_stream_by_key(
    pool: &PgPool,
    stream_key: &str,
) -> Result<Option<(Uuid, Uuid, String)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, Uuid, String)>(
        "SELECT id, tenant_id, name FROM live_streams WHERE stream_key = $1",
    )
    .bind(stream_key)
    .fetch_optional(pool)
    .await
}

pub async fn find_live_stream(
    pool: &PgPool,
    tenant_id: Uuid,
    id: Uuid,
) -> Result<Option<LiveStream>, sqlx::Error> {
    sqlx::query_as::<_, LiveStream>(
        "SELECT id, name, stream_key, created_at FROM live_streams
         WHERE id = $1 AND tenant_id = $2",
    )
    .bind(id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_jobs(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<Job>, sqlx::Error> {
    sqlx::query_as::<_, Job>(
        "SELECT id, asset_id, state, progress, error, output_prefix, queued_at, finished_at,
         (encryption_key IS NOT NULL) AS encrypted, has_mp4, has_audio, has_captions
         FROM jobs WHERE tenant_id = $1 ORDER BY queued_at DESC LIMIT 200",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}
