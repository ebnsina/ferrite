//! Worker configuration (env-driven, dev defaults match the API + compose stack).

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
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

    /// Scratch directory for downloads and transcode output.
    #[serde(default = "default_work_dir")]
    pub work_dir: String,

    /// A stable identity for this worker within the consumer group.
    #[serde(default = "default_consumer")]
    pub consumer_name: String,

    /// Max delivery attempts before a job is dead-lettered.
    #[serde(default = "default_max_attempts")]
    pub max_attempts: usize,
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
fn default_work_dir() -> String {
    "/tmp/ferrite-work".into()
}
fn default_consumer() -> String {
    format!("worker-{}", std::process::id())
}
fn default_max_attempts() -> usize {
    3
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(
                config::Environment::with_prefix("FERRITE")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?
            .try_deserialize()?;
        Ok(settings)
    }
}
