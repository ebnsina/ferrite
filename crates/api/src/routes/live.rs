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

// --- Simulcast targets -------------------------------------------------------

#[derive(Serialize)]
pub struct TargetView {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub stream_key: String,
    pub enabled: bool,
    pub created_at: String,
}

impl From<db::SimulcastTarget> for TargetView {
    fn from(t: db::SimulcastTarget) -> Self {
        TargetView {
            id: t.id,
            name: t.name,
            url: t.url,
            stream_key: t.stream_key,
            enabled: t.enabled,
            created_at: t.created_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize, Validate)]
pub struct CreateTargetRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(min = 1, max = 500))]
    pub url: String,
    #[validate(length(min = 1, max = 200))]
    pub stream_key: String,
}

/// `GET /v1/live/streams/{id}/targets` — list simulcast destinations.
pub async fn list_targets(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<TargetView>>> {
    let targets = db::list_targets(state.db(), ctx.tenant_id, id).await?;
    Ok(Json(targets.into_iter().map(TargetView::from).collect()))
}

/// `POST /v1/live/streams/{id}/targets` — add a simulcast destination.
pub async fn create_target(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateTargetRequest>,
) -> ApiResult<Json<TargetView>> {
    body.validate().map_err(ApiError::Validation)?;
    db::find_live_stream(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let t = db::create_target(
        state.db(),
        ctx.tenant_id,
        id,
        body.name.trim(),
        body.url.trim(),
        body.stream_key.trim(),
    )
    .await?;
    Ok(Json(t.into()))
}

/// `DELETE /v1/live/streams/{id}/targets/{target_id}` — remove a destination.
pub async fn delete_target(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path((_id, target_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<StatusCode> {
    if db::delete_target(state.db(), ctx.tenant_id, target_id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

// --- Simulcast relay control (ingest server publish hooks) -------------------

#[derive(Deserialize)]
pub struct PublishCallback {
    stream: String,
    #[serde(default)]
    app: String,
}

/// `POST /internal/live/publish?secret=` — SRS `on_publish`. Start simulcast
/// relays for the stream's enabled targets.
pub async fn publish_hook(
    State(state): State<AppState>,
    Query(q): Query<HookQuery>,
    Json(cb): Json<PublishCallback>,
) -> (StatusCode, &'static str) {
    if q.secret != state.settings().live_hook_secret {
        return (StatusCode::FORBIDDEN, "1");
    }
    // Only ingest publishes to `live` count — ignore our own relay pushes, or the
    // relays would recursively re-trigger themselves.
    if cb.app != "live" {
        return (StatusCode::OK, "0");
    }
    // Start relays shortly after publish so the stream is pullable.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1200)).await;
        let targets = db::enabled_targets_by_stream_key(state.db(), &cb.stream)
            .await
            .unwrap_or_default();
        if targets.is_empty() {
            return;
        }
        let pull = format!("{}/live/{}", state.settings().live_rtmp_base, cb.stream);
        state.relay().start(&cb.stream, &pull, targets).await;
    });
    (StatusCode::OK, "0")
}

/// `POST /internal/live/unpublish?secret=` — SRS `on_unpublish`. Stop relays.
pub async fn unpublish_hook(
    State(state): State<AppState>,
    Query(q): Query<HookQuery>,
    Json(cb): Json<PublishCallback>,
) -> (StatusCode, &'static str) {
    if q.secret != state.settings().live_hook_secret {
        return (StatusCode::FORBIDDEN, "1");
    }
    if cb.app == "live" {
        state.relay().stop(&cb.stream).await;
    }
    (StatusCode::OK, "0")
}

// --- Instant live clip -------------------------------------------------------

#[derive(Deserialize, Validate)]
pub struct LiveClipRequest {
    /// Seconds to capture from now (1–120).
    pub duration: f64,
}

#[derive(Serialize)]
pub struct LiveClipResponse {
    pub asset_id: Uuid,
    pub filename: String,
    pub status: String,
}

/// `POST /v1/live/streams/{id}/clip` — capture the next `duration` seconds of a
/// live stream into a new asset.
pub async fn clip_live(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
    Json(body): Json<LiveClipRequest>,
) -> ApiResult<Json<LiveClipResponse>> {
    if !(body.duration >= 1.0 && body.duration <= 120.0) {
        return Err(ApiError::BadRequest(
            "duration must be 1–120 seconds".into(),
        ));
    }
    let s = db::find_live_stream(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let ingest = Ingest::from(state.settings());
    if !ingest.is_live(&s.stream_key).await {
        return Err(ApiError::Conflict("stream is not live".into()));
    }

    let asset_id = Uuid::new_v4();
    let filename = format!("{} live clip.mp4", s.name);
    let key = format!("{}/sources/{asset_id}/{filename}", ctx.tenant_id);
    let dest =
        db::create_processing_asset(state.db(), ctx.tenant_id, asset_id, &filename, &key).await?;

    let flv_url = ingest.flv_url(&s.stream_key);
    tokio::spawn(capture_live_clip(
        state,
        ctx.tenant_id,
        asset_id,
        key,
        flv_url,
        body.duration,
    ));

    Ok(Json(LiveClipResponse {
        asset_id: dest.id,
        filename: dest.filename,
        status: dest.status,
    }))
}

async fn capture_live_clip(
    state: AppState,
    tenant_id: Uuid,
    asset_id: Uuid,
    key: String,
    flv_url: String,
    duration: f64,
) {
    let tmp = std::env::temp_dir().join(format!("frt-liveclip-{asset_id}.mp4"));
    let tmp_str = tmp.to_string_lossy().to_string();

    let result = async {
        let status = tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-loglevel",
                "error",
                "-i",
                &flv_url,
                "-t",
                &format!("{duration:.1}"),
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "21",
                "-c:a",
                "aac",
                "-movflags",
                "+faststart",
                &tmp_str,
            ])
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("ffmpeg exited with {status}");
        }
        let bytes = tokio::fs::metadata(&tmp).await?.len() as i64;
        state.storage().put_file(&key, &tmp_str).await?;
        db::mark_asset_ready(state.db(), tenant_id, asset_id, Some(bytes)).await?;
        Ok::<_, anyhow::Error>(())
    }
    .await;

    let _ = tokio::fs::remove_file(&tmp).await;
    match result {
        Ok(()) => tracing::info!(asset = %asset_id, "captured live clip"),
        Err(e) => {
            tracing::error!(asset = %asset_id, error = %e, "live clip failed");
            let _ = db::set_asset_error(state.db(), tenant_id, asset_id).await;
        }
    }
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
        super::jobs::submit_job(
            &state,
            tenant_id,
            asset_id,
            None,
            &super::jobs::JobOptions::default(),
        )
        .await?;
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
