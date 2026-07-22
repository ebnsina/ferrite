//! Runtime config from `FERRITE_*` env vars. No hardcoded fallbacks — a missing
//! required value fails startup loudly (see `.env.example`).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    pub bind_addr: String,
    pub database_url: String,
    pub redis_url: String,
    pub queue_group: String,
    /// Max jobs a single tenant may have in-flight at once (fair scheduling).
    pub max_inflight_per_tenant: usize,
    /// Idle seconds before a stuck (unacked) job is reclaimed for retry.
    pub reclaim_min_idle_secs: u64,
    pub s3_bucket: String,
    /// Custom S3 endpoint (MinIO). Absent = AWS default resolver — the one
    /// legitimately optional value, not a fallback.
    #[serde(default)]
    pub s3_endpoint_url: Option<String>,
    pub s3_force_path_style: bool,
    /// Public base URL (incl. bucket) for playback assets, e.g.
    /// `http://localhost:9100/ferrite` or a CDN domain.
    pub s3_public_url: String,
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
