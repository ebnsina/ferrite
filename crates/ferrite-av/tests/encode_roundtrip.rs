//! The seam through a real FFmpeg: real bytes out, and the properties the
//! split rules depend on actually hold.

#![cfg(feature = "ffmpeg")]

use ferrite_av::{
    BackendId, BackendRegistry, CancelSignal, EncodeSpec, Frame, NeverCancel, Preset, RateControl,
    Rational, Resolution, VideoCodecName, VideoEncoder,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const FPS: i32 = 30;
const FRAMES: i64 = 90; // three GOPs at the 2s default

fn spec(codec: VideoCodecName) -> EncodeSpec {
    EncodeSpec::new(codec, Resolution::new(320, 180), Rational::new(FPS, 1))
        .with_preset(Preset::Ultrafast)
        .with_rate_control(RateControl::crf(28, 500_000))
        .with_threads(2)
}

fn drain(enc: &mut Box<dyn VideoEncoder>, into: &mut Vec<ferrite_av::Packet>) {
    while let Some(p) = enc.receive_packet().expect("receive") {
        into.push(p);
    }
}

fn encode_all(spec: &EncodeSpec, cancel: Arc<dyn CancelSignal>) -> Vec<ferrite_av::Packet> {
    ferrite_av::init().expect("ffmpeg init");
    let registry = BackendRegistry::with_shipped_backends();
    let mut enc = registry.open(spec, cancel).expect("open encoder");

    let mut packets = Vec::new();
    for pts in 0..FRAMES {
        enc.send_frame(&Frame::blank_yuv420p(
            pts,
            spec.resolution.width,
            spec.resolution.height,
        ))
        .expect("send frame");
        drain(&mut enc, &mut packets);
    }
    enc.finish().expect("finish");
    drain(&mut enc, &mut packets);
    packets
}

#[test]
fn the_shipped_backend_produces_real_h264() {
    let spec = spec(VideoCodecName::H264);
    let packets = encode_all(&spec, Arc::new(NeverCancel));

    assert!(!packets.is_empty(), "encoder produced nothing");
    assert!(
        packets.iter().any(|p| !p.data.is_empty()),
        "every packet was empty"
    );
    assert!(packets[0].keyframe, "a chunk must open on a keyframe");
}

#[test]
fn keyframes_land_on_the_gop_grid_and_nowhere_else() {
    // A flat synthetic clip has no scene cuts, so this asserts the grid itself.
    let spec = spec(VideoCodecName::H264);
    let packets = encode_all(&spec, Arc::new(NeverCancel));

    let gop = i64::from(spec.gop_frames);
    let keys: Vec<i64> = packets
        .iter()
        .filter(|p| p.keyframe)
        .map(|p| p.pts)
        .collect();

    assert!(!keys.is_empty(), "no keyframes at all");
    for pts in &keys {
        assert_eq!(
            pts % gop,
            0,
            "keyframe at {pts} is off the {gop}-frame grid"
        );
    }
    assert_eq!(keys[0], 0);
}

#[test]
fn every_rung_of_a_ladder_agrees_on_where_the_cuts_are() {
    // What chunking rests on: same frames, four sizes, identical cut points.
    let ladder = [
        Resolution::new(640, 360),
        Resolution::new(480, 270),
        Resolution::new(320, 180),
        Resolution::new(256, 144),
    ];

    let mut per_rung = Vec::new();
    for res in ladder {
        let mut s = spec(VideoCodecName::H264);
        s.resolution = res;
        let keys: Vec<i64> = encode_all(&s, Arc::new(NeverCancel))
            .into_iter()
            .filter(|p| p.keyframe)
            .map(|p| p.pts)
            .collect();
        per_rung.push((res, keys));
    }

    let (_, first) = &per_rung[0];
    for (res, keys) in &per_rung[1..] {
        assert_eq!(
            keys, first,
            "{res} cuts at {keys:?} but the first rung cuts at {first:?}"
        );
    }
}

#[test]
fn cancellation_stops_a_running_encode_rather_than_the_next_chunk() {
    ferrite_av::init().expect("ffmpeg init");

    struct Flag(AtomicBool);
    impl CancelSignal for Flag {
        fn is_cancelled(&self) -> bool {
            self.0.load(Ordering::SeqCst)
        }
    }

    let flag = Arc::new(Flag(AtomicBool::new(false)));
    let spec = spec(VideoCodecName::H264);
    let registry = BackendRegistry::with_shipped_backends();
    let mut enc = registry.open(&spec, flag.clone()).expect("open encoder");

    let mut sink = Vec::new();
    for pts in 0..10 {
        enc.send_frame(&Frame::blank_yuv420p(pts, 320, 180))
            .expect("send frame");
        drain(&mut enc, &mut sink);
    }

    flag.0.store(true, Ordering::SeqCst);
    let err = enc
        .send_frame(&Frame::blank_yuv420p(10, 320, 180))
        .unwrap_err();
    assert!(
        matches!(err, ferrite_av::AvError::Cancelled),
        "expected Cancelled, got {err}"
    );
}

#[test]
fn the_rendition_record_names_what_actually_ran() {
    ferrite_av::init().expect("ffmpeg init");
    let registry = BackendRegistry::with_shipped_backends();
    let enc = registry
        .open(&spec(VideoCodecName::H264), Arc::new(NeverCancel))
        .expect("open encoder");

    let p = enc.provenance();
    assert_eq!(p.backend, BackendId::Cpu);
    assert_eq!(p.encoder, "libx264");
    assert_eq!(p.ffmpeg_version, ferrite_av::ffmpeg_version());
    assert_ne!(p.ffmpeg_version, "none");
}
