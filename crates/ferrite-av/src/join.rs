//! Join encoded chunks back into one rendition (pipeline step 7).
//!
//! Concatenates compressed data and fixes timestamps. Nothing is re-encoded:
//! decoding and re-encoding a joined chunk would throw away the quality the
//! chunked path exists to preserve.

use crate::error::{AvError, Result};
use ffmpeg_next as ff;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What the join produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Joined {
    /// The rendition.
    pub path: PathBuf,
    /// Chunks concatenated.
    pub parts: usize,
    /// Frames across all of them.
    pub frames: u64,
    /// Compressed bytes.
    pub bytes: u64,
}

/// Concatenate `parts` in order into `output`.
///
/// Every part must come from the same [`crate::EncodeSpec`]: the output carries
/// the first part's codec configuration, and a different one would decode to
/// noise partway through.
pub fn run(parts: &[PathBuf], output: &Path) -> Result<Joined> {
    let Some((first, rest)) = parts.split_first() else {
        return Err(AvError::InvalidSpec("nothing to join".into()));
    };
    crate::init()?;

    let source = open(first)?;
    let stream = source
        .streams()
        .best(ff::media::Type::Video)
        .ok_or(AvError::NoStream { kind: "video" })?;
    let parameters = stream.parameters();
    let time_base = stream.time_base();
    let frame_rate = stream.avg_frame_rate();
    let stream_index = stream.index();

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut muxer = ff::format::output(&output).map_err(wrap("open joined output"))?;
    let mut out_stream = muxer
        .add_stream(ff::encoder::find(parameters.id()).ok_or_else(|| {
            AvError::CodecUnavailable {
                codec: parameters.id().name().to_string(),
                reason: "no encoder to describe the stream".into(),
            }
        })?)
        .map_err(wrap("add joined stream"))?;
    out_stream.set_time_base(time_base);
    out_stream.set_avg_frame_rate(frame_rate);
    out_stream.set_parameters(parameters);
    muxer.write_header().map_err(wrap("write joined header"))?;

    let out_time_base = muxer
        .streams()
        .next()
        .ok_or_else(|| AvError::InvalidSpec("joined muxer lost its stream".into()))?
        .time_base();

    let mut offset = 0i64;
    let mut frames = 0u64;
    let mut bytes = 0u64;

    // The first part is already open; the rest are opened in turn.
    let mut current = Some((source, stream_index, time_base));
    let mut remaining = rest.iter();

    loop {
        let Some((mut part, index, part_time_base)) = current.take() else {
            break;
        };
        let mut last_end = 0i64;

        for (packet_stream, mut packet) in part.packets() {
            if packet_stream.index() != index {
                continue;
            }

            // Each chunk was encoded from zero, so its timestamps restart. The
            // running offset is what makes them one continuous timeline.
            let pts = packet.pts().unwrap_or(0) + offset;
            let dts = packet.dts().unwrap_or(0) + offset;
            packet.set_pts(Some(pts));
            packet.set_dts(Some(dts));
            packet.set_stream(0);
            packet.set_position(-1);

            last_end = last_end.max(pts + packet.duration().max(1));
            bytes += packet.size() as u64;
            frames += 1;

            packet.rescale_ts(part_time_base, out_time_base);
            packet
                .write_interleaved(&mut muxer)
                .map_err(wrap("mux joined packet"))?;
        }

        offset = last_end;
        if let Some(next) = remaining.next() {
            let opened = open(next)?;
            let index = opened
                .streams()
                .best(ff::media::Type::Video)
                .ok_or(AvError::NoStream { kind: "video" })?
                .index();
            let tb = opened
                .streams()
                .best(ff::media::Type::Video)
                .map(|s| s.time_base())
                .unwrap_or(time_base);
            current = Some((opened, index, tb));
        }
    }

    muxer
        .write_trailer()
        .map_err(wrap("write joined trailer"))?;
    Ok(Joined {
        path: output.to_path_buf(),
        parts: parts.len(),
        frames,
        bytes,
    })
}

fn open(path: &Path) -> Result<ff::format::context::Input> {
    ff::format::input(path).map_err(|e| AvError::OpenInput {
        path: path.display().to_string(),
        source: Box::new(e),
    })
}

fn wrap(op: &'static str) -> impl Fn(ff::Error) -> AvError {
    move |e| AvError::Ffmpeg {
        op,
        source: Box::new(e),
    }
}
