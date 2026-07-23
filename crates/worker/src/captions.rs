//! Auto-captions: transcribe the audio into a WebVTT track. Provider-agnostic —
//! prefers a fully local whisper.cpp CLI, otherwise any OpenAI-compatible
//! `/audio/transcriptions` endpoint. Unconfigured → skipped (no failure).

use std::path::{Path, PathBuf};

use tokio::process::Command;

use crate::config::Settings;

/// Resolved transcription backend (or none).
#[derive(Clone)]
pub enum Backend {
    /// whisper.cpp CLI: (binary, model).
    WhisperCpp(String, String),
    /// OpenAI-compatible HTTP: (base_url, api_key, model).
    OpenAiCompatible(String, String, String),
    None,
}

impl Backend {
    pub fn from_settings(s: &Settings) -> Self {
        match (&s.whisper_bin, &s.whisper_model) {
            (Some(bin), Some(model)) => Backend::WhisperCpp(bin.clone(), model.clone()),
            _ => match (&s.ai_base_url, &s.ai_key) {
                (Some(base), Some(key)) => Backend::OpenAiCompatible(
                    base.trim_end_matches('/').to_string(),
                    key.clone(),
                    s.ai_model
                        .clone()
                        .unwrap_or_else(|| "whisper-1".to_string()),
                ),
                _ => Backend::None,
            },
        }
    }

    pub fn is_configured(&self) -> bool {
        !matches!(self, Backend::None)
    }
}

/// A transcript cue with absolute timestamps (seconds).
#[derive(Debug, Clone)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// Transcribe `source` into `{base}.vtt` inside `dir`; returns its path.
/// `None` if unconfigured or on failure (best-effort).
pub async fn to_vtt(
    backend: &Backend,
    source: &str,
    dir: &Path,
    base: &str,
    job_id: uuid::Uuid,
) -> Option<PathBuf> {
    if !backend.is_configured() {
        tracing::info!(job = %job_id, "transcription requested but no transcriber configured");
        return None;
    }

    let wav = dir.join("audio16k.wav");
    let wav_str = wav.to_string_lossy().to_string();
    if let Err(e) = extract_wav(source, &wav_str).await {
        tracing::warn!(job = %job_id, error = %e, "audio extract failed");
        return None;
    }

    let vtt = dir.join(format!("{base}.vtt"));
    let result = match backend {
        Backend::WhisperCpp(bin, model) => whisper_cpp(bin, model, &wav_str, &vtt, dir, base).await,
        Backend::OpenAiCompatible(b, key, model) => {
            openai_compatible(b, key, model, &wav_str, &vtt.to_string_lossy()).await
        }
        Backend::None => Err("no backend".into()),
    };

    match result {
        Ok(()) => Some(vtt),
        Err(e) => {
            tracing::warn!(job = %job_id, error = %e, "transcription failed");
            None
        }
    }
}

/// Parse a WebVTT string into cues.
pub fn parse_cues(vtt: &str) -> Vec<Cue> {
    let mut cues = Vec::new();
    let mut lines = vtt.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((a, b)) = line.split_once("-->") else {
            continue;
        };
        let (Some(start), Some(end)) = (parse_ts(a.trim()), parse_ts(b.trim())) else {
            continue;
        };
        let mut text = String::new();
        while let Some(&next) = lines.peek() {
            if next.trim().is_empty() {
                break;
            }
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(next.trim());
            lines.next();
        }
        if !text.is_empty() {
            cues.push(Cue { start, end, text });
        }
    }
    cues
}

/// `HH:MM:SS.mmm` or `MM:SS.mmm` → seconds.
fn parse_ts(s: &str) -> Option<f64> {
    let s = s.split_whitespace().next()?; // drop cue settings after the time
    let parts: Vec<&str> = s.split(':').collect();
    let mut secs = 0.0;
    for p in &parts {
        secs = secs * 60.0 + p.replace(',', ".").parse::<f64>().ok()?;
    }
    Some(secs)
}

async fn extract_wav(source: &str, out: &str) -> Result<(), String> {
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-i",
            source,
            "-vn",
            "-ar",
            "16000",
            "-ac",
            "1",
            "-f",
            "wav",
            out,
        ])
        .status()
        .await
        .map_err(|e| format!("spawn ffmpeg: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ffmpeg exited with {status}"))
    }
}

async fn whisper_cpp(
    bin: &str,
    model: &str,
    wav: &str,
    vtt: &Path,
    dir: &Path,
    base: &str,
) -> Result<(), String> {
    let of = dir.join(base); // whisper.cpp appends .vtt
    let status = Command::new(bin)
        .args([
            "-m",
            model,
            "-f",
            wav,
            "-ovtt",
            "-of",
            &of.to_string_lossy(),
        ])
        .status()
        .await
        .map_err(|e| format!("spawn whisper: {e}"))?;
    if !status.success() {
        return Err(format!("whisper exited with {status}"));
    }
    if tokio::fs::try_exists(vtt).await.unwrap_or(false) {
        Ok(())
    } else {
        Err("whisper produced no vtt".into())
    }
}

async fn openai_compatible(
    base: &str,
    key: &str,
    model: &str,
    wav: &str,
    vtt_out: &str,
) -> Result<(), String> {
    let bytes = tokio::fs::read(wav).await.map_err(|e| e.to_string())?;
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| e.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text("model", model.to_string())
        .text("response_format", "vtt")
        .part("file", part);

    let resp = reqwest::Client::new()
        .post(format!("{base}/audio/transcriptions"))
        .bearer_auth(key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("transcription API {}", resp.status()));
    }
    let vtt = resp.text().await.map_err(|e| e.to_string())?;
    tokio::fs::write(vtt_out, vtt)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}
