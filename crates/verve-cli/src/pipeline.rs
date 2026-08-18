//! Local pipeline commands. No Temporal, no scheduler, no Postgres — you debug
//! an encode on a laptop with the code the fleet runs.

use anyhow::{Result, bail};
use clap::Args;
use std::path::PathBuf;

/// Arguments for `verve probe`.
#[derive(Debug, Args)]
pub struct ProbeArgs {
    /// The file to read.
    pub file: PathBuf,
}

/// Placeholder arguments for commands that land in Stage 2.
#[derive(Debug, Args)]
pub struct StubArgs {
    /// Anything the eventual command will take.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub rest: Vec<String>,
}

/// `verve probe`.
pub fn probe(args: &ProbeArgs, json: bool) -> Result<()> {
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = (args, json);
        bail!("this build has no FFmpeg; rebuild with --features ffmpeg");
    }
    #[cfg(feature = "ffmpeg")]
    {
        let info = verve_av::probe(&args.file)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&info)?);
            return Ok(());
        }

        println!("{}", args.file.display());
        println!("  format      {}", info.format);
        println!("  duration    {:.3}s", info.duration_ms as f64 / 1000.0);
        println!("  size        {} bytes", info.size_bytes);
        if let Some(v) = info.primary_video() {
            let (w, h) = v.display_size();
            println!("  video       {} {w}x{h} @ {} fps", v.codec, v.frame_rate);
            println!("  pixels      {}", v.pixel_format);
        }
        for a in &info.audio {
            println!(
                "  audio       {} {} Hz {}ch",
                a.codec, a.sample_rate, a.channels
            );
        }
        println!("  keyframes   {}", info.keyframes_ms.len());
        if info.warnings.is_empty() {
            println!("  warnings    none");
        } else {
            for w in &info.warnings {
                println!("  warning     {w}");
            }
        }
        println!(
            "  mezzanine   {}",
            if info.needs_mezzanine(1) {
                "required"
            } else {
                "skippable in job mode"
            }
        );
        Ok(())
    }
}

/// `verve doctor`. What this build can actually do.
pub fn doctor(json: bool) -> Result<()> {
    use verve_av::{BackendRegistry, VideoCodecName};

    verve_av::init()?;
    let registry = BackendRegistry::with_shipped_backends();
    let codecs: Vec<(&str, bool)> = [
        VideoCodecName::H264,
        VideoCodecName::H265,
        VideoCodecName::Av1,
    ]
    .into_iter()
    .map(|c| (c.as_str(), registry.for_codec(c).is_ok()))
    .collect();

    if json {
        let value = serde_json::json!({
            "ffmpeg": verve_av::ffmpeg_version(),
            "has_ffmpeg": verve_av::has_ffmpeg(),
            "backends": registry.backends().iter().map(|b| b.id().as_str()).collect::<Vec<_>>(),
            "codecs": codecs.iter().map(|(c, ok)| (*c, *ok)).collect::<std::collections::BTreeMap<_, _>>(),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    println!("ffmpeg      {}", verve_av::ffmpeg_version());
    println!(
        "backends    {}",
        registry
            .backends()
            .iter()
            .map(|b| b.id().as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    for (codec, ok) in &codecs {
        println!("  {codec:<8}  {}", if *ok { "yes" } else { "no" });
    }
    if !verve_av::has_ffmpeg() {
        println!("\nthis build has no FFmpeg; local pipeline commands will refuse to run");
    }
    Ok(())
}

/// A command whose stage has not landed yet. Says so rather than pretending.
pub fn not_yet(name: &str, _args: &StubArgs) -> Result<()> {
    bail!("`verve {name}` lands in a later stage; see docs/07-implementation.md")
}
