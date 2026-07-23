use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ladder::Ladder;

/// Lifecycle of a transcode job. Persisted as a string in Postgres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Probing,
    Transcoding,
    Packaging,
    Uploading,
    Completed,
    Failed,
}

impl JobState {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Probing => "probing",
            JobState::Transcoding => "transcoding",
            JobState::Packaging => "packaging",
            JobState::Uploading => "uploading",
            JobState::Completed => "completed",
            JobState::Failed => "failed",
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, JobState::Completed | JobState::Failed)
    }
}

/// A unit of transcode work handed to a worker via the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscodeJob {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub asset_id: Uuid,
    /// Object-storage key of the uploaded source.
    pub source_key: String,
    /// Destination prefix for all generated artifacts.
    pub output_prefix: String,
    pub ladder: Ladder,
    pub hls: bool,
    pub dash: bool,
    pub thumbnails: bool,
    /// Encrypt HLS output with AES-128.
    #[serde(default)]
    pub encrypt: bool,
    /// Hex AES-128 key, populated by the worker at runtime (not by the API).
    #[serde(default)]
    pub encryption_key: Option<String>,
    /// When set, this job trims the source into a new asset instead of
    /// transcoding it (the transcode ladder/flags are ignored).
    #[serde(default)]
    pub clip: Option<Clip>,
}

/// A trim operation: cut `[start_secs, end_secs)` of the source into a new asset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub start_secs: f64,
    pub end_secs: f64,
    /// The new asset this clip produces.
    pub dest_asset_id: Uuid,
    /// Object-storage key to upload the trimmed file to.
    pub dest_key: String,
}
