use serde::{Deserialize, Serialize};

/// Probed information about a source video (populated from `ffprobe`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub has_audio: bool,
}

impl MediaInfo {
    /// Source resolution as a shorthand height (e.g. 1080), used to cap the
    /// rendition ladder so we never upscale.
    pub fn source_height(&self) -> u32 {
        self.height
    }
}
