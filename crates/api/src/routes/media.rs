//! On-demand derived media: a still frame at any timestamp, and a short animated
//! preview — extracted from the private source with FFmpeg and cached in storage.
//!
//! Served over signed, embeddable URLs (same HMAC scheme as playback) so an
//! `<img>`/`<video>` can point straight at them without an auth header.

use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;
use tokio::process::Command;
use uuid::Uuid;

use crate::routes::playback::now_unix;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

/// Signed media URLs are long-lived (embeds in the dashboard / shared links).
pub const MEDIA_TOKEN_TTL_SECS: u64 = 24 * 60 * 60;

fn hmac_hex(secret: &str, msg: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key size");
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Token scoped to one tenant's asset: `tenant.asset.exp.sig`.
pub fn sign(secret: &str, tenant: Uuid, asset: Uuid, exp: u64) -> String {
    let payload = format!("{tenant}.{asset}.{exp}");
    format!("{payload}.{}", hmac_hex(secret, &payload))
}

fn verify(secret: &str, token: &str) -> Option<(Uuid, Uuid)> {
    let (payload, sig_hex) = token.rsplit_once('.')?;
    let sig = hex::decode(sig_hex).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(payload.as_bytes());
    mac.verify_slice(&sig).ok()?;

    let mut parts = payload.split('.');
    let tenant: Uuid = parts.next()?.parse().ok()?;
    let asset: Uuid = parts.next()?.parse().ok()?;
    let exp: u64 = parts.next()?.parse().ok()?;
    (exp > now_unix()).then_some((tenant, asset))
}

/// Build the signed thumbnail + preview URLs for a ready asset.
pub fn signed_urls(state: &AppState, tenant: Uuid, asset: Uuid) -> (String, String) {
    let base = state.settings().public_url.trim_end_matches('/');
    let token = sign(
        &state.settings().playback_secret,
        tenant,
        asset,
        now_unix() + MEDIA_TOKEN_TTL_SECS,
    );
    (
        format!("{base}/media/{asset}/thumbnail?token={token}"),
        format!("{base}/media/{asset}/preview?token={token}"),
    )
}

#[derive(Deserialize)]
pub struct ThumbQuery {
    token: String,
    /// Timestamp in seconds (default 1s in).
    #[serde(default)]
    time: Option<f64>,
    /// Output width in px (height keeps aspect). Clamped 16–1920.
    #[serde(default)]
    width: Option<u32>,
}

/// `GET /media/{asset_id}/thumbnail?time=&width=&token=` — cached JPEG frame.
pub async fn thumbnail(
    State(state): State<AppState>,
    Path(asset_id): Path<Uuid>,
    Query(q): Query<ThumbQuery>,
) -> Response {
    let secret = &state.settings().playback_secret;
    let Some((tenant, asset)) = verify(secret, &q.token) else {
        return (StatusCode::FORBIDDEN, "invalid or expired token").into_response();
    };
    if asset != asset_id {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    let time = q.time.unwrap_or(1.0).max(0.0);
    let width = q.width.unwrap_or(320).clamp(16, 1920);
    let cache_key = format!("{tenant}/derived/{asset}/thumb_{:.2}_{width}.jpg", time);

    if let Ok(bytes) = state.storage().get_bytes(&cache_key).await {
        return jpeg(bytes);
    }

    let Some(url) = source_url(&state, tenant, asset).await else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    let out = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-ss",
            &format!("{time:.3}"),
            "-i",
            &url,
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={width}:-2"),
            "-q:v",
            "3",
            "-f",
            "image2",
            "-c:v",
            "mjpeg",
            "pipe:1",
        ])
        .output()
        .await;

    match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            let bytes = o.stdout;
            let _ = state.storage().put_bytes(&cache_key, bytes.clone()).await;
            jpeg(bytes)
        }
        Ok(o) => {
            tracing::warn!(asset = %asset, code = ?o.status.code(), "thumbnail extraction failed");
            (StatusCode::UNPROCESSABLE_ENTITY, "could not extract frame").into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "ffmpeg spawn failed (is it installed?)");
            (StatusCode::INTERNAL_SERVER_ERROR, "extraction unavailable").into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct PreviewQuery {
    token: String,
}

/// `GET /media/{asset_id}/preview?token=` — cached animated preview (timelapse MP4).
pub async fn preview(
    State(state): State<AppState>,
    Path(asset_id): Path<Uuid>,
    Query(q): Query<PreviewQuery>,
) -> Response {
    let secret = &state.settings().playback_secret;
    let Some((tenant, asset)) = verify(secret, &q.token) else {
        return (StatusCode::FORBIDDEN, "invalid or expired token").into_response();
    };
    if asset != asset_id {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    let cache_key = format!("{tenant}/derived/{asset}/preview.mp4");
    if let Ok(bytes) = state.storage().get_bytes(&cache_key).await {
        return mp4(bytes);
    }

    let Some(url) = source_url(&state, tenant, asset).await else {
        return (StatusCode::NOT_FOUND, "asset not found").into_response();
    };

    // Sample ~1 frame/sec and replay fast → a compact hover-preview. Written to a
    // temp file because fragment-free MP4 needs a seekable output, not a pipe.
    let tmp = std::env::temp_dir().join(format!("frt-preview-{asset}.mp4"));
    let tmp_str = tmp.to_string_lossy().to_string();
    let out = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-i",
            &url,
            "-vf",
            "fps=1,scale=320:-2,setpts=N/12/TB",
            "-frames:v",
            "72",
            "-an",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            &tmp_str,
        ])
        .status()
        .await;

    let result = match out {
        Ok(s) if s.success() => match tokio::fs::read(&tmp).await {
            Ok(bytes) if !bytes.is_empty() => {
                let _ = state.storage().put_bytes(&cache_key, bytes.clone()).await;
                mp4(bytes)
            }
            _ => (StatusCode::UNPROCESSABLE_ENTITY, "empty preview").into_response(),
        },
        Ok(_) => (StatusCode::UNPROCESSABLE_ENTITY, "could not build preview").into_response(),
        Err(e) => {
            tracing::error!(error = %e, "ffmpeg spawn failed (is it installed?)");
            (StatusCode::INTERNAL_SERVER_ERROR, "preview unavailable").into_response()
        }
    };
    let _ = tokio::fs::remove_file(&tmp).await;
    result
}

/// Presigned GET URL for the asset's source (FFmpeg reads it over HTTP range).
async fn source_url(state: &AppState, tenant: Uuid, asset: Uuid) -> Option<String> {
    let a = crate::db::find_asset(state.db(), tenant, asset)
        .await
        .ok()??;
    state
        .storage()
        .presign_get(&a.original_key, Duration::from_secs(600))
        .await
        .ok()
}

fn jpeg(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        bytes,
    )
        .into_response()
}

fn mp4(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, "video/mp4"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
        .into_response()
}
