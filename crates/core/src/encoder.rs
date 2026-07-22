use std::future::Future;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::job::TranscodeJob;
use crate::media::MediaInfo;

/// The kind of artifact a transcode produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    HlsPlaylist,
    HlsSegment,
    DashManifest,
    DashSegment,
    Thumbnail,
    Sprite,
    Vtt,
}

/// One output file produced by an encoder, ready to upload to object storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub kind: ArtifactKind,
    /// Local path on the worker before upload.
    pub local_path: String,
    /// Destination key within the job's output prefix.
    pub key: String,
    pub rendition: Option<String>,
    pub bytes: u64,
}

/// Errors an encoder can surface. Kept coarse on purpose — callers map these to
/// job failures and retry policy, not fine-grained control flow.
#[derive(Debug, Error)]
pub enum TranscodeError {
    #[error("source could not be probed: {0}")]
    Probe(String),
    #[error("ffmpeg failed: {0}")]
    Ffmpeg(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unsupported input: {0}")]
    Unsupported(String),
}

/// A sink the encoder calls to report progress (0.0..=1.0) for a rendition.
/// The worker wires this to the queue/DB so the dashboard can stream updates.
pub type ProgressSink<'a> = &'a (dyn Fn(&str, f32) + Send + Sync);

/// The single contract every encoder implements.
///
/// `CpuEncoder` (FFmpeg, software) satisfies this today; a future
/// `NvencEncoder` (GPU) implements the same trait and drops into the worker
/// with no pipeline changes.
pub trait Encoder: Send + Sync {
    /// Inspect a source and return its media characteristics.
    fn probe(
        &self,
        source_path: &str,
    ) -> impl Future<Output = Result<MediaInfo, TranscodeError>> + Send;

    /// Transcode a job into one or more artifacts, reporting progress as it goes.
    fn transcode<'a>(
        &'a self,
        job: &'a TranscodeJob,
        media: &'a MediaInfo,
        on_progress: ProgressSink<'a>,
    ) -> impl Future<Output = Result<Vec<Artifact>, TranscodeError>> + Send;
}
