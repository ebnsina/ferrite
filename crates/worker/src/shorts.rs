//! AI vertical shorts: transcribe → pick highlights (LLM or heuristic) → reframe
//! each to 9:16 with burned-in captions → register as new assets.

use std::path::Path;

use ferrite_stream_core::{ShortsSpec, TranscodeJob};
use ferrite_stream_storage::Storage;
use sqlx::PgPool;
use tokio::process::Command;
use uuid::Uuid;

use crate::ai::{Chat, Highlight};
use crate::captions::{self, Backend, Cue};
use crate::db;
use crate::pipeline::PipelineError;

const MIN_LEN: f64 = 8.0;
const MAX_LEN: f64 = 60.0;

pub async fn run(
    pool: &PgPool,
    job: &TranscodeJob,
    spec: &ShortsSpec,
    storage: &Storage,
    job_dir: &Path,
    transcriber: &Backend,
    chat: Option<&Chat>,
    provenance: Option<&str>,
) -> Result<usize, PipelineError> {
    db::mark_started(pool, job.id, "probing").await?;

    let source = job_dir.join("source.input");
    let source_str = source.to_string_lossy().to_string();
    storage.get_file(&job.source_key, &source_str).await?;

    // 1. Transcript with timestamps.
    db::set_state(pool, job.id, "transcoding").await?;
    let vtt_path = captions::to_vtt(transcriber, &source_str, job_dir, "transcript", job.id)
        .await
        .ok_or_else(|| {
            PipelineError::Clip(
                "no transcriber configured — set FERRITE_WHISPER_* or FERRITE_AI_*".into(),
            )
        })?;
    let vtt = tokio::fs::read_to_string(&vtt_path).await?;
    let cues = captions::parse_cues(&vtt);
    if cues.is_empty() {
        return Err(PipelineError::Clip(
            "no speech detected in the source".into(),
        ));
    }

    // 2. Highlight windows — LLM if available, else an even-split heuristic.
    let count = spec.count.clamp(1, 10);
    let highlights = select(chat, &cues, count).await;
    if highlights.is_empty() {
        return Err(PipelineError::Clip(
            "could not derive any highlights".into(),
        ));
    }
    // Burning captions needs ffmpeg's libass-backed `subtitles` filter.
    let burn = subtitles_available().await;
    if !burn {
        tracing::warn!(job = %job.id, "ffmpeg lacks the subtitles filter (libass) — shorts won't have burned captions");
    }
    tracing::info!(job = %job.id, n = highlights.len(), burn, "shorts: producing");

    // 3. Reframe each highlight and register it as a new asset.
    let total = highlights.len();
    let mut made = 0;
    for (i, h) in highlights.iter().enumerate() {
        match produce_short(
            pool,
            job,
            &source_str,
            job_dir,
            &cues,
            h,
            i,
            burn,
            storage,
            provenance,
        )
        .await
        {
            Ok(()) => made += 1,
            Err(e) => tracing::warn!(job = %job.id, error = %e, "short generation failed"),
        }
        db::set_progress(pool, job.id, (i + 1) as f32 / total as f32)
            .await
            .ok();
    }
    if made == 0 {
        return Err(PipelineError::Clip("all shorts failed to render".into()));
    }
    Ok(made)
}

/// Pick highlight windows. Falls back to an even split when no LLM is configured.
async fn select(chat: Option<&Chat>, cues: &[Cue], count: u32) -> Vec<Highlight> {
    if let Some(c) = chat {
        let transcript = cues
            .iter()
            .map(|c| format!("[{:.1}-{:.1}] {}", c.start, c.end, c.text))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(hl) = c.select_highlights(&transcript, count).await {
            let clamped: Vec<Highlight> = hl.into_iter().map(clamp_len).collect();
            if !clamped.is_empty() {
                return clamped;
            }
        }
        tracing::info!("LLM highlight selection unavailable; using heuristic");
    }
    heuristic(cues, count)
}

/// Split the speech span into `count` even windows, each anchored on a cue.
fn heuristic(cues: &[Cue], count: u32) -> Vec<Highlight> {
    let span_start = cues.first().map(|c| c.start).unwrap_or(0.0);
    let span_end = cues.last().map(|c| c.end).unwrap_or(0.0);
    let span = (span_end - span_start).max(MIN_LEN);
    let n = count.max(1) as f64;
    let step = span / n;
    (0..count)
        .map(|i| {
            let start = span_start + step * i as f64;
            let end = (start + step.min(MAX_LEN)).min(span_end);
            let title = cues
                .iter()
                .find(|c| c.start >= start)
                .map(|c| snippet(&c.text))
                .unwrap_or_else(|| format!("Clip {}", i + 1));
            clamp_len(Highlight { start, end, title })
        })
        .filter(|h| h.end - h.start >= 1.0)
        .collect()
}

fn clamp_len(mut h: Highlight) -> Highlight {
    let len = (h.end - h.start).clamp(MIN_LEN.min(h.end - h.start), MAX_LEN);
    h.end = h.start + len;
    if h.title.trim().is_empty() {
        h.title = "Short".into();
    }
    h
}

fn snippet(text: &str) -> String {
    let s: String = text
        .split_whitespace()
        .take(6)
        .collect::<Vec<_>>()
        .join(" ");
    if s.is_empty() {
        "Short".into()
    } else {
        s
    }
}

#[allow(clippy::too_many_arguments)]
async fn produce_short(
    pool: &PgPool,
    job: &TranscodeJob,
    source: &str,
    job_dir: &Path,
    cues: &[Cue],
    h: &Highlight,
    index: usize,
    burn: bool,
    storage: &Storage,
    provenance: Option<&str>,
) -> Result<(), String> {
    let out_name = format!("short{index}.mp4");
    let duration = h.end - h.start;
    // scale-to-cover then crop to a 1080x1920 canvas; optionally burn captions.
    let base = "scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920";
    let vf = if burn {
        let srt_name = format!("short{index}.srt");
        write_srt(&job_dir.join(&srt_name), cues, h.start, h.end)
            .await
            .map_err(|e| e.to_string())?;
        format!("{base},subtitles={srt_name}")
    } else {
        base.to_string()
    };
    let status = Command::new("ffmpeg")
        .current_dir(job_dir)
        .args([
            "-y",
            "-loglevel",
            "error",
            "-ss",
            &format!("{:.3}", h.start),
            "-i",
            source,
            "-t",
            &format!("{duration:.3}"),
            "-vf",
            &vf,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
            &out_name,
        ])
        .status()
        .await
        .map_err(|e| format!("spawn ffmpeg: {e}"))?;
    if !status.success() {
        return Err(format!("ffmpeg exited with {status}"));
    }

    let out_path = job_dir.join(&out_name);
    let bytes = tokio::fs::metadata(&out_path)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    // Register as a new, ready source asset.
    let asset_id = Uuid::new_v4();
    let filename = format!("{}.mp4", safe_title(&h.title));
    let key = format!("{}/sources/{asset_id}/{filename}", job.tenant_id);
    storage
        .put_file(&key, &out_path.to_string_lossy())
        .await
        .map_err(|e| e.to_string())?;
    db::create_ready_asset(pool, job.tenant_id, asset_id, &filename, &key, Some(bytes))
        .await
        .map_err(|e| e.to_string())?;

    crate::provenance::record(
        pool,
        provenance,
        job.tenant_id,
        asset_id,
        &filename,
        "shorts",
        Some(job.asset_id),
        &out_path,
    )
    .await;
    Ok(())
}

/// Whether this ffmpeg build has the libass-backed `subtitles` filter.
async fn subtitles_available() -> bool {
    Command::new("ffmpeg")
        .args(["-hide_banner", "-filters"])
        .output()
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(" subtitles "))
        .unwrap_or(false)
}

async fn write_srt(path: &Path, cues: &[Cue], start: f64, end: f64) -> std::io::Result<()> {
    let mut out = String::new();
    let mut idx = 1;
    for c in cues.iter().filter(|c| c.end > start && c.start < end) {
        let s = (c.start - start).max(0.0);
        let e = (c.end - start).min(end - start);
        out.push_str(&format!(
            "{idx}\n{} --> {}\n{}\n\n",
            srt_time(s),
            srt_time(e),
            c.text
        ));
        idx += 1;
    }
    tokio::fs::write(path, out).await
}

fn srt_time(secs: f64) -> String {
    let ms = (secs * 1000.0).round() as u64;
    let (h, rem) = (ms / 3_600_000, ms % 3_600_000);
    let (m, rem) = (rem / 60_000, rem % 60_000);
    let (s, ms) = (rem / 1000, rem % 1000);
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

fn safe_title(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "short".into()
    } else {
        trimmed.chars().take(40).collect()
    }
}
