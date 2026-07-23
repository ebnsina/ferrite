//! Worker config from `FERRITE_*` env vars. No hardcoded fallbacks — a missing
//! required value fails startup loudly (see `.env.example`).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub database_url: String,
    pub redis_url: String,
    pub queue_group: String,
    pub s3_bucket: String,
    #[serde(default)]
    pub s3_endpoint_url: Option<String>,
    pub s3_force_path_style: bool,

    /// Scratch directory for downloads and transcode output.
    pub work_dir: String,

    /// Max delivery attempts before a job is dead-lettered.
    pub max_attempts: usize,

    /// Max jobs a single tenant may have in-flight at once (fair scheduling).
    pub max_inflight_per_tenant: usize,

    /// Idle seconds before a stuck (unacked) job is reclaimed for retry.
    pub reclaim_min_idle_secs: u64,

    /// Whether this worker also runs the fair-dispatch scheduler loop.
    pub run_scheduler: bool,

    /// Video encoder: `cpu` (libx264) or `nvenc` (NVIDIA GPU).
    pub encoder: String,

    /// Stable identity within the consumer group; generated per-process if unset.
    #[serde(default = "default_consumer")]
    pub consumer_name: String,

    // --- Captions (all optional; unset = captions are skipped) ---
    /// Path to a whisper.cpp CLI binary (fully local transcription).
    #[serde(default)]
    pub whisper_bin: Option<String>,
    /// Path to a whisper.cpp ggml model file.
    #[serde(default)]
    pub whisper_model: Option<String>,
    /// OpenAI-compatible base URL for `/audio/transcriptions` (provider-agnostic:
    /// OpenAI, Groq, a local faster-whisper server, …). Used only if whisper.cpp
    /// isn't configured.
    #[serde(default)]
    pub ai_base_url: Option<String>,
    #[serde(default)]
    pub ai_key: Option<String>,
    /// Transcription model name (default `whisper-1`).
    #[serde(default)]
    pub ai_model: Option<String>,
}

fn default_consumer() -> String {
    format!("worker-{}", std::process::id())
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        config::Config::builder()
            .add_source(
                config::Environment::with_prefix("FERRITE")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?
            .try_deserialize()
            .map_err(|e| anyhow::anyhow!("invalid/missing config ({e}); see .env.example"))
    }
}
