//! Live streams: RTMP ingest keyed by a secret stream key, HLS/FLV playback.
//!
//! The ingest server is abstracted behind [`Ingest`] — only URL shapes and a
//! status poll are server-specific, so SRS can be swapped for another backend.

use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::TenantContext;
use crate::config::Settings;
use crate::db::{self, LiveStream};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Server-agnostic view over the ingest server (SRS today).
struct Ingest<'a> {
    rtmp_base: &'a str,
    srt_base: &'a str,
    hls_base: &'a str,
    api_url: &'a str,
}

impl<'a> Ingest<'a> {
    fn from(settings: &'a Settings) -> Self {
        Self {
            rtmp_base: &settings.live_rtmp_base,
            srt_base: &settings.live_srt_base,
            hls_base: &settings.live_hls_base,
            api_url: &settings.live_api_url,
        }
    }

    fn ingest_url(&self, key: &str) -> String {
        format!("{}/live/{key}", self.rtmp_base)
    }
    /// SRT publish URL — streamid encodes the app/stream and publish intent.
    fn srt_url(&self, key: &str) -> String {
        format!("{}?streamid=#!::r=live/{key},m=publish", self.srt_base)
    }
    fn hls_url(&self, key: &str) -> String {
        format!("{}/live/{key}.m3u8", self.hls_base)
    }
    fn flv_url(&self, key: &str) -> String {
        format!("{}/live/{key}.flv", self.hls_base)
    }

    /// Whether a stream key is currently publishing (SRS `/api/v1/streams/`).
    async fn is_live(&self, key: &str) -> bool {
        let url = format!("{}/api/v1/streams/", self.api_url);
        let Ok(resp) = reqwest::Client::new()
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        else {
            return false;
        };
        let Ok(json) = resp.json::<serde_json::Value>().await else {
            return false;
        };
        json["streams"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|s| s["name"] == *key && s["publish"]["active"] == true)
            })
            .unwrap_or(false)
    }
}

#[derive(Serialize)]
pub struct LiveStreamView {
    pub id: Uuid,
    pub name: String,
    pub stream_key: String,
    pub ingest_url: String,
    pub srt_url: String,
    pub hls_url: String,
    pub flv_url: String,
    pub created_at: String,
    pub live: bool,
}

fn view(settings: &Settings, s: LiveStream, live: bool) -> LiveStreamView {
    let ingest = Ingest::from(settings);
    LiveStreamView {
        ingest_url: ingest.ingest_url(&s.stream_key),
        srt_url: ingest.srt_url(&s.stream_key),
        hls_url: ingest.hls_url(&s.stream_key),
        flv_url: ingest.flv_url(&s.stream_key),
        id: s.id,
        name: s.name,
        stream_key: s.stream_key,
        created_at: s.created_at.to_rfc3339(),
        live,
    }
}

#[derive(Deserialize, Validate)]
pub struct CreateLiveRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
}

/// `POST /v1/live/streams` — create a stream and its RTMP ingest URL.
pub async fn create_stream(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<CreateLiveRequest>,
) -> ApiResult<Json<LiveStreamView>> {
    body.validate().map_err(ApiError::Validation)?;

    let id = Uuid::new_v4();
    let stream_key = generate_stream_key();
    let s = db::create_live_stream(state.db(), ctx.tenant_id, id, &body.name, &stream_key).await?;
    Ok(Json(view(state.settings(), s, false)))
}

/// `GET /v1/live/streams` — list the tenant's streams (no per-stream status poll).
pub async fn list_streams(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<LiveStreamView>>> {
    let streams = db::list_live_streams(state.db(), ctx.tenant_id).await?;
    let settings = state.settings();
    Ok(Json(
        streams
            .into_iter()
            .map(|s| view(settings, s, false))
            .collect(),
    ))
}

/// `GET /v1/live/streams/:id` — fetch a stream with its current live status.
pub async fn get_stream(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<LiveStreamView>> {
    let s = db::find_live_stream(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let live = Ingest::from(state.settings()).is_live(&s.stream_key).await;
    Ok(Json(view(state.settings(), s, live)))
}

fn generate_stream_key() -> String {
    let mut bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

// --- Live -> VOD archival (ingest server DVR callback) -----------------------

#[derive(Deserialize)]
pub struct HookQuery {
    secret: String,
}

/// SRS `on_dvr` callback body (subset).
#[derive(Deserialize)]
pub struct DvrCallback {
    stream: String,
    file: String,
}

/// `POST /internal/live/dvr?secret=` — the ingest server calls this when a live
/// session's recording is finalized. We archive it into object storage as a VOD
/// asset and auto-submit a transcode job. Returns `0` (SRS success).
pub async fn dvr_hook(
    State(state): State<AppState>,
    Query(q): Query<HookQuery>,
    Json(cb): Json<DvrCallback>,
) -> (StatusCode, &'static str) {
    if q.secret != state.settings().live_hook_secret {
        return (StatusCode::FORBIDDEN, "1");
    }

    // Resolve the tenant from the stream key; ack unknown streams without work.
    let Ok(Some((_id, tenant_id, name))) =
        db::find_live_stream_by_key(state.db(), &cb.stream).await
    else {
        return (StatusCode::OK, "0");
    };

    // Map the DVR file path to its HTTP URL (SRS serves objs/nginx/html at hls_base).
    let Some(idx) = cb.file.find("/dvr/") else {
        return (StatusCode::OK, "0");
    };
    let url = format!("{}{}", state.settings().live_hls_base, &cb.file[idx..]);
    let filename = cb
        .file
        .rsplit('/')
        .next()
        .unwrap_or("recording.flv")
        .to_string();

    // Archive in the background so the callback returns immediately.
    tokio::spawn(archive_recording(state, tenant_id, name, url, filename));
    (StatusCode::OK, "0")
}

async fn archive_recording(
    state: AppState,
    tenant_id: Uuid,
    stream_name: String,
    url: String,
    filename: String,
) {
    let asset_id = Uuid::new_v4();
    let key = format!("{tenant_id}/sources/{asset_id}/{filename}");

    let result = async {
        let bytes = reqwest::get(&url).await?.bytes().await?.to_vec();
        let size = bytes.len() as i64;
        state.storage().put_bytes(&key, bytes).await?;
        db::create_asset(
            state.db(),
            tenant_id,
            asset_id,
            &format!("{stream_name} (recording)"),
            &key,
        )
        .await?;
        db::mark_asset_ready(state.db(), tenant_id, asset_id, Some(size)).await?;
        super::jobs::submit_job(&state, tenant_id, asset_id, None, false).await?;
        Ok::<_, anyhow::Error>(())
    }
    .await;

    match result {
        Ok(()) => {
            tracing::info!(tenant = %tenant_id, asset = %asset_id, "archived live recording to VOD")
        }
        Err(e) => tracing::error!(tenant = %tenant_id, error = %e, "failed to archive recording"),
    }
}
