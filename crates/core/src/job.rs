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
}
