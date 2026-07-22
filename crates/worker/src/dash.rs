//! MPEG-DASH packaging: one ffmpeg pass emits a manifest.mpd + fMP4 segments
//! for the whole ladder, under `{output_prefix}/dash/`.
//!
//! Note: this re-encodes for DASH. A future optimization is CMAF (fMP4 shared
//! by both HLS and DASH from a single encode).

use std::path::Path;

use ferrite_core::{Artifact, ArtifactKind, MediaInfo, TranscodeError, TranscodeJob};
use tokio::process::Command;

/// Generate a DASH package into `dir`, returning uploadable artifacts.
pub async fn generate(
    job: &TranscodeJob,
    media: &MediaInfo,
    source: &str,
    dir: &Path,
) -> Result<Vec<Artifact>, TranscodeError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(TranscodeError::Io)?;
    let renditions = &job.ladder.renditions;
    if renditions.is_empty() {
        return Ok(Vec::new());
    }

    let manifest = dir.join("manifest.mpd");
    let args = build_args(job, media, source, &manifest.to_string_lossy());

    let output = Command::new("ffmpeg")
        .args(&args)
        .output()
        .await
        .map_err(|e| TranscodeError::Ffmpeg(format!("failed to spawn ffmpeg: {e}")))?;
    if !output.status.success() {
        return Err(TranscodeError::Ffmpeg(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    collect_artifacts(job, dir).await
}

/// Build the multi-bitrate DASH ffmpeg command for the ladder.
fn build_args(job: &TranscodeJob, media: &MediaInfo, source: &str, manifest: &str) -> Vec<String> {
    let renditions = &job.ladder.renditions;
    let mut a: Vec<String> = vec!["-y", "-loglevel", "error", "-nostats", "-i", source]
        .into_iter()
        .map(String::from)
        .collect();

    // One video output per rendition (+ audio if present).
    for _ in renditions {
        a.push("-map".into());
        a.push("0:v:0".into());
    }
    if media.has_audio {
        a.push("-map".into());
        a.push("0:a:0".into());
    }

    a.extend(["-c:v", "libx264", "-preset", "veryfast"].map(String::from));
    if media.has_audio {
        a.extend(["-c:a", "aac", "-b:a", "128k"].map(String::from));
    }

    for (i, r) in renditions.iter().enumerate() {
        a.push(format!("-b:v:{i}"));
        a.push(format!("{}k", r.bitrate_kbps));
        a.push(format!("-filter:v:{i}"));
        a.push(format!("scale=-2:{}", r.height));
    }

    a.extend(
        [
            "-use_timeline",
            "1",
            "-use_template",
            "1",
            "-seg_duration",
            "6",
        ]
        .map(String::from),
    );
    a.push("-adaptation_sets".into());
    a.push(
        if media.has_audio {
            "id=0,streams=v id=1,streams=a"
        } else {
            "id=0,streams=v"
        }
        .into(),
    );
    a.extend(["-f", "dash"].map(String::from));
    a.push(manifest.to_string());
    a
}

async fn collect_artifacts(
    job: &TranscodeJob,
    dir: &Path,
) -> Result<Vec<Artifact>, TranscodeError> {
    let mut out = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await.map_err(TranscodeError::Io)?;
    while let Some(entry) = entries.next_entry().await.map_err(TranscodeError::Io)? {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let kind = if name.ends_with(".mpd") {
            ArtifactKind::DashManifest
        } else {
            ArtifactKind::DashSegment
        };
        let bytes = entry.metadata().await.map_err(TranscodeError::Io)?.len();
        out.push(Artifact {
            kind,
            local_path: path.to_string_lossy().to_string(),
            key: format!("{}/dash/{}", job.output_prefix, name),
            rendition: None,
            bytes,
        });
    }
    Ok(out)
}
