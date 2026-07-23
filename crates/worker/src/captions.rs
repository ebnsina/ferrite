//! Auto-captions: transcribe the audio into a WebVTT track. Provider-agnostic —
//! prefers a fully local whisper.cpp CLI, otherwise any OpenAI-compatible
//! `/audio/transcriptions` endpoint. Unconfigured → skipped (no failure).

use std::path::Path;

use ferrite_core::{Artifact, ArtifactKind, TranscodeJob};
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

/// Produce `captions.vtt` for the job. `None` if unconfigured or on failure
/// (best-effort — never fails the transcode).
pub async fn generate(
    backend: &Backend,
    job: &TranscodeJob,
    source: &str,
    dir: &Path,
) -> Option<Artifact> {
    if !backend.is_configured() {
        tracing::info!(job = %job.id, "captions requested but no transcriber configured — skipping");
        return None;
    }

    // 16 kHz mono WAV is what whisper models expect.
    let wav = dir.join("audio16k.wav");
    let wav_str = wav.to_string_lossy().to_string();
    if let Err(e) = extract_wav(source, &wav_str).await {
        tracing::warn!(job = %job.id, error = %e, "captions: audio extract failed");
        return None;
    }

    let vtt = dir.join("captions.vtt");
    let vtt_str = vtt.to_string_lossy().to_string();

    let result = match backend {
        Backend::WhisperCpp(bin, model) => whisper_cpp(bin, model, &wav_str, &vtt, dir).await,
        Backend::OpenAiCompatible(base, key, model) => {
            openai_compatible(base, key, model, &wav_str, &vtt_str).await
        }
        Backend::None => Err("no backend".into()),
    };

    match result {
        Ok(()) => {
            let bytes = tokio::fs::metadata(&vtt)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            Some(Artifact {
                kind: ArtifactKind::HlsPlaylist, // text sidecar; served as-is
                local_path: vtt_str,
                key: format!("{}/captions.vtt", job.output_prefix),
                rendition: None,
                bytes,
            })
        }
        Err(e) => {
            tracing::warn!(job = %job.id, error = %e, "captions: transcription failed");
            None
        }
    }
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

/// whisper.cpp writes `{of}.vtt`; we point `-of` at the captions path (minus ext).
async fn whisper_cpp(
    bin: &str,
    model: &str,
    wav: &str,
    vtt: &Path,
    dir: &Path,
) -> Result<(), String> {
    let of = dir.join("captions"); // whisper.cpp appends .vtt
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
