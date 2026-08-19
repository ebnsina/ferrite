//! Job mode: one file in, one file out.
//!
//! Shares steps 1–3 with asset mode and then diverges — no ladder, no manifest,
//! no chunk fan-out. Building it on the same `Source` path is the point: it is
//! how we know that path is genuinely shared rather than asset plumbing renamed.

use ferrite_av::encoder::{EncodeSpec, Preset, RateControl, Resolution, VideoCodecName};
use ferrite_av::transcode::Output;
use ferrite_av::{AvError, MediaInfo, NeverCancel};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What the customer asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Output codec.
    pub codec: VideoCodecName,
    /// Target height. Width follows the source's shape; `None` keeps the size.
    pub height: Option<u32>,
    /// Speed against efficiency, from the plan.
    pub preset: Preset,
    /// Quality target.
    pub crf: u8,
    /// Bitrate ceiling in bits per second.
    pub max_bitrate: u32,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            codec: VideoCodecName::H264,
            height: None,
            preset: Preset::Medium,
            crf: 23,
            max_bitrate: 5_000_000,
        }
    }
}

/// What came out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Completed {
    /// The output file, for a signed download link.
    pub path: PathBuf,
    /// Output size.
    pub width: u32,
    /// Output size.
    pub height: u32,
    /// Frames encoded.
    pub frames: u64,
    /// Compressed bytes.
    pub bytes: u64,
    /// Whether a separate normalising pass was needed.
    pub mezzanine: bool,
    /// Perceptual hashes of the sampled frames, for the blocklist.
    pub hashes: Vec<i64>,
    /// The contact sheet a reviewer opens.
    pub contact_sheet: PathBuf,
}

/// Failures a customer or an operator can act on.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// The source could not be read or converted.
    #[error(transparent)]
    Av(#[from] AvError),
    /// The source has no video to convert.
    #[error("no video stream in {0}")]
    NoVideo(String),
    /// The output does not match what was asked for.
    #[error("output failed its checks: {0}")]
    Rejected(String),
}

/// Convert `input` into a single `output`.
///
/// Runs the sampling pass even though there is no mezzanine, because the
/// alternative is job mode publishing files nobody has looked at.
pub fn run(input: &Path, output: &Path, request: &Request) -> Result<Completed, JobError> {
    let info = ferrite_av::probe(input)?;
    let video = info
        .primary_video()
        .ok_or_else(|| JobError::NoVideo(input.display().to_string()))?;

    let spec = spec_for(&info, request)?;
    // One output, so the encode's own filter chain does the normalising a
    // mezzanine would have done for a ladder. Recorded either way.
    let mezzanine = info.needs_mezzanine(1);

    let reports = ferrite_av::transcode::run(
        input,
        &[Output {
            path: output.to_path_buf(),
            spec: spec.clone(),
        }],
        Arc::new(NeverCancel),
    )?;
    let report = reports
        .into_iter()
        .next()
        .ok_or_else(|| JobError::Rejected("nothing was encoded".into()))?;

    let sheet_path = output.with_extension("contactsheet.jpg");
    let sheet = ferrite_av::sheet::build(input, &sheet_path)?;

    check(output, &spec, video.frame_rate)?;

    Ok(Completed {
        path: report.path,
        width: spec.resolution.width,
        height: spec.resolution.height,
        frames: report.frames,
        bytes: report.bytes,
        mezzanine,
        hashes: sheet.samples.iter().map(|s| s.phash).collect(),
        contact_sheet: sheet.path,
    })
}

/// Turn a request into an encode spec against this source.
fn spec_for(info: &MediaInfo, request: &Request) -> Result<EncodeSpec, JobError> {
    let video = info
        .primary_video()
        .ok_or_else(|| JobError::NoVideo("source".into()))?;
    let (src_w, src_h) = video.display_size();

    // Never upscale: a customer asking for 1080p from a 480p file gets 480p,
    // not a bigger file carrying the same detail.
    let height = request.height.unwrap_or(src_h).min(src_h) & !1;
    let width = if src_h == 0 {
        src_w
    } else {
        (((u64::from(height) * u64::from(src_w) + u64::from(src_h) / 2) / u64::from(src_h)) as u32
            + 1)
            & !1
    };

    Ok(EncodeSpec::new(
        request.codec,
        Resolution::new(width.max(2), height.max(2)),
        video.frame_rate,
    )
    .with_preset(request.preset)
    .with_rate_control(RateControl::crf(request.crf, request.max_bitrate)))
}

/// Read the output back and confirm it is what was asked for.
fn check(
    output: &Path,
    spec: &EncodeSpec,
    source_rate: ferrite_av::Rational,
) -> Result<(), JobError> {
    let produced = ferrite_av::probe(output)?;
    let video = produced
        .primary_video()
        .ok_or_else(|| JobError::Rejected("the output has no video stream".into()))?;

    if (video.width, video.height) != (spec.resolution.width, spec.resolution.height) {
        return Err(JobError::Rejected(format!(
            "asked for {} and got {}x{}",
            spec.resolution, video.width, video.height
        )));
    }
    if video.frame_rate != source_rate {
        return Err(JobError::Rejected(format!(
            "frame rate changed from {source_rate} to {}",
            video.frame_rate
        )));
    }
    Ok(())
}
