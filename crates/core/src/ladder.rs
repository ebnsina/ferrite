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

    /// Content-aware (per-title) ladder: cap to source resolution, then cap each
    /// rung's bitrate to the source's own bitrate (encoding above it buys no
    /// quality) and collapse rungs that would land within ~15% of each other —
    /// keeping the higher-resolution one. Cuts egress on low-complexity sources
    /// (screencasts, talking heads) without touching high-bitrate content.
    pub fn content_aware(&self, media: &MediaInfo) -> Ladder {
        let base = self.cap_to_source(media);
        let Some(src_kbps) = media.bitrate_kbps.filter(|&b| b > 0) else {
            return base; // unknown source bitrate — keep the capped standard ladder
        };
        // Leave audio headroom; never below a sane floor.
        let video_ceiling = src_kbps.saturating_sub(128).max(150);

        let mut out: Vec<Rendition> = Vec::new();
        let mut prev = 0u32;
        for r in &base.renditions {
            let target = r.bitrate_kbps.min(video_ceiling);
            let redundant = prev > 0 && (target as f32) <= (prev as f32) * 1.15;
            if redundant {
                // Same bitrate ceiling as the previous rung → prefer more pixels.
                if let Some(last) = out.last_mut() {
                    *last = Rendition::new(&r.name, r.height, target);
                    prev = target;
                }
                continue;
            }
            out.push(Rendition::new(&r.name, r.height, target));
            prev = target;
        }
        if out.is_empty() {
            out.push(base.renditions[0].clone());
        }
        Ladder { renditions: out }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::MediaInfo;

    fn media(height: u32, bitrate_kbps: Option<u32>) -> MediaInfo {
        MediaInfo {
            duration_secs: 10.0,
            width: height * 16 / 9,
            height,
            fps: 30.0,
            video_codec: "h264".into(),
            audio_codec: Some("aac".into()),
            bitrate_kbps,
            has_audio: true,
        }
    }

    #[test]
    fn high_bitrate_1080p_keeps_full_ladder() {
        let l = Ladder::default_abr().content_aware(&media(1080, Some(8000)));
        assert_eq!(l.renditions.len(), 5);
        assert_eq!(l.renditions.last().unwrap().bitrate_kbps, 5000);
    }

    #[test]
    fn low_bitrate_1080p_collapses_to_few_rungs() {
        let l = Ladder::default_abr().content_aware(&media(1080, Some(1000)));
        // Should not emit five ~1000kbps rungs; top rung is 1080p at the ceiling.
        assert!(l.renditions.len() < 5);
        let top = l.renditions.last().unwrap();
        assert_eq!(top.height, 1080);
        assert!(top.bitrate_kbps <= 872);
    }

    #[test]
    fn unknown_bitrate_falls_back_to_capped_standard() {
        let l = Ladder::default_abr().content_aware(&media(720, None));
        assert_eq!(l.renditions.len(), 4); // 240..720
    }
}
