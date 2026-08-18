//! FFmpeg wrapper. All `unsafe` in the workspace lives here.
//! The `ffmpeg` feature is off by default so the types and seam build without it.

#![warn(missing_docs)]

pub mod encoder;
pub mod error;
pub mod media;

#[cfg(feature = "ffmpeg")]
mod probe;

pub use encoder::{
    BackendId, BackendRegistry, CancelSignal, EncodeSpec, EncoderBackend, Frame, NeverCancel,
    Packet, PixelFormat, Preset, Provenance, RateControl, Resolution, VideoCodecName, VideoEncoder,
};
pub use error::{AvError, Result};
pub use media::{AudioStream, ColorInfo, MediaInfo, Rational, VideoStream, Warning};

#[cfg(feature = "ffmpeg")]
pub use probe::probe;

/// The FFmpeg build this binary is linked against. Recorded per rendition.
pub fn ffmpeg_version() -> String {
    #[cfg(feature = "ffmpeg")]
    {
        format!(
            "{}.{}.{}",
            ffmpeg_next::util::version() >> 16,
            (ffmpeg_next::util::version() >> 8) & 0xff,
            ffmpeg_next::util::version() & 0xff
        )
    }
    #[cfg(not(feature = "ffmpeg"))]
    {
        "none".to_string()
    }
}

/// Whether this build can actually touch video.
pub const fn has_ffmpeg() -> bool {
    cfg!(feature = "ffmpeg")
}

/// Initialize FFmpeg. Idempotent; call once per process before anything else.
pub fn init() -> Result<()> {
    #[cfg(feature = "ffmpeg")]
    {
        ffmpeg_next::init().map_err(|e| AvError::Ffmpeg {
            op: "ffmpeg init",
            source: Box::new(e),
        })?;
        ffmpeg_next::util::log::set_level(ffmpeg_next::util::log::Level::Warning);
    }
    Ok(())
}
