//! Transcode job submission and status.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use ferrite_core::{Ladder, TranscodeJob, Watermark};
use ferrite_queue::JobQueue;
use futures::Stream;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::TenantContext;
use crate::db::{self, Job};
use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct JobView {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub state: String,
    pub progress: f32,
    pub error: Option<String>,
    pub queued_at: String,
    pub finished_at: Option<String>,
    /// HLS master playlist proxy URL; present once the job is completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playback_url: Option<String>,
    /// MPEG-DASH manifest proxy URL; present once the job is completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash_url: Option<String>,
    /// Poster image proxy URL; present once the job is completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poster_url: Option<String>,
    /// Public WebVTT storyboard (scrubbing thumbnails); present once completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storyboard_url: Option<String>,
    /// Progressive MP4 download; present when produced and completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mp4_url: Option<String>,
    /// Audio-only download; present when produced and completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
    /// WebVTT captions track; present when produced and completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captions_url: Option<String>,
    /// Translated caption tracks (lang → signed URL).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub caption_tracks: Vec<CaptionTrack>,
}

#[derive(Serialize)]
pub struct CaptionTrack {
    pub lang: String,
    pub url: String,
}

impl From<Job> for JobView {
    fn from(j: Job) -> Self {
        JobView {
            id: j.id,
            asset_id: j.asset_id,
            state: j.state,
            progress: j.progress,
            error: j.error,
            queued_at: j.queued_at.to_rfc3339(),
            finished_at: j.finished_at.map(|t| t.to_rfc3339()),
            playback_url: None,
            dash_url: None,
            poster_url: None,
            storyboard_url: None,
            mp4_url: None,
            audio_url: None,
            captions_url: None,
            caption_tracks: Vec::new(),
        }
    }
}

/// Transcode output options from the API request.
#[derive(Deserialize, Default)]
pub struct JobOptions {
    #[serde(default)]
    pub encrypt: bool,
    #[serde(default)]
    pub mp4: bool,
    #[serde(default)]
    pub audio: bool,
    #[serde(default)]
    pub captions: bool,
    #[serde(default)]
    pub watermark: Option<WatermarkOpt>,
}

#[derive(Deserialize)]
pub struct WatermarkOpt {
    /// Corner: tl | tr | bl | br.
    pub position: String,
    pub opacity: f32,
}

#[derive(Deserialize)]
pub struct CreateJobRequest {
    pub asset_id: Uuid,
    /// Optional; a repeated key returns the same job instead of a duplicate.
    pub idempotency_key: Option<String>,
    #[serde(flatten)]
    pub options: JobOptions,
}

/// `POST /v1/jobs` — submit a transcode job for a ready asset.
pub async fn create_job(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<CreateJobRequest>,
) -> ApiResult<Json<JobView>> {
    let job = submit_job(
        &state,
        ctx.tenant_id,
        body.asset_id,
        body.idempotency_key.as_deref(),
        &body.options,
    )
    .await?;
    Ok(Json(job.into()))
}

/// Validate a ready asset, create its job, and enqueue it (idempotency-safe).
pub(crate) async fn submit_job(
    state: &AppState,
    tenant_id: Uuid,
    asset_id: Uuid,
    idempotency_key: Option<&str>,
    options: &JobOptions,
) -> ApiResult<Job> {
    let asset = db::find_asset(state.db(), tenant_id, asset_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if asset.status != "ready" {
        return Err(ApiError::Conflict(format!(
            "asset is not ready for transcoding (status: {})",
            asset.status
        )));
    }

    // A watermark only lands on the MP4 download, so it implies mp4.
    let want_mp4 = options.mp4 || options.watermark.is_some();

    let job_id = Uuid::new_v4();
    let output_prefix = format!("{tenant_id}/outputs/{job_id}");
    let (job, created) = db::create_job(
        state.db(),
        tenant_id,
        job_id,
        asset_id,
        &output_prefix,
        idempotency_key,
        want_mp4,
        options.audio,
        options.captions,
    )
    .await?;

    if created {
        let watermark = options.watermark.as_ref().map(|w| Watermark {
            logo_key: super::brand::logo_key(tenant_id),
            position: w.position.clone(),
            opacity: w.opacity,
        });
        let transcode = TranscodeJob {
            id: job.id,
            tenant_id,
            asset_id,
            source_key: asset.original_key,
            output_prefix,
            ladder: Ladder::default_abr(),
            hls: true,
            dash: true,
            thumbnails: true,
            encrypt: options.encrypt,
            encryption_key: None,
            clip: None,
            shorts: None,
            mp4: want_mp4,
            audio: options.audio,
            captions: options.captions,
            watermark,
        };
        state.queue().enqueue(&transcode).await?;
        tracing::info!(job = %job.id, tenant = %tenant_id, "job enqueued");
    }
    Ok(job)
}

const MAX_BATCH: usize = 500;

#[derive(Deserialize)]
pub struct BatchJobRequest {
    pub asset_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct BatchResult {
    pub submitted: Vec<JobView>,
    pub skipped: Vec<SkippedItem>,
}

#[derive(Serialize)]
pub struct SkippedItem {
    pub asset_id: Uuid,
    pub reason: String,
}

/// `POST /v1/jobs/batch` — submit many assets at once. Partial success: each
/// asset is reported as submitted or skipped; the fair queue spreads the load.
pub async fn create_jobs_batch(
    State(state): State<AppState>,
    ctx: TenantContext,
    Json(body): Json<BatchJobRequest>,
) -> ApiResult<Json<BatchResult>> {
    if body.asset_ids.is_empty() {
        return Err(ApiError::BadRequest("asset_ids is empty".into()));
    }
    if body.asset_ids.len() > MAX_BATCH {
        return Err(ApiError::BadRequest(format!(
            "batch too large ({}, max {MAX_BATCH})",
            body.asset_ids.len()
        )));
    }

    let mut submitted = Vec::new();
    let mut skipped = Vec::new();
    for asset_id in body.asset_ids {
        match submit_job(
            &state,
            ctx.tenant_id,
            asset_id,
            None,
            &JobOptions::default(),
        )
        .await
        {
            Ok(job) => submitted.push(job.into()),
            Err(e) => skipped.push(SkippedItem {
                asset_id,
                reason: e.to_string(),
            }),
        }
    }
    Ok(Json(BatchResult { submitted, skipped }))
}

/// `GET /v1/jobs` — list the tenant's jobs.
pub async fn list_jobs(
    State(state): State<AppState>,
    ctx: TenantContext,
) -> ApiResult<Json<Vec<JobView>>> {
    let jobs = db::list_jobs(state.db(), ctx.tenant_id).await?;
    Ok(Json(jobs.into_iter().map(JobView::from).collect()))
}

#[derive(Serialize)]
pub struct CueView {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// `GET /v1/jobs/{id}/transcript` — the job's transcript cues (for the
/// interactive transcript panel).
pub async fn job_transcript(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<CueView>>> {
    db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let cues = db::transcript_for_job(state.db(), ctx.tenant_id, id).await?;
    Ok(Json(
        cues.into_iter()
            .map(|(s, e, t)| CueView {
                start: s as f64,
                end: e as f64,
                text: t,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct TranslateRequest {
    /// Target language, e.g. "Spanish" or "es".
    pub lang: String,
}

/// `POST /v1/jobs/{id}/translate` — translate the job's transcript into another
/// language and publish it as an additional caption track.
pub async fn translate_captions(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
    Json(body): Json<TranslateRequest>,
) -> ApiResult<Json<CaptionTrack>> {
    let lang = body.lang.trim().to_lowercase();
    if lang.is_empty() || lang.len() > 40 {
        return Err(ApiError::BadRequest("invalid language".into()));
    }
    let translator = crate::ai::Translator::from_settings(state.settings())
        .ok_or_else(|| ApiError::Unavailable("AI translation is not configured".into()))?;

    let job = db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;
    if job.state != "completed" {
        return Err(ApiError::Conflict("job is not completed".into()));
    }
    let cues = db::transcript_for_job(state.db(), ctx.tenant_id, id).await?;
    if cues.is_empty() {
        return Err(ApiError::Conflict(
            "no transcript to translate (transcode with captions first)".into(),
        ));
    }

    let texts: Vec<String> = cues.iter().map(|(_, _, t)| t.clone()).collect();
    let translated = translator
        .translate(&texts, &lang)
        .await
        .ok_or_else(|| ApiError::Unavailable("translation failed".into()))?;

    // Rebuild a WebVTT from the original timings + translated text.
    let mut vtt = String::from("WEBVTT\n\n");
    for ((start, end, _), text) in cues.iter().zip(translated.iter()) {
        vtt.push_str(&format!(
            "{} --> {}\n{}\n\n",
            vtt_time(*start as f64),
            vtt_time(*end as f64),
            text
        ));
    }

    let key = format!("{}/captions.{lang}.vtt", job.output_prefix);
    state
        .storage()
        .put_bytes(&key, vtt.into_bytes())
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    db::insert_caption_track(state.db(), ctx.tenant_id, id, &lang).await?;

    let s = state.settings();
    let exp = super::playback::now_unix() + super::playback::TOKEN_TTL_SECS;
    let token = super::playback::sign_token(&s.playback_secret, ctx.tenant_id, id, exp);
    Ok(Json(CaptionTrack {
        lang: lang.clone(),
        url: format!(
            "{}/playback/{id}/captions.{lang}.vtt?token={token}",
            s.public_url
        ),
    }))
}

fn vtt_time(secs: f64) -> String {
    let ms = (secs * 1000.0).round() as u64;
    let (h, rem) = (ms / 3_600_000, ms % 3_600_000);
    let (m, rem) = (rem / 60_000, rem % 60_000);
    let (s, ms) = (rem / 1000, rem % 1000);
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

/// `GET /v1/jobs/:id` — fetch a single job's status.
pub async fn get_job(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<JobView>> {
    let job = db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let langs = if job.state == "completed" {
        db::list_caption_langs(state.db(), ctx.tenant_id, id)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    Ok(Json(view_with_urls(&state, ctx.tenant_id, job, &langs)))
}

/// Build a JobView, adding tokenized playback proxy URLs once completed.
/// Outputs are private in storage; the proxy authorizes delivery via a
/// short-lived token (see [`super::playback`]).
fn view_with_urls(
    state: &AppState,
    tenant_id: Uuid,
    job: Job,
    caption_langs: &[String],
) -> JobView {
    let completed = job.state == "completed";
    let job_id = job.id;
    let encrypted = job.encrypted;
    let has_mp4 = job.has_mp4;
    let has_audio = job.has_audio;
    let has_captions = job.has_captions;
    // Only transcode jobs package an HLS/DASH stream (non-empty output prefix).
    // Clip/shorts jobs produce a new asset instead, so they have no stream to play.
    let has_stream = !job.output_prefix.is_empty();
    let mut view = JobView::from(job);
    if completed {
        let s = state.settings();
        let exp = super::playback::now_unix() + super::playback::TOKEN_TTL_SECS;
        let token = super::playback::sign_token(&s.playback_secret, tenant_id, job_id, exp);
        let base = &s.public_url;
        if has_mp4 {
            view.mp4_url = Some(format!(
                "{base}/playback/{job_id}/download.mp4?token={token}"
            ));
        }
        if has_audio {
            view.audio_url = Some(format!("{base}/playback/{job_id}/audio.m4a?token={token}"));
        }
        if has_captions {
            view.captions_url = Some(format!(
                "{base}/playback/{job_id}/captions.vtt?token={token}"
            ));
        }
        if has_stream {
            view.playback_url = Some(format!(
                "{base}/playback/{job_id}/master.m3u8?token={token}"
            ));
            // Encrypted jobs have no DASH package (would be plaintext). Unencrypted
            // jobs use CMAF: manifest.mpd sits alongside the HLS master.
            if !encrypted {
                view.dash_url = Some(format!(
                    "{base}/playback/{job_id}/manifest.mpd?token={token}"
                ));
            }
            view.poster_url = Some(format!(
                "{base}/playback/{job_id}/thumbs/poster.jpg?token={token}"
            ));
            view.storyboard_url = Some(format!(
                "{base}/playback/{job_id}/thumbs/thumbs.vtt?token={token}"
            ));
            view.caption_tracks = caption_langs
                .iter()
                .map(|lang| CaptionTrack {
                    lang: lang.clone(),
                    url: format!("{base}/playback/{job_id}/captions.{lang}.vtt?token={token}"),
                })
                .collect();
        }
    }
    view
}

const SSE_POLL: Duration = Duration::from_millis(1000);
const SSE_MAX_TICKS: u32 = 30 * 60; // ~30 min safety cap

/// `GET /v1/jobs/:id/events` — Server-Sent Events stream of job status until it
/// reaches a terminal state. One long-lived connection replaces client polling.
pub async fn job_events(
    State(state): State<AppState>,
    ctx: TenantContext,
    Path(id): Path<Uuid>,
) -> ApiResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    // Verify ownership before opening the stream.
    db::find_job(state.db(), ctx.tenant_id, id)
        .await?
        .ok_or(ApiError::NotFound)?;

    let tenant_id = ctx.tenant_id;
    let stream = async_stream::stream! {
        let mut ticker = tokio::time::interval(SSE_POLL);
        for _ in 0..SSE_MAX_TICKS {
            ticker.tick().await;
            match db::find_job(state.db(), tenant_id, id).await {
                Ok(Some(job)) => {
                    let terminal = job.state == "completed" || job.state == "failed";
                    let view = view_with_urls(&state, tenant_id, job, &[]);
                    if let Ok(event) = Event::default().json_data(view) {
                        yield Ok(event);
                    }
                    if terminal { break; }
                }
                _ => break,
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
