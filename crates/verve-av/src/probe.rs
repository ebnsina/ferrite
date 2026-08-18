//! Pipeline step 2: read a file and say what is wrong with it.
//! Nothing FFmpeg-shaped escapes this module.

use crate::error::{AvError, Result};
use crate::media::{AudioStream, ColorInfo, MediaInfo, Rational, VideoStream, Warning};
use ffmpeg_next as ff;
use std::collections::BTreeSet;
use std::path::Path;

/// Keyframes further apart than this make ~10s chunking unreliable.
const SPARSE_KEYFRAME_GAP_MS: u64 = 15_000;

/// Probe `path`. Demuxes the whole file for the keyframe index but decodes
/// nothing, so the cost is I/O, not CPU.
pub fn probe(path: impl AsRef<Path>) -> Result<MediaInfo> {
    let path = path.as_ref();
    crate::init()?;

    let mut input = ff::format::input(path).map_err(|e| AvError::OpenInput {
        path: path.display().to_string(),
        source: Box::new(e),
    })?;

    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let mut warnings: BTreeSet<Warning> = BTreeSet::new();

    let duration_ms = if input.duration() > 0 {
        // AV_TIME_BASE units are microseconds.
        (input.duration() as u64) / 1_000
    } else {
        warnings.insert(Warning::UnknownDuration);
        0
    };

    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut video_index = None;

    for stream in input.streams() {
        let params = stream.parameters();
        match params.medium() {
            ff::media::Type::Video => {
                if video_index.is_none() {
                    video_index = Some(stream.index());
                }
                video.push(read_video(&stream, &mut warnings)?);
            }
            ff::media::Type::Audio => audio.push(read_audio(&stream)?),
            _ => {}
        }
    }

    let Some(video_index) = video_index else {
        return Err(AvError::NoStream { kind: "video" });
    };
    if video.len() > 1 {
        warnings.insert(Warning::MultipleVideoStreams);
    }
    if audio.is_empty() {
        warnings.insert(Warning::NoAudio);
    }

    let time_base = input
        .streams()
        .find(|s| s.index() == video_index)
        .map(|s| s.time_base())
        .unwrap_or(ff::Rational(1, 1000));

    let keyframes_ms = scan_keyframes(&mut input, video_index, time_base, &mut warnings);

    if keyframes_ms.is_empty() {
        warnings.insert(Warning::NoKeyframeIndex);
    } else if keyframes_ms
        .windows(2)
        .any(|w| w[1].saturating_sub(w[0]) > SPARSE_KEYFRAME_GAP_MS)
    {
        warnings.insert(Warning::SparseKeyframes);
    }

    Ok(MediaInfo {
        format: input.format().name().to_string(),
        duration_ms,
        size_bytes,
        bit_rate: (input.bit_rate() > 0).then(|| input.bit_rate() as u64),
        video,
        audio,
        keyframes_ms,
        warnings: warnings.into_iter().collect(),
    })
}

fn read_video(
    stream: &ff::format::stream::Stream<'_>,
    warnings: &mut BTreeSet<Warning>,
) -> Result<VideoStream> {
    let params = stream.parameters();
    let codec_id = params.id();
    let decoder = ff::codec::context::Context::from_parameters(params.clone())
        .map_err(|e| AvError::Ffmpeg {
            op: "read video parameters",
            source: Box::new(e),
        })?
        .decoder()
        .video()
        .map_err(|e| AvError::CodecUnavailable {
            codec: codec_id.name().to_string(),
            reason: e.to_string(),
        })?;

    let avg = stream.avg_frame_rate();
    let real = stream.rate();
    if avg.numerator() != real.numerator() || avg.denominator() != real.denominator() {
        // r_frame_rate fits every frame duration; a mismatch means VFR.
        warnings.insert(Warning::VariableFrameRate);
    }
    if stream.start_time() > 0 {
        warnings.insert(Warning::StartTimeOffset);
    }

    let rotation = rotation_degrees(stream);
    if rotation != 0 {
        warnings.insert(Warning::RotationMetadata);
    }

    let aspect = decoder.aspect_ratio();
    if aspect.numerator() != 0 && aspect.numerator() != aspect.denominator() {
        warnings.insert(Warning::NonSquarePixels);
    }

    let color = ColorInfo {
        primaries: decoder.color_primaries().name().map(str::to_string),
        transfer: decoder
            .color_transfer_characteristic()
            .name()
            .map(str::to_string),
        matrix: decoder.color_space().name().map(str::to_string),
        range: decoder.color_range().name().map(str::to_string),
    };
    if color.is_hdr() {
        warnings.insert(Warning::HdrTransfer);
    }

    Ok(VideoStream {
        index: stream.index(),
        codec: codec_id.name().to_string(),
        width: decoder.width(),
        height: decoder.height(),
        frame_rate: Rational::new(avg.numerator(), avg.denominator()),
        pixel_format: decoder
            .format()
            .descriptor()
            .map_or_else(|| "unknown".to_string(), |d| d.name().to_string()),
        rotation_degrees: rotation,
        color,
        bit_rate: (decoder.bit_rate() > 0).then(|| decoder.bit_rate() as u64),
        frame_count: (stream.frames() > 0).then(|| stream.frames() as u64),
    })
}

fn read_audio(stream: &ff::format::stream::Stream<'_>) -> Result<AudioStream> {
    let params = stream.parameters();
    let codec_id = params.id();
    let decoder = ff::codec::context::Context::from_parameters(params.clone())
        .map_err(|e| AvError::Ffmpeg {
            op: "read audio parameters",
            source: Box::new(e),
        })?
        .decoder()
        .audio()
        .map_err(|e| AvError::CodecUnavailable {
            codec: codec_id.name().to_string(),
            reason: e.to_string(),
        })?;

    Ok(AudioStream {
        index: stream.index(),
        codec: codec_id.name().to_string(),
        sample_rate: decoder.rate(),
        channels: decoder.channels(),
        channel_layout: format!("{:?}", decoder.channel_layout()),
        bit_rate: (decoder.bit_rate() > 0).then(|| decoder.bit_rate() as u64),
        language: stream.metadata().get("language").map(str::to_string),
    })
}

/// Display matrix to whole degrees. The one probe path needing raw side data.
fn rotation_degrees(stream: &ff::format::stream::Stream<'_>) -> i32 {
    for side in stream.side_data() {
        if side.kind() != ff::codec::packet::side_data::Type::DisplayMatrix {
            continue;
        }
        let data = side.data();
        if data.len() < 36 {
            continue;
        }
        // SAFETY: DISPLAYMATRIX is nine i32s; length checked, and the pointer
        // goes straight back to FFmpeg's own accessor.
        let degrees = unsafe { ff::ffi::av_display_rotation_get(data.as_ptr().cast::<i32>()) };
        if degrees.is_nan() {
            return 0;
        }
        // FFmpeg returns the angle to rotate back.
        return (-degrees).round() as i32 % 360;
    }
    0
}

/// Record every keyframe's presentation time.
fn scan_keyframes(
    input: &mut ff::format::context::Input,
    video_index: usize,
    time_base: ff::Rational,
    warnings: &mut BTreeSet<Warning>,
) -> Vec<u64> {
    let tb = f64::from(time_base.numerator()) / f64::from(time_base.denominator());
    let mut keyframes = Vec::new();
    let mut last_pts = i64::MIN;
    let mut non_monotonic = false;

    for (stream, packet) in input.packets() {
        if stream.index() != video_index {
            continue;
        }
        let Some(pts) = packet.pts() else { continue };
        if pts < last_pts {
            non_monotonic = true;
        }
        last_pts = pts;
        if packet.is_key() {
            keyframes.push((pts.max(0) as f64 * tb * 1000.0).round() as u64);
        }
    }

    if non_monotonic {
        warnings.insert(Warning::NonMonotonicTimestamps);
    }
    keyframes.sort_unstable();
    keyframes.dedup();
    keyframes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_names_the_path() {
        let err = probe("/nonexistent/verve/nope.mp4").unwrap_err();
        assert!(matches!(err, AvError::OpenInput { .. }));
        assert!(err.to_string().contains("nope.mp4"), "{err}");
    }
}
