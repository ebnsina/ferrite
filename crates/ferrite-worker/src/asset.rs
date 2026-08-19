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
    /// Split the source and encode the pieces separately. Time then depends on
    /// how many machines are free rather than on how long the video is.
    pub chunked: bool,
    /// Chunk length. The industry norm is ten seconds.
    pub chunk_ms: u64,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            preset: Preset::Medium,
            fast_only: false,
            chunked: true,
            chunk_ms: ferrite_av::split::TARGET_CHUNK_MS,
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
    /// Chunks the source was split into. One means it was encoded whole.
    pub chunks: usize,
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

/// Plan an asset without encoding it, for handing to the fleet.
///
/// The plan is computed once here and travels with the job: a worker that
/// resplit could cut somewhere else, and a retried chunk must reproduce its
/// boundaries exactly.
pub fn plan(
    input: &Path,
    out_dir: &Path,
    request: &Request,
) -> Result<crate::work::AssetJob, AssetError> {
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

    Ok(crate::work::AssetJob {
        input: input.to_path_buf(),
        out_dir: out_dir.to_path_buf(),
        plan: ferrite_av::split::plan(&info.keyframes_ms, info.duration_ms, request.chunk_ms),
        rungs: steps
            .iter()
            .map(|s| crate::work::Rung {
                name: s.name.to_string(),
                spec: s.spec.clone(),
            })
            .collect(),
    })
}

/// Package whatever renditions have landed, plus audio if there is any.
///
/// Called every time a rung finishes, so the manifest grows rather than
/// appearing all at once. Nothing here re-encodes; the segments already exist.
pub fn publish(job: &crate::work::AssetJob) -> Result<ferrite_av::package::Packaged, AssetError> {
    let ready = job.ready_rungs();
    if ready.is_empty() {
        return Err(AssetError::Rejected(
            "nothing has finished encoding yet".into(),
        ));
    }

    let mut inputs: Vec<Input> = ready
        .iter()
        .map(|rung| Input {
            name: rung.name.clone(),
            path: job.rendition(rung),
            kind: Track::Video,
            language: None,
        })
        .collect();

    let audio = job.out_dir.join("renditions").join("audio.m4a");
    if audio.is_file() {
        inputs.push(Input {
            name: "audio".into(),
            path: audio,
            kind: Track::Audio,
            language: None,
        });
    }

    Ok(ferrite_av::package::run(
        &inputs,
        &job.out_dir.join("cmaf"),
    )?)
}

/// Encode the one audio track, shared by every rung and never chunked.
pub fn encode_audio(job: &crate::work::AssetJob) -> Result<bool, AssetError> {
    let output = job.out_dir.join("renditions").join("audio.m4a");
    Ok(ferrite_av::audio::encode(&job.input, &output, &Default::default())?.is_some())
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
    let plan = ferrite_av::split::plan(&info.keyframes_ms, info.duration_ms, request.chunk_ms);
    let split = request.chunked && plan.is_split();
    let (reports, chunks) = if split {
        (
            encode_in_chunks(input, &outputs, &plan, &encoded_dir)?,
            plan.chunks.len(),
        )
    } else {
        (
            ferrite_av::transcode::run(input, &outputs, Arc::new(NeverCancel))?,
            1,
        )
    };

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
            language: None,
        })
        .collect();
    if audio.is_some() {
        inputs.push(Input {
            name: "audio".into(),
            path: audio_track,
            kind: Track::Audio,
            language: None,
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
        chunks,
    })
}

/// Encode each chunk across every rung, then join the pieces per rung.
///
/// One decode per chunk still feeds all rungs, so splitting the work does not
/// undo decode-once.
fn encode_in_chunks(
    input: &Path,
    outputs: &[Output],
    plan: &ferrite_av::split::SplitPlan,
    work_dir: &Path,
) -> Result<Vec<ferrite_av::transcode::Report>, AssetError> {
    let parts_dir = work_dir.join("parts");
    let mut per_rung: Vec<Vec<PathBuf>> = vec![Vec::new(); outputs.len()];

    for chunk in &plan.chunks {
        let chunk_outputs: Vec<Output> = outputs
            .iter()
            .enumerate()
            .map(|(rung, out)| {
                let name = out.path.file_stem().unwrap_or_default().to_string_lossy();
                let path = parts_dir.join(format!("{name}-{:05}.mp4", chunk.index));
                per_rung[rung].push(path.clone());
                Output {
                    path,
                    spec: out.spec.clone(),
                }
            })
            .collect();

        ferrite_av::transcode::run_range(
            input,
            &chunk_outputs,
            Some(*chunk),
            Arc::new(NeverCancel),
        )?;
    }

    let mut reports = Vec::with_capacity(outputs.len());
    for (out, parts) in outputs.iter().zip(&per_rung) {
        let joined = ferrite_av::join::run(parts, &out.path)?;
        reports.push(ferrite_av::transcode::Report {
            path: joined.path,
            frames: joined.frames,
            bytes: joined.bytes,
            provenance: ferrite_av::Provenance {
                backend: ferrite_av::BackendId::Cpu,
                encoder: ferrite_av::encoder::CpuBackend::encoder_name(out.spec.codec).to_string(),
                encoder_version: None,
                ffmpeg_version: ferrite_av::ffmpeg_version(),
            },
        });
    }

    // The pieces only matter while a straggler might still be re-issued.
    let _ = std::fs::remove_dir_all(&parts_dir);
    Ok(reports)
}
