//! CMAF packaging: one ffmpeg pass emits fMP4 (CMAF) segments shared by both an
//! HLS master/media playlists (`-hls_playlist`) and a DASH manifest. Single
//! encode for both formats — no double-encode.
//!
//! Output (flat under the job prefix): master.m3u8, manifest.mpd, media_N.m3u8,
//! init-streamN.m4s, chunk-streamN-*.m4s. Used for unencrypted jobs; encrypted
//! jobs keep the TS + AES-128 path.

use std::path::Path;

use ferrite_core::{Artifact, ArtifactKind, MediaInfo, TranscodeError, TranscodeJob};
use tokio::process::Command;

use crate::encoding::EncodeParams;

/// Produce the CMAF package (HLS + DASH) into the job's output dir.
pub async fn generate(
    job: &TranscodeJob,
    media: &MediaInfo,
    source: &str,
    dir: &Path,
    encode: EncodeParams,
) -> Result<Vec<Artifact>, TranscodeError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(TranscodeError::Io)?;
    if job.ladder.renditions.is_empty() {
        return Ok(Vec::new());
    }

    let manifest = dir.join("manifest.mpd");
    let args = build_args(job, media, source, &manifest.to_string_lossy(), encode);

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

    collect(job, dir).await
}

fn build_args(
    job: &TranscodeJob,
    media: &MediaInfo,
    source: &str,
    manifest: &str,
    encode: EncodeParams,
) -> Vec<String> {
    let renditions = &job.ladder.renditions;
    let mut a: Vec<String> = ["-y", "-loglevel", "error", "-nostats", "-i", source]
        .into_iter()
        .map(String::from)
        .collect();

    for _ in renditions {
        a.push("-map".into());
        a.push("0:v:0".into());
    }
    if media.has_audio {
        a.push("-map".into());
        a.push("0:a:0".into());
    }

    a.extend(["-c:v", encode.codec, "-preset", encode.preset].map(String::from));
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
    // Emit HLS (fMP4) playlists alongside the DASH manifest from the same segments.
    a.extend(["-hls_playlist", "1", "-f", "dash"].map(String::from));
    a.push(manifest.to_string());
    a
}

async fn collect(job: &TranscodeJob, dir: &Path) -> Result<Vec<Artifact>, TranscodeError> {
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
        } else if name.ends_with(".m3u8") {
            ArtifactKind::HlsPlaylist
        } else {
            ArtifactKind::DashSegment // fMP4 (.m4s) shared by HLS + DASH
        };
        let bytes = entry.metadata().await.map_err(TranscodeError::Io)?.len();
        out.push(Artifact {
            kind,
            local_path: path.to_string_lossy().to_string(),
            key: format!("{}/{}", job.output_prefix, name),
            rendition: None,
            bytes,
        });
    }
    Ok(out)
}
