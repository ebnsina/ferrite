//! The machines converting video. Stage 0 has the storage boundary only.

#![warn(missing_docs)]

#[cfg(feature = "ffmpeg")]
pub mod asset;

#[cfg(feature = "ffmpeg")]
pub mod job;

pub mod storage;
pub mod work;
