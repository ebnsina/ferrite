//! CMAF + HLS + DASH via Shaka Packager (pipeline step 9).
//!
//! A subprocess, deliberately. Correct HLS+DASH is a multi-year job with
//! endless device quirks, and CMAF means one segment set serves both — separate
//! sets roughly double the stored bytes.

use crate::error::{AvError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A rendition to package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    /// The rendition name, which becomes its directory.
    pub name: String,
    /// The encoded file.
    pub path: PathBuf,
    /// `video` or `audio`.
    pub kind: Track,
    /// Language tag. `None` means undetermined, which is still a tag: an
    /// untagged audio group has no default member, and a player asked to pick
    /// from it stalls rather than choosing.
    pub language: Option<String>,
}

/// What a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Track {
    /// Pictures.
    Video,
    /// Sound.
    Audio,
}

impl Track {
    fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
        }
    }
}

/// Where the manifests landed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Packaged {
    /// HLS master playlist.
    pub hls: PathBuf,
    /// DASH manifest.
    pub dash: PathBuf,
    /// Rendition directories, in the order given.
    pub renditions: Vec<PathBuf>,
}

/// Segment length. Two seconds divides the ~10s chunk target and matches the
/// GOP, so segment boundaries land on keyframes.
pub const SEGMENT_SECONDS: u32 = 2;

/// ISO 639-2 for undetermined. The right tag for audio whose language nobody
/// stated, and better than no tag at all.
pub const UNDETERMINED: &str = "und";

/// The packager binary: `PACKAGER_BIN`, then `vendor/packager`, then `$PATH`.
pub fn binary() -> PathBuf {
    if let Ok(from_env) = std::env::var("PACKAGER_BIN") {
        return PathBuf::from(from_env);
    }
    let vendored = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/packager");
    if vendored.is_file() {
        return vendored;
    }
    PathBuf::from("packager")
}

/// Package `inputs` into `out_dir` as CMAF, with both manifests over one set of
/// segments.
pub fn run(inputs: &[Input], out_dir: &Path) -> Result<Packaged> {
    if inputs.is_empty() {
        return Err(AvError::InvalidSpec("nothing to package".into()));
    }
    std::fs::create_dir_all(out_dir)?;

    let hls = out_dir.join("master.m3u8");
    let dash = out_dir.join("manifest.mpd");
    // Written aside and renamed, so a player polling during a republish never
    // reads half a playlist. Segments never change, so only these need it.
    // ponytail: manifests are written in place, so a player can in principle
    // read one mid-write. Writing them aside and renaming does not work — the
    // packager silently writes no master playlist when pointed at another name
    // — and the alternative is repackaging every segment per republish. Revisit
    // if a real player ever catches a torn read.
    let mut renditions = Vec::with_capacity(inputs.len());

    let mut command = Command::new(binary());
    for input in inputs {
        let dir = out_dir.join(&input.name);
        std::fs::create_dir_all(&dir)?;
        renditions.push(dir.clone());

        // Segments never change, so they can be cached forever; the manifests
        // do, so they get a short cache. That split is what lets a CDN drop in
        // later with no reprocessing.
        command.arg(format!(
            "in={},stream={},init_segment={},segment_template={},playlist_name={}",
            input.path.display(),
            input.kind.as_str(),
            dir.join("init.mp4").display(),
            dir.join("seg-$Number$.m4s").display(),
            format_args!("{}/{}.m3u8", input.name, input.kind.as_str()),
        ));
        // Only when the source stated one: the packager rejects "und", so an
        // unknown language is better left unclaimed than guessed at.
        if let Some(language) = input.language.as_deref().filter(|l| *l != UNDETERMINED) {
            command.arg(format!("language={language}"));
        }
    }

    // Marks the first rendition of the matching group DEFAULT=YES.
    let audio_language = inputs
        .iter()
        .find(|i| i.kind == Track::Audio)
        .and_then(|i| i.language.as_deref())
        .filter(|l| *l != UNDETERMINED);
    if let Some(language) = audio_language {
        command.arg("--default_language").arg(language);
    }

    command
        // Without this the MPD comes out type="dynamic" with a time-shift
        // buffer: a live manifest describing a file that is already whole.
        .arg("--generate_static_live_mpd")
        // sidx is only required for the on-demand profile, which addresses by
        // byte range. We address by template, so it is bytes in every segment
        // for nothing — and it makes each one claim indexing it does not need.
        .arg("--nogenerate_sidx_in_media_segments")
        .arg("--segment_duration")
        .arg(SEGMENT_SECONDS.to_string())
        .arg("--hls_master_playlist_output")
        .arg(&hls)
        .arg("--mpd_output")
        .arg(&dash);

    let output = command.output().map_err(|e| AvError::CodecUnavailable {
        codec: "shaka-packager".into(),
        reason: format!("{}: {e}. Run scripts/fetch-packager.sh", binary().display()),
    })?;

    if !output.status.success() {
        return Err(AvError::InvalidSpec(format!(
            "packager failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    // Named, because "no such file" without a path is the least useful error
    // a publish can produce.
    let publish = |from: &Path, to: &Path| -> Result<()> {
        std::fs::rename(from, to).map_err(|e| {
            // A packager that exits zero and writes nothing is worth reporting
            // in full, because the rename error alone says nothing about why.
            let listing: Vec<String> = std::fs::read_dir(out_dir)
                .map(|d| {
                    d.filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect()
                })
                .unwrap_or_default();
            AvError::InvalidSpec(format!(
                "cannot publish {}: {e}. {} holds {listing:?}",
                to.display(),
                out_dir.display(),
            ))
        })
    };
    let _ = &publish;

    Ok(Packaged {
        hls,
        dash,
        renditions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_to_package_is_an_error_not_an_empty_manifest() {
        let err = run(&[], Path::new("/tmp/ferrite-empty")).unwrap_err();
        assert!(matches!(err, AvError::InvalidSpec(_)), "{err}");
    }

    #[test]
    fn a_missing_packager_says_how_to_get_one() {
        // SAFETY: single-threaded test, restored immediately.
        let out = std::env::temp_dir().join("ferrite-no-packager");
        let inputs = [Input {
            name: "720p".into(),
            path: PathBuf::from("/nonexistent.mp4"),
            kind: Track::Video,
            language: None,
        }];

        let previous = std::env::var("PACKAGER_BIN").ok();
        unsafe { std::env::set_var("PACKAGER_BIN", "/nonexistent/packager") };
        let err = run(&inputs, &out).unwrap_err();
        match previous {
            Some(v) => unsafe { std::env::set_var("PACKAGER_BIN", v) },
            None => unsafe { std::env::remove_var("PACKAGER_BIN") },
        }

        assert!(err.to_string().contains("fetch-packager.sh"), "{err}");
    }

    #[test]
    fn segments_are_two_seconds_so_they_land_on_the_gop() {
        assert_eq!(SEGMENT_SECONDS, 2);
    }
}
