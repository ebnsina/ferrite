//! CMAF packaging: one ffmpeg pass emits fMP4 (CMAF) segments shared by both an
//! HLS master/media playlists (`-hls_playlist`) and a DASH manifest. Single
//! encode for both formats — no double-encode.
//!
//! Output (flat under the job prefix): master.m3u8, manifest.mpd, media_N.m3u8,
//! init-streamN.m4s, chunk-streamN-*.m4s. Used for unencrypted jobs; encrypted
//! jobs keep the TS + AES-128 path.

use std::path::Path;
use std::process::Stdio;

use ferrite_core::{Artifact, ArtifactKind, MediaInfo, TranscodeError, TranscodeJob};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::encoding::EncodeParams;

/// Produce the CMAF package (HLS + DASH) into the job's output dir.
/// `logo` is a local path to the watermark image, overlaid on every rendition.
/// `on_progress` is called with 0.0–0.99 as ffmpeg advances (real, not stepped).
pub async fn generate(
    job: &TranscodeJob,
    media: &MediaInfo,
    source: &str,
    dir: &Path,
    encode: EncodeParams,
    logo: Option<&str>,
    total_secs: f64,
    on_progress: &(dyn Fn(f32) + Send + Sync),
) -> Result<Vec<Artifact>, TranscodeError> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(TranscodeError::Io)?;
    if job.ladder.renditions.is_empty() {
        return Ok(Vec::new());
    }

    let manifest = dir.join("manifest.mpd");
    let args = build_args(
        job,
        media,
        source,
        &manifest.to_string_lossy(),
        encode,
        logo,
    );

    // Spawn (not .output()) so we can stream `-progress` and report real
    // percentage. stderr is drained concurrently for the error message.
    let mut child = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| TranscodeError::Ffmpeg(format!("failed to spawn ffmpeg: {e}")))?;

    let stderr = child.stderr.take().expect("piped stderr");
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut s).await;
        s
    });

    if let Some(stdout) = child.stdout.take() {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // ffmpeg -progress emits key=value; out_time_us is microseconds
            // (older builds mislabel it out_time_ms — same units).
            let us = line
                .strip_prefix("out_time_us=")
                .or_else(|| line.strip_prefix("out_time_ms="));
            if let Some(v) = us {
                if let Ok(n) = v.trim().parse::<i64>() {
                    if n >= 0 && total_secs > 0.0 {
                        let pct = (((n as f64) / 1_000_000.0) / total_secs).clamp(0.0, 0.99);
                        on_progress(pct as f32);
                    }
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| TranscodeError::Ffmpeg(format!("ffmpeg wait failed: {e}")))?;
    let stderr_str = err_task.await.unwrap_or_default();
    if !status.success() {
        return Err(TranscodeError::Ffmpeg(stderr_str.trim().to_string()));
    }

    collect(job, dir).await
}

fn build_args(
    job: &TranscodeJob,
    media: &MediaInfo,
    source: &str,
    manifest: &str,
    encode: EncodeParams,
    logo: Option<&str>,
) -> Vec<String> {
    let renditions = &job.ladder.renditions;
    let mut a: Vec<String> = ["-y", "-loglevel", "error", "-nostats", "-i", source]
        .into_iter()
        .map(String::from)
        .collect();

    // Watermark: overlay the logo once, then split+scale per rendition via a
    // filter_complex (per-output simple filters can't reference a 2nd input).
    let watermark = job.watermark.as_ref().zip(logo);
    if let Some((_, logo_path)) = watermark {
        a.push("-i".into());
        a.push(logo_path.to_string());
    }

    if let Some((wm, _)) = watermark {
        let n = renditions.len();
        a.push("-filter_complex".into());
        a.push(watermark_graph(wm, renditions, media.width, media.height));
        for i in 0..n {
            a.push("-map".into());
            a.push(format!("[o{i}]"));
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
        }
    } else {
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
            // Pin SAR=1 and force the source's display aspect ratio on every
            // rendition. Without this, rounding each height to an even width
            // yields slightly different ARs, and the DASH muxer refuses to write
            // ("Conflicting stream aspect ratios in Adaptation Set").
            a.push(format!(
                "scale=-2:{},setsar=1,setdar={}/{}",
                r.height, media.width, media.height
            ));
        }
    }

    // Stream machine-readable progress to stdout so the worker can report %.
    a.extend(["-progress", "pipe:1"].map(String::from));
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

/// filter_complex that overlays the logo on the source, then splits and scales
/// it into one output per rendition: `[o0]`, `[o1]`, …
fn watermark_graph(
    wm: &ferrite_core::Watermark,
    renditions: &[ferrite_core::Rendition],
    src_w: u32,
    src_h: u32,
) -> String {
    let op = wm.opacity.clamp(0.0, 1.0);
    let m = 24;
    let pos = match wm.position.as_str() {
        "tl" => format!("{m}:{m}"),
        "tr" => format!("W-w-{m}:{m}"),
        "bl" => format!("{m}:H-h-{m}"),
        _ => format!("W-w-{m}:H-h-{m}"),
    };
    let n = renditions.len();
    let mut g = format!(
        "[1:v]format=rgba,colorchannelmixer=aa={op}[wm];[0:v][wm]overlay={pos}[bg];[bg]split={n}"
    );
    for i in 0..n {
        g.push_str(&format!("[s{i}]"));
    }
    g.push(';');
    for (i, r) in renditions.iter().enumerate() {
        g.push_str(&format!(
            "[s{i}]scale=-2:{},setsar=1,setdar={src_w}/{src_h}[o{i}]",
            r.height
        ));
        if i + 1 < n {
            g.push(';');
        }
    }
    g
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
