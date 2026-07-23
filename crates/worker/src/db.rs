//! Worker-side job persistence: state transitions, progress, renditions.

use sqlx::PgPool;
use uuid::Uuid;

/// Set state and stamp `started_at` on first transition out of `queued`.
pub async fn mark_started(pool: &PgPool, job_id: Uuid, state: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET state = $2, started_at = COALESCE(started_at, now()) WHERE id = $1",
    )
    .bind(job_id)
    .bind(state)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_state(pool: &PgPool, job_id: Uuid, state: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE jobs SET state = $2 WHERE id = $1")
        .bind(job_id)
        .bind(state)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_progress(pool: &PgPool, job_id: Uuid, progress: f32) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE jobs SET progress = $2 WHERE id = $1")
        .bind(job_id)
        .bind(progress)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_completed(pool: &PgPool, job_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET state = 'completed', progress = 1, error = NULL, finished_at = now()
         WHERE id = $1",
    )
    .bind(job_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mark a produced (clip) asset ready with its final size.
pub async fn mark_asset_ready(
    pool: &PgPool,
    asset_id: Uuid,
    bytes: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE assets SET status = 'ready', bytes = COALESCE($2, bytes) WHERE id = $1")
        .bind(asset_id)
        .bind(bytes)
        .execute(pool)
        .await?;
    Ok(())
}

/// Correct the captions flag to what the pipeline actually produced.
pub async fn set_has_captions(pool: &PgPool, job_id: Uuid, value: bool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE jobs SET has_captions = $2 WHERE id = $1")
        .bind(job_id)
        .bind(value)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_asset_status(
    pool: &PgPool,
    asset_id: Uuid,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE assets SET status = $2 WHERE id = $1")
        .bind(asset_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_failed(pool: &PgPool, job_id: Uuid, error: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE jobs SET state = 'failed', error = $2, finished_at = now(), attempts = attempts + 1
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

/// Store the AES-128 key so the API's key endpoint can serve it to authorized viewers.
pub async fn set_encryption_key(
    pool: &PgPool,
    job_id: Uuid,
    key: &[u8],
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE jobs SET encryption_key = $2 WHERE id = $1")
        .bind(job_id)
        .bind(key)
        .execute(pool)
        .await?;
    Ok(())
}

/// Accrue transcoded minutes to the tenant's usage for the current month.
pub async fn record_usage(pool: &PgPool, tenant_id: Uuid, minutes: f64) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO usage (tenant_id, period, minutes)
         VALUES ($1, date_trunc('month', now())::date, $2)
         ON CONFLICT (tenant_id, period)
         DO UPDATE SET minutes = usage.minutes + EXCLUDED.minutes",
    )
    .bind(tenant_id)
    .bind(minutes)
    .execute(pool)
    .await?;
    Ok(())
}

/// Tenant webhooks subscribed to an event: (url, secret).
pub async fn webhooks_for_event(
    pool: &PgPool,
    tenant_id: Uuid,
    event: &str,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT url, secret FROM webhooks WHERE tenant_id = $1 AND $2 = ANY(events)",
    )
    .bind(tenant_id)
    .bind(event)
    .fetch_all(pool)
    .await
}

pub async fn insert_rendition(
    pool: &PgPool,
    job_id: Uuid,
    kind: &str,
    name: &str,
    height: i32,
    bitrate_kbps: i32,
    playlist_key: &str,
    prefix: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO renditions (job_id, kind, name, height, bitrate_kbps, playlist_key, prefix)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(job_id)
    .bind(kind)
    .bind(name)
    .bind(height)
    .bind(bitrate_kbps)
    .bind(playlist_key)
    .bind(prefix)
    .execute(pool)
    .await?;
    Ok(())
}
