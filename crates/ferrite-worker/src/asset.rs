//! Asset mode end to end: a source in, a playable directory out.
//!
//! The same steps the fleet runs, on one machine and with no cluster. Job mode
//! shares steps 1–3 with it — see [`crate::job`].

use ferrite_av::encoder::Preset;
use ferrite_av::package::{Input, Track};
use ferrite_av::transcode::Output;
use ferrite_av::{AvError, NeverCancel};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What to build.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Speed against efficiency, from the plan.
    pub preset: Preset,
    /// Encode only the rung that makes the asset playable.
    pub fast_only: bool,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            preset: Preset::Medium,
            fast_only: false,
        }
    }
}

/// One video rendition that was produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rendition {
    /// Rendition name, which is also its directory.
    pub name: String,
    /// Output size.
    pub width: u32,
    /// Output size.
    pub height: u32,
    /// Frames encoded.
    pub frames: u64,
    /// Compressed bytes.
    pub bytes: u64,
}

/// A playable asset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Published {
    /// HLS master playlist.
    pub hls: PathBuf,
    /// DASH manifest.
    pub dash: PathBuf,
    /// Video renditions, largest first.
    pub renditions: Vec<Rendition>,
    /// Whether an audio track was produced. Silence is not a failure.
    pub audio: bool,
    /// The contact sheet a reviewer opens.
    pub contact_sheet: PathBuf,
    /// Perceptual hashes, for the blocklist.
    pub hashes: Vec<i64>,
    /// Individual frames for scrub previews.
    pub thumbnails: usize,
    /// Source duration.
    pub duration_ms: u64,
}

/// Why an asset could not be published.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// The source could not be read or converted.
    #[error(transparent)]
    Av(#[from] AvError),
    /// The source has no video to convert.
    #[error("no video stream in {0}")]
    NoVideo(String),
    /// The renditions disagree with each other, so nothing is published.
    #[error("checks failed: {0}")]
    Rejected(String),
}

/// Run the pipeline over `input` and leave a playable directory in `out_dir`.
///
/// Publishing is gated on the checks: a dropped chunk produces a file that
/// still plays, so the only way to catch it is to look on purpose.
pub fn run(input: &Path, out_dir: &Path, request: &Request) -> Result<Published, AssetError> {
    let info = ferrite_av::probe(input)?;
    let video = info
        .primary_video()
        .ok_or_else(|| AssetError::NoVideo(input.display().to_string()))?;

    let mut steps = ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, request.preset);
    if request.fast_only {
        steps = ferrite_av::ladder::fast_path(&steps)
            .cloned()
            .into_iter()
            .collect();
    }
    if steps.is_empty() {
        return Err(AssetError::Rejected("the ladder planned no rungs".into()));
    }

    let encoded_dir = out_dir.join("renditions");
    let outputs: Vec<Output> = steps
        .iter()
        .map(|s| Output {
            path: encoded_dir.join(format!("{}.mp4", s.name)),
            spec: s.spec.clone(),
        })
        .collect();
    let reports = ferrite_av::transcode::run(input, &outputs, Arc::new(NeverCancel))?;

    // Never chunked and never per rung: one track, shared.
    let audio_track = encoded_dir.join("audio.m4a");
    let audio = ferrite_av::audio::encode(input, &audio_track, &Default::default())?;

    let probed: Vec<ferrite_av::MediaInfo> = reports
        .iter()
        .map(|r| ferrite_av::probe(&r.path))
        .collect::<Result<_, _>>()?;
    let renditions: Vec<ferrite_av::verify::Rendition<'_>> = steps
        .iter()
        .zip(&probed)
        .map(|(s, i)| ferrite_av::verify::Rendition {
            name: s.name,
            info: i,
        })
        .collect();

    let verdict = ferrite_av::verify::verify(&renditions);
    if !verdict.is_ok() {
        return Err(AssetError::Rejected(
            verdict
                .findings
                .iter()
                .map(|f| format!("{} {:?}", f.rendition, f.problem))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }

    let sheet = ferrite_av::sheet::build_with(
        input,
        &out_dir.join("contactsheet.jpg"),
        &ferrite_av::sheet::Options {
            thumbnails: Some(out_dir.join("thumbs")),
        },
    )?;

    let mut inputs: Vec<Input> = steps
        .iter()
        .zip(&reports)
        .map(|(s, r)| Input {
            name: s.name.to_string(),
            path: r.path.clone(),
            kind: Track::Video,
        })
        .collect();
    if audio.is_some() {
        inputs.push(Input {
            name: "audio".into(),
            path: audio_track,
            kind: Track::Audio,
        });
    }
    let packaged = ferrite_av::package::run(&inputs, &out_dir.join("cmaf"))?;

    Ok(Published {
        hls: packaged.hls,
        dash: packaged.dash,
        renditions: steps
            .iter()
            .zip(&reports)
            .map(|(s, r)| Rendition {
                name: s.name.to_string(),
                width: s.spec.resolution.width,
                height: s.spec.resolution.height,
                frames: r.frames,
                bytes: r.bytes,
            })
            .collect(),
        audio: audio.is_some(),
        contact_sheet: sheet.path,
        hashes: sheet.samples.iter().map(|s| s.phash).collect(),
        thumbnails: sheet.thumbnails.len(),
        duration_ms: info.duration_ms,
    })
}
