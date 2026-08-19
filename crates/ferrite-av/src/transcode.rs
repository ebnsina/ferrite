//! Decode once, encode many (pipeline step 6).
//!
//! The biggest saving in the design: one task per chunk *per rung* decodes the
//! same pictures four to six times, and decode is 15–40% of the work.
//!
//! The mezzanine is this with a single output, so there is one code path.

use crate::encoder::{
    BackendRegistry, CancelSignal, EncodeSpec, Frame, NeverCancel, Provenance, VideoEncoder,
};
use crate::error::{AvError, Result};
use ffmpeg_next as ff;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One file to produce.
#[derive(Debug, Clone)]
pub struct Output {
    /// Where to write it.
    pub path: PathBuf,
    /// How to encode it.
    pub spec: EncodeSpec,
}

/// What one output cost and what produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Where it was written.
    pub path: PathBuf,
    /// Frames encoded.
    pub frames: u64,
    /// Compressed bytes written.
    pub bytes: u64,
    /// Recorded per rendition, next to the FFmpeg version.
    pub provenance: Provenance,
}

/// Decode `input` once and produce every output.
///
/// Cancellation is checked per frame, so a cancelled four-hour encode stops
/// now rather than at the next chunk boundary.
pub fn run(input: &Path, outputs: &[Output], cancel: Arc<dyn CancelSignal>) -> Result<Vec<Report>> {
    if outputs.is_empty() {
        return Ok(Vec::new());
    }
    for out in outputs {
        out.spec.validate()?;
    }
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

    let mut decoder = ff::codec::context::Context::from_parameters(stream.parameters())
        .map_err(wrap("read parameters"))?
        .decoder()
        .video()
        .map_err(wrap("open decoder"))?;

    // The filter chain needs the source's rotation, which lives in side data
    // rather than on the decoder.
    let probed = crate::probe(input)?;
    let source_stream = probed
        .primary_video()
        .cloned()
        .ok_or(AvError::NoStream { kind: "video" })?;
    let source_time_base = source
        .streams()
        .find(|s| s.index() == stream_index)
        .map(|s| s.time_base())
        .unwrap_or(ff::Rational(1, 1000));

    let registry = BackendRegistry::with_shipped_backends();
    let mut sinks: Vec<Sink> = outputs
        .iter()
        .map(|o| {
            Sink::open(
                o,
                &registry,
                &decoder,
                &source_stream,
                source_time_base,
                cancel.clone(),
            )
        })
        .collect::<Result<_>>()?;

    let mut decoded = ff::frame::Video::empty();
    for (packet_stream, packet) in source.packets() {
        if packet_stream.index() != stream_index {
            continue;
        }
        decoder.send_packet(&packet).map_err(wrap("send packet"))?;
        while decoder.receive_frame(&mut decoded).is_ok() {
            for sink in &mut sinks {
                sink.push(&decoded)?;
            }
        }
    }

    decoder.send_eof().map_err(wrap("decoder eof"))?;
    while decoder.receive_frame(&mut decoded).is_ok() {
        for sink in &mut sinks {
            sink.push(&decoded)?;
        }
    }

    sinks.into_iter().map(Sink::finish).collect()
}

/// The filter chain that turns a decoded picture into what the encoder wants:
/// rotation baked into pixels, a fixed frame rate, one size, one pixel format.
///
/// libavfilter does all four; hand-rolling any of them would be worse at it.
fn chain(source: &crate::media::VideoStream, spec: &EncodeSpec) -> String {
    let mut steps: Vec<String> = Vec::new();

    // Directions verified against ffmpeg's own autorotate, not reasoned about:
    // the two conventions read alike and are opposites. See the rotation tests.
    match source.rotation_degrees.rem_euclid(360) {
        90 => steps.push("transpose=clock".into()),
        180 => steps.push("hflip,vflip".into()),
        270 => steps.push("transpose=cclock".into()),
        _ => {}
    }

    steps.push(format!(
        "fps={}/{}",
        spec.frame_rate.num, spec.frame_rate.den
    ));
    steps.push(format!(
        "scale={}:{}:flags=bicubic",
        spec.resolution.width, spec.resolution.height
    ));
    steps.push("setsar=1/1".into());
    steps.push(format!("format={}", spec.pixel_format.as_str()));
    steps.join(",")
}

/// One output's filter graph, encoder and muxer.
struct Sink {
    path: PathBuf,
    graph: ff::filter::Graph,
    encoder: Box<dyn VideoEncoder>,
    muxer: ff::format::context::Output,
    filtered: ff::frame::Video,
    time_base: ff::Rational,
    stream_time_base: ff::Rational,
    frames: u64,
    bytes: u64,
}

impl Sink {
    fn open(
        out: &Output,
        registry: &BackendRegistry,
        decoder: &ff::decoder::Video,
        source_stream: &crate::media::VideoStream,
        source_time_base: ff::Rational,
        cancel: Arc<dyn CancelSignal>,
    ) -> Result<Self> {
        let spec = &out.spec;
        let format = match spec.pixel_format {
            crate::PixelFormat::Yuv420p => ff::format::Pixel::YUV420P,
            crate::PixelFormat::Yuv420p10le => ff::format::Pixel::YUV420P10LE,
        };

        let graph = build_graph(decoder, source_stream, source_time_base, spec)?;

        let encoder = registry.open(spec, cancel)?;

        if let Some(parent) = out.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let mut muxer = ff::format::output(&out.path).map_err(|e| AvError::Ffmpeg {
            op: "open output",
            source: Box::new(e),
        })?;

        let codec = ff::encoder::find_by_name(crate::encoder::CpuBackend::encoder_name(spec.codec))
            .ok_or_else(|| AvError::CodecUnavailable {
                codec: spec.codec.to_string(),
                reason: "not in this FFmpeg build".into(),
            })?;

        // The muxer needs the same parameters the encoder was opened with.
        let mut params = ff::codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()
            .map_err(wrap("describe stream"))?;
        params.set_width(spec.resolution.width);
        params.set_height(spec.resolution.height);
        params.set_format(format);
        let time_base = ff::Rational(spec.frame_rate.den, spec.frame_rate.num);
        params.set_time_base(time_base);

        // The muxer needs the encoder's setup bytes, not just its geometry:
        // MP4 stores SPS/PPS out of band, and without them the file decodes to
        // nothing while still looking well-formed.
        let mut parameters = ff::codec::Parameters::from(&params);
        set_extradata(&mut parameters, &encoder.extradata());

        let frame_rate = ff::Rational(spec.frame_rate.num, spec.frame_rate.den);
        let mut stream = muxer.add_stream(codec).map_err(wrap("add stream"))?;
        stream.set_time_base(time_base);
        stream.set_avg_frame_rate(frame_rate);
        stream.set_parameters(parameters);

        muxer.write_header().map_err(wrap("write header"))?;

        // write_header is where a container picks its own timescale — MP4 always
        // does. Reading the time base before this point rescales every timestamp
        // against a number the file does not use.
        let stream_time_base = muxer
            .streams()
            .next()
            .ok_or_else(|| AvError::InvalidSpec("muxer lost its stream".into()))?
            .time_base();

        Ok(Self {
            path: out.path.clone(),
            graph,
            encoder,
            muxer,
            filtered: ff::frame::Video::empty(),
            time_base,
            stream_time_base,
            frames: 0,
            bytes: 0,
        })
    }

    fn push(&mut self, decoded: &ff::frame::Video) -> Result<()> {
        let mut source = self
            .graph
            .get("in")
            .ok_or_else(|| AvError::InvalidSpec("filter graph lost its source".into()))?;

        // KEEP_REF, because the plain add() hands the picture to the filter and
        // blanks the caller's frame — which would leave every rung after the
        // first fed an empty picture. Decode-once means one picture, N readers.
        // SAFETY: the frame outlives this call and the graph takes its own ref.
        let rc = unsafe {
            ff::ffi::av_buffersrc_add_frame_flags(
                source.as_mut_ptr(),
                decoded.as_ptr() as *mut _,
                ff::ffi::AV_BUFFERSRC_FLAG_KEEP_REF as i32,
            )
        };
        if rc < 0 {
            return Err(wrap("feed filter")(ff::Error::from(rc)));
        }
        self.pull()
    }

    /// Take everything the graph will give us. The fps filter drops and
    /// duplicates frames, so one in is not one out.
    fn pull(&mut self) -> Result<()> {
        loop {
            let got = self
                .graph
                .get("out")
                .ok_or_else(|| AvError::InvalidSpec("filter graph lost its sink".into()))?
                .sink()
                .frame(&mut self.filtered)
                .is_ok();
            if !got {
                return Ok(());
            }

            // The chain fixes the frame rate, so the output really is evenly
            // spaced and a frame index is an honest timestamp.
            let pts = self.frames as i64;
            self.filtered.set_pts(Some(pts));
            self.encoder
                .send_frame(&Frame::from_video(pts, self.filtered.clone()))?;
            self.frames += 1;
            self.drain()?;
        }
    }

    fn drain(&mut self) -> Result<()> {
        while let Some(packet) = self.encoder.receive_packet()? {
            let mut out = ff::Packet::copy(&packet.data);
            out.set_stream(0);
            out.set_pts(Some(packet.pts));
            out.set_dts(Some(packet.dts));
            if packet.keyframe {
                out.set_flags(ff::codec::packet::Flags::KEY);
            }
            out.rescale_ts(self.time_base, self.stream_time_base);
            self.bytes += packet.data.len() as u64;
            out.write_interleaved(&mut self.muxer)
                .map_err(wrap("mux packet"))?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Report> {
        if let Some(mut source) = self.graph.get("in") {
            source.source().flush().map_err(wrap("flush filter"))?;
        }
        self.pull()?;
        self.encoder.finish()?;
        self.drain()?;
        self.muxer.write_trailer().map_err(wrap("write trailer"))?;
        Ok(Report {
            path: self.path,
            frames: self.frames,
            bytes: self.bytes,
            provenance: self.encoder.provenance(),
        })
    }
}

/// One output at the source's own size: fixed frame rate, timestamps from zero,
/// rotation already baked in by the scaler.
pub fn mezzanine(input: &Path, output: &Path, spec: EncodeSpec) -> Result<Report> {
    let outputs = [Output {
        path: output.to_path_buf(),
        spec,
    }];
    run(input, &outputs, Arc::new(NeverCancel))?
        .pop()
        .ok_or_else(|| AvError::InvalidSpec("mezzanine produced no output".into()))
}

/// Attach codec setup bytes to stream parameters.
fn set_extradata(parameters: &mut ff::codec::Parameters, extradata: &[u8]) {
    if extradata.is_empty() {
        return;
    }
    // SAFETY: FFmpeg frees this buffer when Parameters drops, so it must come
    // from av_mallocz, and it must carry the decoder's read-ahead padding.
    unsafe {
        let padded = extradata.len() + ff::ffi::AV_INPUT_BUFFER_PADDING_SIZE as usize;
        let buffer = ff::ffi::av_mallocz(padded).cast::<u8>();
        if buffer.is_null() {
            return;
        }
        std::ptr::copy_nonoverlapping(extradata.as_ptr(), buffer, extradata.len());
        let p = parameters.as_mut_ptr();
        (*p).extradata = buffer;
        (*p).extradata_size = extradata.len() as i32;
    }
}

/// buffer → chain → buffersink, described to libavfilter as a string.
fn build_graph(
    decoder: &ff::decoder::Video,
    source_stream: &crate::media::VideoStream,
    time_base: ff::Rational,
    spec: &EncodeSpec,
) -> Result<ff::filter::Graph> {
    let pixel = decoder
        .format()
        .descriptor()
        .map(|d| d.name())
        .ok_or_else(|| AvError::InvalidSpec("source has no known pixel format".into()))?;

    let args = format!(
        "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect=1/1",
        decoder.width(),
        decoder.height(),
        pixel,
        time_base.numerator(),
        time_base.denominator(),
    );

    let mut graph = ff::filter::Graph::new();
    let buffer = ff::filter::find("buffer")
        .ok_or_else(|| AvError::InvalidSpec("this FFmpeg has no buffer filter".into()))?;
    let sink = ff::filter::find("buffersink")
        .ok_or_else(|| AvError::InvalidSpec("this FFmpeg has no buffersink filter".into()))?;

    graph
        .add(&buffer, "in", &args)
        .map_err(wrap("add filter source"))?;
    graph
        .add(&sink, "out", "")
        .map_err(wrap("add filter sink"))?;

    let spec_str = chain(source_stream, spec);
    graph
        .output("in", 0)
        .map_err(wrap("wire filter source"))?
        .input("out", 0)
        .map_err(wrap("wire filter sink"))?
        .parse(&spec_str)
        .map_err(wrap("parse filter chain"))?;
    graph.validate().map_err(wrap("validate filter chain"))?;
    Ok(graph)
}

fn wrap(op: &'static str) -> impl Fn(ff::Error) -> AvError {
    move |e| AvError::Ffmpeg {
        op,
        source: Box::new(e),
    }
}
