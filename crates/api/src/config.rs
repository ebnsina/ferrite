//! Runtime configuration, layered from environment variables.
//!
//! All values have sensible dev defaults so `cargo run` works against the
//! bundled docker-compose stack with no `.env` edits.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_bind")]
    pub bind_addr: String,

    #[serde(default = "default_database_url")]
    pub database_url: String,

    #[serde(default = "default_redis_url")]
    pub redis_url: String,

    #[serde(default = "default_queue_group")]
    pub queue_group: String,

    #[serde(default = "default_bucket")]
    pub s3_bucket: String,

    #[serde(default)]
    pub s3_endpoint_url: Option<String>,

    #[serde(default = "default_true")]
    pub s3_force_path_style: bool,
}

fn default_bind() -> String {
    "0.0.0.0:8080".into()
}
fn default_database_url() -> String {
    "postgres://ferrite:ferrite@localhost:5432/ferrite".into()
}
fn default_redis_url() -> String {
    "redis://localhost:6379".into()
}
fn default_queue_group() -> String {
    "transcoders".into()
}
fn default_bucket() -> String {
    "ferrite".into()
}
fn default_true() -> bool {
    true
}

impl Settings {
    /// Load from environment variables (prefix `FERRITE_`), falling back to
    /// defaults. Returns a clear error if a provided value is malformed.
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(
                config::Environment::with_prefix("FERRITE")
                    // e.g. FERRITE_DATABASE_URL -> database_url; nested keys use "__".
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?
            .try_deserialize()?;
        Ok(settings)
    }
}
