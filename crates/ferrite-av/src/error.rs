//! Errors crossing the FFmpeg boundary.

/// Result alias used throughout `ferrite-av`.
pub type Result<T, E = AvError> = std::result::Result<T, E>;

/// Everything that can go wrong inside the FFmpeg wrapper.
/// Callers never see a raw `AVERROR`.
#[derive(Debug, thiserror::Error)]
pub enum AvError {
    /// The file could not be opened or its container could not be read.
    #[error("cannot open input {path}: {source}")]
    OpenInput {
        /// Path we were asked to read.
        path: String,
        /// Underlying FFmpeg error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The container opened but has no stream we can work with.
    #[error("no usable {kind} stream")]
    NoStream {
        /// `"video"` or `"audio"`.
        kind: &'static str,
    },

    /// A decoder or encoder could not be constructed.
    #[error("codec {codec} unavailable: {reason}")]
    CodecUnavailable {
        /// Codec name as we asked for it.
        codec: String,
        /// Why the build or lookup failed.
        reason: String,
    },

    /// No registered backend can produce the requested codec.
    #[error("no encoder backend supports {codec}")]
    NoBackend {
        /// Codec the caller asked for.
        codec: VideoCodecName,
    },

    /// The encode spec is internally inconsistent — caught before FFmpeg sees it.
    #[error("invalid encode spec: {0}")]
    InvalidSpec(String),

    /// FFmpeg returned an error mid-stream.
    #[error("{op} failed: {source}")]
    Ffmpeg {
        /// What we were doing.
        op: &'static str,
        /// Underlying FFmpeg error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The caller cancelled between frames. Not a failure.
    #[error("encode cancelled")]
    Cancelled,

    /// This build of `ferrite-av` was compiled without the `ffmpeg` feature.
    #[error("ferrite-av was built without the `ffmpeg` feature; rebuild with --features ffmpeg")]
    NotBuiltWithFfmpeg,

    /// Anything filesystem-shaped.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

use crate::encoder::VideoCodecName;
