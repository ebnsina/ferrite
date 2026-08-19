//! Progressive publish: the manifest grows as rungs land, so a viewer gets a
//! playable asset without waiting for the whole ladder.

#![cfg(feature = "ffmpeg")]

use ferrite_worker::asset;
use ferrite_worker::work::{AssetJob, Rung};
use std::path::PathBuf;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Option<Self> {
        if !ferrite_av::package::binary().is_file() {
            eprintln!("skipped: no packager — run scripts/fetch-packager.sh");
            return None;
        }
        let dir = std::env::temp_dir().join(format!("ferrite-publish-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("renditions")).ok()?;
        Some(Self { dir })
    }

    /// Encode one rendition straight into the job's layout.
    fn rendition(&self, name: &str, height: u32) -> Option<Rung> {
        let source = self.dir.join("source.mp4");
        if !source.is_file() {
            let ok = std::process::Command::new("ffmpeg")
                .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
                .arg("testsrc2=size=640x360:rate=30:duration=4")
                .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=4"])
                .args([
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-g",
                    "60",
                    "-pix_fmt",
                    "yuv420p",
                ])
                .args(["-c:a", "aac", "-shortest"])
                .arg(&source)
                .status()
                .ok()?;
            if !ok.success() {
                return None;
            }
        }

        let spec = ferrite_av::EncodeSpec::new(
            ferrite_av::VideoCodecName::H264,
            ferrite_av::Resolution::new((height * 16 / 9) & !1, height),
            ferrite_av::Rational::new(30, 1),
        )
        .with_preset(ferrite_av::encoder::Preset::Ultrafast);

        ferrite_av::transcode::run(
            &source,
            &[ferrite_av::transcode::Output {
                path: self.dir.join("renditions").join(format!("{name}.mp4")),
                spec: spec.clone(),
            }],
            std::sync::Arc::new(ferrite_av::NeverCancel),
        )
        .ok()?;

        Some(Rung {
            name: name.to_string(),
            spec,
        })
    }

    fn job(&self, rungs: Vec<Rung>) -> AssetJob {
        AssetJob {
            input: self.dir.join("source.mp4"),
            out_dir: self.dir.clone(),
            plan: ferrite_av::split::plan(&[0], 4_000, 10_000),
            rungs,
        }
    }

    fn variants(&self) -> usize {
        std::fs::read_to_string(self.dir.join("cmaf").join("master.m3u8"))
            .map(|m| m.matches("#EXT-X-STREAM-INF").count())
            .unwrap_or(0)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn the_manifest_grows_as_rungs_land() {
    let Some(fx) = Fixture::new("progressive") else {
        return;
    };
    let Some(first) = fx.rendition("360p", 360) else {
        return;
    };
    let Some(second) = fx.rendition("240p", 240) else {
        return;
    };

    // Only the fast rung exists: publishable, and that is the point.
    let job = fx.job(vec![first.clone(), second.clone()]);
    let fast = fx.job(vec![first.clone()]);
    asset::publish(&fast).expect("first publish");
    assert_eq!(fx.variants(), 1, "the first publish should carry one rung");

    // The second rung lands and the manifest is rewritten to include it.
    asset::publish(&job).expect("republish");
    assert_eq!(fx.variants(), 2, "the manifest did not grow");
}

#[test]
fn publishing_twice_over_the_same_directory_works() {
    // Republishing is the normal case, not the exception: every rung that
    // lands rewrites the manifest.
    let Some(fx) = Fixture::new("republish") else {
        return;
    };
    let Some(rung) = fx.rendition("360p", 360) else {
        return;
    };
    let job = fx.job(vec![rung]);

    for attempt in 1..=3 {
        asset::publish(&job).unwrap_or_else(|e| panic!("publish {attempt}: {e}"));
        assert_eq!(fx.variants(), 1);
    }
}

#[test]
fn a_rung_that_has_not_landed_is_not_advertised() {
    let Some(fx) = Fixture::new("pending") else {
        return;
    };
    let Some(landed) = fx.rendition("360p", 360) else {
        return;
    };

    let mut spec = landed.spec.clone();
    spec.resolution = ferrite_av::Resolution::new(1280, 720);
    let job = fx.job(vec![
        landed,
        Rung {
            name: "720p".into(),
            spec,
        },
    ]);

    asset::publish(&job).expect("publish");
    let master = std::fs::read_to_string(fx.dir.join("cmaf").join("master.m3u8")).unwrap();
    assert!(
        !master.contains("720p/"),
        "advertised a rung with no segments"
    );
    assert_eq!(fx.variants(), 1);
}

#[test]
fn publishing_before_anything_has_encoded_is_refused() {
    let Some(fx) = Fixture::new("empty") else {
        return;
    };
    let spec = ferrite_av::EncodeSpec::new(
        ferrite_av::VideoCodecName::H264,
        ferrite_av::Resolution::new(640, 360),
        ferrite_av::Rational::new(30, 1),
    );
    let job = fx.job(vec![Rung {
        name: "360p".into(),
        spec,
    }]);
    assert!(asset::publish(&job).is_err());
}
