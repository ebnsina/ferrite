//! Authorized playback proxy. Outputs are private in storage; this serves them
//! via short-lived HMAC tokens and rewrites HLS playlists so every child
//! playlist/segment URL carries the token. Production would front this with a CDN.

use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;
use uuid::Uuid;

use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

/// How long a playback token is valid (covers a viewing session).
pub const TOKEN_TTL_SECS: u64 = 4 * 60 * 60;

/// Sign a token scoped to one tenant's job: `tenant.job.exp.sig`.
pub fn sign_token(secret: &str, tenant: Uuid, job: Uuid, exp: u64) -> String {
    let payload = format!("{tenant}.{job}.{exp}");
    let sig = hmac_hex(secret, &payload);
    format!("{payload}.{sig}")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hmac_hex(secret: &str, msg: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac accepts any key size");
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verify a token: returns (tenant, job) when the signature is valid and unexpired.
fn verify_token(secret: &str, token: &str) -> Option<(Uuid, Uuid)> {
    let (payload, sig_hex) = token.rsplit_once('.')?;
    let sig = hex::decode(sig_hex).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).ok()?;
    mac.update(payload.as_bytes());
    mac.verify_slice(&sig).ok()?; // constant-time

    let mut parts = payload.split('.');
    let tenant: Uuid = parts.next()?.parse().ok()?;
    let job: Uuid = parts.next()?.parse().ok()?;
    let exp: u64 = parts.next()?.parse().ok()?;
    (exp > now_unix()).then_some((tenant, job))
}

#[derive(Deserialize)]
pub struct TokenQuery {
    token: String,
}

/// `GET /playback/{job_id}/{*path}` — token-authorized asset delivery.
pub async fn serve(
    State(state): State<AppState>,
    Path((job_id, path)): Path<(Uuid, String)>,
    Query(q): Query<TokenQuery>,
) -> Response {
    let secret = &state.settings().playback_secret;
    let Some((tenant, job)) = verify_token(secret, &q.token) else {
        return (StatusCode::FORBIDDEN, "invalid or expired token").into_response();
    };
    // The token is scoped to a single job; reject path/id mismatch or traversal.
    if job != job_id || path.contains("..") || path.starts_with('/') {
        return (StatusCode::FORBIDDEN, "forbidden").into_response();
    }

    // The AES-128 key is served from the DB (never stored as an object).
    if path.ends_with("enc.key") {
        return match crate::db::get_encryption_key(state.db(), tenant, job).await {
            Ok(Some(key)) => {
                ([(header::CONTENT_TYPE, "application/octet-stream")], key).into_response()
            }
            _ => (StatusCode::NOT_FOUND, "no key").into_response(),
        };
    }

    let key = format!("{tenant}/outputs/{job}/{path}");
    let bytes = match state.storage().get_bytes(&key).await {
        Ok(b) => b,
        Err(_) => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };

    if path.ends_with(".m3u8") {
        let rewritten = rewrite_playlist(&String::from_utf8_lossy(&bytes), &q.token);
        (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            rewritten,
        )
            .into_response()
    } else if path.ends_with(".mpd") {
        let rewritten = rewrite_dash(&String::from_utf8_lossy(&bytes), &q.token);
        ([(header::CONTENT_TYPE, "application/dash+xml")], rewritten).into_response()
    } else {
        ([(header::CONTENT_TYPE, content_type(&path))], bytes).into_response()
    }
}

/// Append the token to DASH segment/init template URLs (they end in `.m4s`) so
/// the player's relative segment requests stay authorized.
fn rewrite_dash(xml: &str, token: &str) -> String {
    xml.replace(".m4s\"", &format!(".m4s?token={token}\""))
}

/// Append the token to every URI line so relative child/segment refs stay authorized.
fn rewrite_playlist(text: &str, token: &str) -> String {
    let mut out: String = text
        .lines()
        .map(|line| {
            let t = line.trim();
            // Tags whose URI attribute must carry the token: AES-128 key (#EXT-X-KEY),
            // fMP4 init segment (#EXT-X-MAP), and rendition groups (#EXT-X-MEDIA).
            if t.starts_with("#EXT-X-KEY")
                || t.starts_with("#EXT-X-MAP")
                || t.starts_with("#EXT-X-MEDIA")
            {
                tokenize_uri_attr(line, token)
            } else if t.is_empty() || t.starts_with('#') {
                line.to_string()
            } else {
                let sep = if line.contains('?') { '&' } else { '?' };
                format!("{line}{sep}token={token}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    out.push('\n');
    out
}

/// Append the token to the `URI="..."` value inside an HLS tag line.
fn tokenize_uri_attr(line: &str, token: &str) -> String {
    let Some(start) = line.find("URI=\"") else {
        return line.to_string();
    };
    let value_start = start + 5;
    let Some(rel) = line[value_start..].find('"') else {
        return line.to_string();
    };
    let value_end = value_start + rel;
    let sep = if line[value_start..value_end].contains('?') {
        '&'
    } else {
        '?'
    };
    format!(
        "{}{sep}token={token}{}",
        &line[..value_end],
        &line[value_end..]
    )
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("ts") => "video/mp2t",
        Some("m4s" | "mp4") => "video/mp4",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("vtt") => "text/vtt",
        _ => "application/octet-stream",
    }
}
