//! Software (CPU) encoder built on the `ffmpeg`/`ffprobe` binaries.
//!
//! Implements [`ferrite_core::Encoder`]. A future `NvencEncoder` will implement
//! the same trait and slot into the pipeline unchanged — this type holds no
//! assumptions the GPU path can't also satisfy.

use std::path::{Path, PathBuf};

use ferrite_core::{
    Artifact, ArtifactKind, Encoder, MediaInfo, ProgressSink, TranscodeError, TranscodeJob,
};
use serde_json::Value;
use tokio::process::Command;

/// Encodes with libx264 into HLS renditions. `output_dir` is a per-job scratch
/// directory the worker owns and cleans up.
pub struct CpuEncoder {
    output_dir: PathBuf,
}

impl CpuEncoder {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    /// Write the ffmpeg keyinfo file for encrypted jobs; returns its path.
    /// Line 1 is the URI written into playlists; line 2 is the local key file.
    async fn write_keyinfo(&self, job: &TranscodeJob) -> Result<Option<String>, TranscodeError> {
        let Some(hex_key) = &job.encryption_key else {
            return Ok(None);
        };
        let key = hex::decode(hex_key)
            .map_err(|e| TranscodeError::Unsupported(format!("bad encryption key: {e}")))?;

        let key_path = self.output_dir.join("enc.key.bin");
        tokio::fs::write(&key_path, &key)
            .await
            .map_err(TranscodeError::Io)?;

        let keyinfo_path = self.output_dir.join("enc.keyinfo");
        let content = format!("enc.key\n{}\n", key_path.to_string_lossy());
        tokio::fs::write(&keyinfo_path, content)
            .await
            .map_err(TranscodeError::Io)?;

        Ok(Some(keyinfo_path.to_string_lossy().to_string()))
    }
}

impl Encoder for CpuEncoder {
    async fn probe(&self, source_path: &str) -> Result<MediaInfo, TranscodeError> {
        let output = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(source_path)
            .output()
            .await
            .map_err(|e| TranscodeError::Probe(format!("failed to spawn ffprobe: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TranscodeError::Probe(stderr.trim().to_string()));
        }

        let json: Value = serde_json::from_slice(&output.stdout)
            .map_err(|e| TranscodeError::Probe(format!("invalid ffprobe json: {e}")))?;

        parse_media_info(&json)
    }

    async fn transcode<'a>(
        &'a self,
        job: &'a TranscodeJob,
        _media: &'a MediaInfo,
        on_progress: ProgressSink<'a>,
    ) -> Result<Vec<Artifact>, TranscodeError> {
        tokio::fs::create_dir_all(&self.output_dir)
            .await
            .map_err(TranscodeError::Io)?;

        let source = self.output_dir.join("source.input");
        let source_str = source.to_string_lossy().to_string();

        // For AES-128 HLS: write a keyinfo file so ffmpeg encrypts segments and
        // stamps `#EXT-X-KEY:URI="enc.key"` into each playlist. The key file is
        // read only for encryption — it is never written to storage.
        let keyinfo = self.write_keyinfo(job).await?;

        let mut artifacts = Vec::new();
        let total = job.ladder.renditions.len().max(1);

        for (idx, rendition) in job.ladder.renditions.iter().enumerate() {
            on_progress(&rendition.name, idx as f32 / total as f32);

            let rendition_dir = self.output_dir.join(&rendition.name);
            tokio::fs::create_dir_all(&rendition_dir)
                .await
                .map_err(TranscodeError::Io)?;

            let playlist = rendition_dir.join("index.m3u8");
            let segment_pattern = rendition_dir.join("seg_%04d.ts");

            let mut cmd = Command::new("ffmpeg");
            cmd.args(["-y", "-loglevel", "error", "-nostats", "-i"])
                .arg(&source_str)
                .args([
                    "-vf",
                    &format!("scale=-2:{}", rendition.height),
                    "-c:v",
                    "libx264",
                    "-preset",
                    "veryfast",
                    "-b:v",
                    &format!("{}k", rendition.bitrate_kbps),
                    "-maxrate",
                    &format!("{}k", rendition.max_bitrate_kbps),
                    "-bufsize",
                    &format!("{}k", rendition.bitrate_kbps * 2),
                    "-c:a",
                    "aac",
                    "-b:a",
                    "128k",
                    "-hls_time",
                    "6",
                    "-hls_playlist_type",
                    "vod",
                ]);
            if let Some(ki) = &keyinfo {
                cmd.args(["-hls_key_info_file", ki]);
            }
            let output = cmd
                .args(["-hls_segment_filename"])
                .arg(&segment_pattern)
                .arg(&playlist)
                .output()
                .await
                .map_err(|e| TranscodeError::Ffmpeg(format!("failed to spawn ffmpeg: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(TranscodeError::Ffmpeg(format!(
                    "rendition {} failed: {}",
                    rendition.name,
                    stderr.trim()
                )));
            }

            collect_rendition_artifacts(&rendition_dir, &rendition.name, job, &mut artifacts)
                .await?;
        }

        on_progress("done", 1.0);
        Ok(artifacts)
    }
}

/// Turn a rendition's on-disk output into uploadable [`Artifact`]s.
async fn collect_rendition_artifacts(
    dir: &Path,
    rendition: &str,
    job: &TranscodeJob,
    out: &mut Vec<Artifact>,
) -> Result<(), TranscodeError> {
    let mut entries = tokio::fs::read_dir(dir).await.map_err(TranscodeError::Io)?;
    while let Some(entry) = entries.next_entry().await.map_err(TranscodeError::Io)? {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let kind = if file_name.ends_with(".m3u8") {
            ArtifactKind::HlsPlaylist
        } else if file_name.ends_with(".ts") {
            ArtifactKind::HlsSegment
        } else {
            continue;
        };
        let bytes = entry.metadata().await.map_err(TranscodeError::Io)?.len();

        out.push(Artifact {
            kind,
            local_path: path.to_string_lossy().to_string(),
            key: format!("{}/{}/{}", job.output_prefix, rendition, file_name),
            rendition: Some(rendition.to_string()),
            bytes,
        });
    }
    Ok(())
}

/// Extract the fields we care about from ffprobe's JSON output.
fn parse_media_info(json: &Value) -> Result<MediaInfo, TranscodeError> {
    let streams = json
        .get("streams")
        .and_then(Value::as_array)
        .ok_or_else(|| TranscodeError::Probe("no streams in ffprobe output".into()))?;

    let video = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(Value::as_str) == Some("video"))
        .ok_or_else(|| TranscodeError::Unsupported("no video stream found".into()))?;

    let audio = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"));

    let width = video.get("width").and_then(Value::as_u64).unwrap_or(0) as u32;
    let height = video.get("height").and_then(Value::as_u64).unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err(TranscodeError::Unsupported(
            "video stream has no dimensions".into(),
        ));
    }

    let video_codec = video
        .get("codec_name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();

    let fps = video
        .get("r_frame_rate")
        .and_then(Value::as_str)
        .and_then(parse_fraction)
        .unwrap_or(0.0);

    let format = json.get("format");
    let duration_secs = format
        .and_then(|f| f.get("duration"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let bitrate_kbps = format
        .and_then(|f| f.get("bit_rate"))
        .and_then(Value::as_str)
        .and_then(|s| s.parse::<u32>().ok())
        .map(|b| b / 1000);

    let audio_codec = audio
        .and_then(|a| a.get("codec_name"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    Ok(MediaInfo {
        duration_secs,
        width,
        height,
        fps,
        video_codec,
        has_audio: audio.is_some(),
        audio_codec,
        bitrate_kbps,
    })
}

/// Parse ffprobe fractional rates like "30000/1001" into f64.
fn parse_fraction(s: &str) -> Option<f64> {
    let (num, den) = s.split_once('/')?;
    let num: f64 = num.parse().ok()?;
    let den: f64 = den.parse().ok()?;
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}
