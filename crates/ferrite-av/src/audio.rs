//! One audio rendition, encoded once (pipeline step 6, audio side).
//!
//! Never chunked: an encoder's priming samples make every join a click, and a
//! drifting one at that. One pass, one file, shared by every video rung.
//!
//! Normalising matters more than it looks — an MP3 soundtrack in an MP4 is
//! something Shaka Packager refuses outright, so passing source audio straight
//! through fails on real uploads.

use crate::error::{AvError, Result};
use ffmpeg_next as ff;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What to produce. AAC-LC stereo is what every device plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSpec {
    /// Bits per second.
    pub bitrate: u32,
    /// Output sample rate.
    pub sample_rate: u32,
    /// Output channels. Anything wider is downmixed.
    pub channels: u16,
}

impl Default for AudioSpec {
    fn default() -> Self {
        Self {
            bitrate: 128_000,
            sample_rate: 48_000,
            channels: 2,
        }
    }
}

/// What was produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioReport {
    /// The encoded file.
    pub path: PathBuf,
    /// Codec name, for the rendition record.
    pub codec: String,
    /// Output sample rate.
    pub sample_rate: u32,
    /// Output channels.
    pub channels: u16,
    /// Compressed bytes.
    pub bytes: u64,
}

/// Encode `input`'s audio into `output`.
///
/// Returns `Ok(None)` when the source is silent: that is not an error, and a
/// missing track is better than an empty one.
pub fn encode(input: &Path, output: &Path, spec: &AudioSpec) -> Result<Option<AudioReport>> {
    crate::init()?;

    let mut source = ff::format::input(input).map_err(|e| AvError::OpenInput {
        path: input.display().to_string(),
        source: Box::new(e),
    })?;

    let Some(stream) = source.streams().best(ff::media::Type::Audio) else {
        return Ok(None);
    };
    let stream_index = stream.index();
    let time_base = stream.time_base();

    let mut decoder = ff::codec::context::Context::from_parameters(stream.parameters())
        .map_err(wrap("read audio parameters"))?
        .decoder()
        .audio()
        .map_err(wrap("open audio decoder"))?;

    let codec = ff::encoder::find_by_name("aac").ok_or_else(|| AvError::CodecUnavailable {
        codec: "aac".into(),
        reason: "not in this FFmpeg build".into(),
    })?;

    let layout = ff::ChannelLayout::default(i32::from(spec.channels));
    let mut context = ff::codec::context::Context::new_with_codec(codec)
        .encoder()
        .audio()
        .map_err(wrap("open aac"))?;
    context.set_rate(spec.sample_rate as i32);
    context.set_channel_layout(layout);
    context.set_format(ff::format::Sample::F32(ff::format::sample::Type::Planar));
    context.set_bit_rate(spec.bitrate as usize);
    context.set_time_base(ff::Rational(1, spec.sample_rate as i32));
    let mut encoder = context.open().map_err(wrap("configure aac"))?;

    let mut graph = build_graph(&decoder, time_base, spec, layout)?;
    // The encoder wants exactly this many samples per frame; libavfilter can
    // deliver that, which is the whole reason not to hand-roll a sample FIFO.
    if let Some(mut sink) = graph.get("out") {
        sink.sink().set_frame_size(encoder.frame_size());
    }

    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    let mut muxer = ff::format::output(&output).map_err(wrap("open audio output"))?;
    let mut out_stream = muxer.add_stream(codec).map_err(wrap("add audio stream"))?;
    out_stream.set_time_base(ff::Rational(1, spec.sample_rate as i32));
    out_stream.set_parameters(ff::codec::Parameters::from(&encoder));
    muxer.write_header().map_err(wrap("write audio header"))?;
    let stream_time_base = muxer
        .streams()
        .next()
        .ok_or_else(|| AvError::InvalidSpec("audio muxer lost its stream".into()))?
        .time_base();

    let mut bytes = 0u64;
    let mut samples = 0i64;
    let mut decoded = ff::frame::Audio::empty();

    for (packet_stream, packet) in source.packets() {
        if packet_stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(wrap("send audio packet"))?;
        while decoder.receive_frame(&mut decoded).is_ok() {
            feed(&mut graph, &decoded)?;
            drain(
                &mut graph,
                &mut encoder,
                &mut muxer,
                &mut samples,
                &mut bytes,
                spec,
                stream_time_base,
            )?;
        }
    }

    decoder.send_eof().map_err(wrap("audio decoder eof"))?;
    while decoder.receive_frame(&mut decoded).is_ok() {
        feed(&mut graph, &decoded)?;
        drain(
            &mut graph,
            &mut encoder,
            &mut muxer,
            &mut samples,
            &mut bytes,
            spec,
            stream_time_base,
        )?;
    }

    if let Some(mut src) = graph.get("in") {
        src.source().flush().map_err(wrap("flush audio filter"))?;
    }
    drain(
        &mut graph,
        &mut encoder,
        &mut muxer,
        &mut samples,
        &mut bytes,
        spec,
        stream_time_base,
    )?;

    encoder.send_eof().map_err(wrap("aac eof"))?;
    write_packets(&mut encoder, &mut muxer, &mut bytes, spec, stream_time_base)?;
    muxer.write_trailer().map_err(wrap("write audio trailer"))?;

    Ok(Some(AudioReport {
        path: output.to_path_buf(),
        codec: "aac".into(),
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bytes,
    }))
}

fn feed(graph: &mut ff::filter::Graph, frame: &ff::frame::Audio) -> Result<()> {
    let mut source = graph
        .get("in")
        .ok_or_else(|| AvError::InvalidSpec("audio graph lost its source".into()))?;
    // KEEP_REF for the same reason as video: add() would blank the caller's frame.
    // SAFETY: the frame outlives this call and the graph takes its own reference.
    let rc = unsafe {
        ff::ffi::av_buffersrc_add_frame_flags(
            source.as_mut_ptr(),
            frame.as_ptr() as *mut _,
            ff::ffi::AV_BUFFERSRC_FLAG_KEEP_REF as i32,
        )
    };
    if rc < 0 {
        return Err(wrap("feed audio filter")(ff::Error::from(rc)));
    }
    Ok(())
}

fn drain(
    graph: &mut ff::filter::Graph,
    encoder: &mut ff::encoder::Audio,
    muxer: &mut ff::format::context::Output,
    samples: &mut i64,
    bytes: &mut u64,
    spec: &AudioSpec,
    stream_time_base: ff::Rational,
) -> Result<()> {
    let mut filtered = ff::frame::Audio::empty();
    loop {
        let got = graph
            .get("out")
            .ok_or_else(|| AvError::InvalidSpec("audio graph lost its sink".into()))?
            .sink()
            .frame(&mut filtered)
            .is_ok();
        if !got {
            return Ok(());
        }

        // Timestamps are a running sample count, which is exact at a fixed rate.
        filtered.set_pts(Some(*samples));
        *samples += filtered.samples() as i64;
        encoder
            .send_frame(&filtered)
            .map_err(wrap("encode audio"))?;
        write_packets(encoder, muxer, bytes, spec, stream_time_base)?;
    }
}

fn write_packets(
    encoder: &mut ff::encoder::Audio,
    muxer: &mut ff::format::context::Output,
    bytes: &mut u64,
    spec: &AudioSpec,
    stream_time_base: ff::Rational,
) -> Result<()> {
    let encoder_time_base = ff::Rational(1, spec.sample_rate as i32);
    let mut packet = ff::Packet::empty();
    while encoder.receive_packet(&mut packet).is_ok() {
        packet.set_stream(0);
        packet.rescale_ts(encoder_time_base, stream_time_base);
        *bytes += packet.size() as u64;
        packet.write_interleaved(muxer).map_err(wrap("mux audio"))?;
    }
    Ok(())
}

/// abuffer → aresample → aformat → abuffersink. Downmixing and rate conversion
/// are what `aformat` is for; hand-rolling either is a bug farm.
fn build_graph(
    decoder: &ff::decoder::Audio,
    time_base: ff::Rational,
    spec: &AudioSpec,
    layout: ff::ChannelLayout,
) -> Result<ff::filter::Graph> {
    let args = format!(
        "time_base={}/{}:sample_rate={}:sample_fmt={}:channel_layout=0x{:x}",
        time_base.numerator(),
        time_base.denominator(),
        decoder.rate(),
        decoder.format().name(),
        decoder.channel_layout().bits(),
    );

    let mut graph = ff::filter::Graph::new();
    let abuffer = ff::filter::find("abuffer")
        .ok_or_else(|| AvError::InvalidSpec("this FFmpeg has no abuffer filter".into()))?;
    let abuffersink = ff::filter::find("abuffersink")
        .ok_or_else(|| AvError::InvalidSpec("this FFmpeg has no abuffersink filter".into()))?;

    graph
        .add(&abuffer, "in", &args)
        .map_err(wrap("add audio source"))?;
    graph
        .add(&abuffersink, "out", "")
        .map_err(wrap("add audio sink"))?;

    let chain = format!(
        "aresample={},aformat=sample_fmts=fltp:sample_rates={}:channel_layouts=0x{:x}",
        spec.sample_rate,
        spec.sample_rate,
        layout.bits(),
    );
    graph
        .output("in", 0)
        .map_err(wrap("wire audio source"))?
        .input("out", 0)
        .map_err(wrap("wire audio sink"))?
        .parse(&chain)
        .map_err(wrap("parse audio chain"))?;
    graph.validate().map_err(wrap("validate audio chain"))?;
    Ok(graph)
}

fn wrap(op: &'static str) -> impl Fn(ff::Error) -> AvError {
    move |e| AvError::Ffmpeg {
        op,
        source: Box::new(e),
    }
}
