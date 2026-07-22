use serde::{Deserialize, Serialize};

use crate::media::MediaInfo;

/// A single output rendition target in an adaptive-bitrate ladder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rendition {
    pub name: String,
    pub height: u32,
    pub bitrate_kbps: u32,
    pub max_bitrate_kbps: u32,
}

impl Rendition {
    pub fn new(name: &str, height: u32, bitrate_kbps: u32) -> Self {
        Self {
            name: name.to_string(),
            height,
            bitrate_kbps,
            max_bitrate_kbps: (bitrate_kbps as f32 * 1.5) as u32,
        }
    }
}

/// An ordered set of renditions. A profile references one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ladder {
    pub renditions: Vec<Rendition>,
}

impl Ladder {
    /// A sensible default ABR ladder (240p → 1080p).
    pub fn default_abr() -> Self {
        Self {
            renditions: vec![
                Rendition::new("240p", 240, 400),
                Rendition::new("360p", 360, 800),
                Rendition::new("480p", 480, 1400),
                Rendition::new("720p", 720, 2800),
                Rendition::new("1080p", 1080, 5000),
            ],
        }
    }

    /// Drop renditions that would upscale the source. Always keeps at least the
    /// smallest rendition so tiny sources still get one output.
    pub fn cap_to_source(&self, media: &MediaInfo) -> Ladder {
        let src = media.source_height();
        let mut kept: Vec<Rendition> = self
            .renditions
            .iter()
            .filter(|r| r.height <= src)
            .cloned()
            .collect();
        if kept.is_empty() {
            if let Some(smallest) = self.renditions.iter().min_by_key(|r| r.height) {
                kept.push(smallest.clone());
            }
        }
        Ladder { renditions: kept }
    }
}
