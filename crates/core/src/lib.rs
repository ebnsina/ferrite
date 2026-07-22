//! Ferrite core domain: shared types, job model, and the encoder abstraction.
//!
//! This crate has no I/O dependencies. It defines *what* a transcode is so that
//! the CPU encoder today and a GPU (NVENC) encoder later implement one contract.

pub mod encoder;
pub mod job;
pub mod ladder;
pub mod media;

pub use encoder::{Artifact, ArtifactKind, Encoder, ProgressSink, TranscodeError};
pub use job::{JobState, TranscodeJob};
pub use ladder::{Ladder, Rendition};
pub use media::MediaInfo;
