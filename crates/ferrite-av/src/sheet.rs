//! Contact sheet and per-frame perceptual hashes (part of pipeline step 3).
//!
//! What a human reviewer opens: a two-hour film cleared in three seconds. The
//! video itself is never routed into review.
//!
//! Seeks rather than decoding everything — one to two seconds for a
//! feature-length file, so it runs even when the mezzanine is skipped.

use crate::error::{AvError, Result};
use crate::phash;
use ffmpeg_next as ff;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Frames sampled across the source.
pub const FRAMES: u32 = 60;

/// Grid the sheet is laid out in. 10 × 6 = [`FRAMES`].
pub const GRID: (u32, u32) = (10, 6);

/// Each cell's size in the sheet.
pub const CELL: (u32, u32) = (320, 180);

/// One sampled frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// Position in the sheet, reading order.
    pub index: u32,
    /// Where it was taken from.
    pub time_ms: u64,
    /// 64-bit dHash, stored as a signed `BIGINT`.
    pub phash: i64,
}

/// What else to write from the same decode.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// Write each sampled frame as its own JPEG here, for scrub previews.
    pub thumbnails: Option<PathBuf>,
}

/// The sheet and what was hashed to build it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    /// The JPEG. Stays hot: a re-review should not wait on a cold restore.
    pub path: PathBuf,
    /// One per sampled frame.
    pub samples: Vec<Sample>,
    /// Individual frames, when they were asked for.
    #[serde(default)]
    pub thumbnails: Vec<PathBuf>,
}

impl Sheet {
    /// Every hash, for matching against a blocklist.
    pub fn hashes(&self) -> Vec<u64> {
        self.samples.iter().map(|s| s.phash as u64).collect()
    }

    /// The closest blocklist hit across every sampled frame.
    ///
    /// One frame matching is enough: sampling by time does not land on the same
    /// pictures as the original encode, so most frames will not line up even
    /// for a genuine re-upload.
    pub fn blocklist_hit(&self, blocklist: &[u64]) -> Option<(Sample, u64, u32)> {
        self.samples
            .iter()
            .filter_map(|s| {
                phash::find_match(s.phash as u64, blocklist)
                    .map(|(known, distance)| (*s, known, distance))
            })
            .min_by_key(|&(_, _, distance)| distance)
    }
}

/// Sample `input`, write the sheet to `out`, and hash every frame taken.
pub fn build(input: &Path, out: &Path) -> Result<Sheet> {
    build_with(input, out, &Options::default())
}

/// [`build`], plus whatever else `options` asks for from the same decode.
///
/// Thumbnails are free here and a second pass later is not: the frames are
/// already scaled and in memory.
pub fn build_with(input: &Path, out: &Path, options: &Options) -> Result<Sheet> {
    crate::init()?;

    let mut source = ff::format::input(input).map_err(|e| AvError::OpenInput {
        path: input.display().to_string(),
        source: Box::new(e),
    })?;

    let stream = source
        .streams()
        .best(ff::media::Type::Video)
        .ok_or(AvError::NoStream { kind: "video" })?;
    let stream_index = stream.index();
    let time_base = stream.time_base();

    let mut decoder = ff::codec::context::Context::from_parameters(stream.parameters())
        .map_err(wrap("read parameters"))?
        .decoder()
        .video()
        .map_err(wrap("open decoder"))?;

    let duration_ms = if source.duration() > 0 {
        (source.duration() as u64) / 1_000
    } else {
        0
    };

    let mut cells: Vec<ff::frame::Video> = Vec::with_capacity(FRAMES as usize);
    let mut samples = Vec::with_capacity(FRAMES as usize);

    let mut to_cell = scaler(&decoder, CELL.0, CELL.1, ff::format::Pixel::YUVJ420P)?;
    let mut to_hash = scaler(
        &decoder,
        phash::HASH_WIDTH as u32,
        phash::HASH_HEIGHT as u32,
        ff::format::Pixel::GRAY8,
    )?;

    for index in 0..FRAMES {
        // Even intervals across the middle, avoiding the very first and last
        // frames — leaders and fades tell a reviewer nothing.
        let time_ms = duration_ms * u64::from(index * 2 + 1) / u64::from(FRAMES * 2);

        let Some(decoded) = frame_at(&mut source, &mut decoder, stream_index, time_base, time_ms)
        else {
            continue;
        };

        let mut grey = ff::frame::Video::empty();
        to_hash
            .run(&decoded, &mut grey)
            .map_err(wrap("scale for hash"))?;
        samples.push(Sample {
            index,
            time_ms,
            phash: phash::dhash(&packed(&grey)) as i64,
        });

        let mut cell = ff::frame::Video::empty();
        to_cell
            .run(&decoded, &mut cell)
            .map_err(wrap("scale for sheet"))?;
        cells.push(cell);
    }

    if cells.is_empty() {
        return Err(AvError::NoStream {
            kind: "decodable video",
        });
    }

    let thumbnails = match &options.thumbnails {
        Some(dir) => write_thumbnails(&cells, &samples, dir)?,
        None => Vec::new(),
    };

    write_sheet(&cells, out)?;
    Ok(Sheet {
        path: out.to_path_buf(),
        samples,
        thumbnails,
    })
}

/// One JPEG per sampled frame, named by the time it was taken from.
fn write_thumbnails(
    cells: &[ff::frame::Video],
    samples: &[Sample],
    dir: &Path,
) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let mut written = Vec::with_capacity(cells.len());

    for (cell, sample) in cells.iter().zip(samples) {
        let path = dir.join(format!("{:08}ms.jpg", sample.time_ms));
        std::fs::write(&path, encode_jpeg(cell)?)?;
        written.push(path);
    }
    Ok(written)
}

/// Seek to `time_ms` and decode one frame there.
///
/// Seeking lands on a keyframe, which is exactly what we want: a whole picture
/// with no preceding frames needed.
fn frame_at(
    source: &mut ff::format::context::Input,
    decoder: &mut ff::decoder::Video,
    stream_index: usize,
    time_base: ff::Rational,
    time_ms: u64,
) -> Option<ff::frame::Video> {
    let ts = (time_ms as i64) * i64::from(ff::ffi::AV_TIME_BASE) / 1000;
    let _ = time_base;
    source.seek(ts, ..ts).ok()?;
    decoder.flush();

    let mut decoded = ff::frame::Video::empty();
    // A seek lands before the target, so a handful of packets may be needed.
    for (packet_stream, packet) in source.packets().take(64) {
        if packet_stream.index() != stream_index {
            continue;
        }
        if decoder.send_packet(&packet).is_err() {
            continue;
        }
        if decoder.receive_frame(&mut decoded).is_ok() {
            return Some(decoded);
        }
    }
    None
}

/// Copy a frame's first plane out of its padded rows.
fn packed(frame: &ff::frame::Video) -> Vec<u8> {
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let stride = frame.stride(0);
    let data = frame.data(0);

    let mut out = Vec::with_capacity(width * height);
    for row in 0..height {
        let start = row * stride;
        out.extend_from_slice(&data[start..start + width]);
    }
    out
}

/// Tile the cells into a grid and encode one JPEG.
///
/// An MJPEG packet is a complete JPEG file, so there is no container to write.
fn write_sheet(cells: &[ff::frame::Video], out: &Path) -> Result<()> {
    let (cols, rows) = GRID;
    let (cell_w, cell_h) = CELL;
    let mut sheet =
        ff::frame::Video::new(ff::format::Pixel::YUVJ420P, cols * cell_w, rows * cell_h);

    // Mid-grey, so a short source leaves empty cells rather than green ones.
    for (plane, value) in [(0usize, 128u8), (1, 128), (2, 128)] {
        sheet.data_mut(plane).fill(value);
    }

    for (i, cell) in cells.iter().enumerate() {
        let col = i as u32 % cols;
        let row = i as u32 / cols;
        if row >= rows {
            break;
        }
        blit(cell, &mut sheet, col * cell_w, row * cell_h);
    }

    // YUVJ420P is deprecated but it is what mjpeg means by full range, and the
    // encoder refuses anything else. The swscaler notice about it is cosmetic.
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out, encode_jpeg(&sheet)?)?;
    Ok(())
}

/// One frame as a JPEG. An MJPEG packet is a complete JPEG file, so there is no
/// container to write.
fn encode_jpeg(frame: &ff::frame::Video) -> Result<Vec<u8>> {
    // YUVJ420P is deprecated but it is what mjpeg means by full range, and the
    // encoder refuses anything else. The swscaler notice about it is cosmetic.
    let codec = ff::encoder::find_by_name("mjpeg").ok_or_else(|| AvError::CodecUnavailable {
        codec: "mjpeg".into(),
        reason: "not in this FFmpeg build".into(),
    })?;

    let mut context = ff::codec::context::Context::new_with_codec(codec)
        .encoder()
        .video()
        .map_err(wrap("open mjpeg"))?;
    context.set_width(frame.width());
    context.set_height(frame.height());
    context.set_format(ff::format::Pixel::YUVJ420P);
    context.set_time_base(ff::Rational(1, 1));

    let mut options = ff::Dictionary::new();
    options.set("q", "6");
    let mut encoder = context
        .open_with(options)
        .map_err(wrap("configure mjpeg"))?;

    let mut copy = frame.clone();
    copy.set_pts(Some(0));
    encoder.send_frame(&copy).map_err(wrap("encode jpeg"))?;
    encoder.send_eof().map_err(wrap("finish jpeg"))?;

    let mut packet = ff::Packet::empty();
    encoder
        .receive_packet(&mut packet)
        .map_err(wrap("read jpeg"))?;
    Ok(packet
        .data()
        .ok_or_else(|| AvError::InvalidSpec("empty jpeg".into()))?
        .to_vec())
}

/// Copy one cell into the sheet at a pixel offset.
fn blit(cell: &ff::frame::Video, sheet: &mut ff::frame::Video, x: u32, y: u32) {
    // Chroma is half resolution in 4:2:0, so plane 0 steps by one and 1–2 by two.
    for plane in 0..3 {
        let shift = if plane == 0 { 0 } else { 1 };
        let width = (cell.width() >> shift) as usize;
        let height = (cell.height() >> shift) as usize;
        let (dst_x, dst_y) = ((x >> shift) as usize, (y >> shift) as usize);

        let src_stride = cell.stride(plane);
        let dst_stride = sheet.stride(plane);
        let src = cell.data(plane).to_vec();
        let dst = sheet.data_mut(plane);

        for row in 0..height {
            let from = row * src_stride;
            let to = (dst_y + row) * dst_stride + dst_x;
            if to + width > dst.len() || from + width > src.len() {
                break;
            }
            dst[to..to + width].copy_from_slice(&src[from..from + width]);
        }
    }
}

fn scaler(
    decoder: &ff::decoder::Video,
    width: u32,
    height: u32,
    format: ff::format::Pixel,
) -> Result<ff::software::scaling::Context> {
    ff::software::scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        format,
        width,
        height,
        ff::software::scaling::Flags::BILINEAR,
    )
    .map_err(wrap("build scaler"))
}

fn wrap(op: &'static str) -> impl Fn(ff::Error) -> AvError {
    move |e| AvError::Ffmpeg {
        op,
        source: Box::new(e),
    }
}
