//! Local pipeline commands. No Temporal, no scheduler, no Postgres — you debug
//! an encode on a laptop with the code the fleet runs.

use anyhow::{Context as _, Result};
use clap::Args;
use std::path::PathBuf;

/// Arguments for `verve probe`.
#[derive(Debug, Args)]
pub struct ProbeArgs {
    /// The file to read.
    pub file: PathBuf,
}

/// Read a file, or say why this build cannot.
fn probe_file(path: &std::path::Path) -> Result<verve_av::MediaInfo> {
    #[cfg(feature = "ffmpeg")]
    {
        Ok(verve_av::probe(path)?)
    }
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = path;
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
}

/// `verve probe`.
pub fn probe(args: &ProbeArgs, json: bool) -> Result<()> {
    {
        let info = probe_file(&args.file)?;
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

/// `verve split`.
pub fn split(args: &ProbeArgs, json: bool) -> Result<()> {
    let info = probe_file(&args.file)?;
    let plan = verve_av::split::plan(
        &info.keyframes_ms,
        info.duration_ms,
        verve_av::split::TARGET_CHUNK_MS,
    );

    if json {
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }

    match plan.whole {
        Some(reason) => println!("one chunk ({reason:?})"),
        None => println!("{} chunks", plan.chunks.len()),
    }
    for c in &plan.chunks {
        println!(
            "  {:>4}  {:>9.3}s → {:>9.3}s  ({:.3}s)",
            c.index,
            c.start_ms as f64 / 1000.0,
            c.end_ms as f64 / 1000.0,
            c.duration_ms() as f64 / 1000.0
        );
    }
    Ok(())
}

/// `verve ladder`.
pub fn ladder(args: &ProbeArgs, json: bool) -> Result<()> {
    use verve_av::encoder::Preset;

    let info = probe_file(&args.file)?;
    let Some(video) = info.primary_video() else {
        anyhow::bail!("no video stream in {}", args.file.display());
    };
    let steps = verve_av::ladder::plan(video, &verve_av::ladder::STANDARD, Preset::Medium);
    let fast = verve_av::ladder::fast_path(&steps).map(|s| s.name);

    if json {
        let rows: Vec<_> = steps
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "width": s.spec.resolution.width,
                    "height": s.spec.resolution.height,
                    "rate_control": s.spec.rate_control,
                    "fast_path": Some(s.name) == fast,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    let (w, h) = video.display_size();
    println!("source      {w}x{h}");
    for s in &steps {
        let marker = if Some(s.name) == fast {
            "  ← fast path"
        } else {
            ""
        };
        println!("  {:<6} {}{marker}", s.name, s.spec.resolution);
    }
    Ok(())
}

/// Arguments for `verve encode`.
#[derive(Debug, Args)]
pub struct EncodeArgs {
    /// The file to encode.
    pub file: PathBuf,
    /// Directory to write renditions into.
    #[arg(short, long, default_value = "out")]
    pub out: PathBuf,
    /// Encode the fast-path rung only.
    #[arg(long)]
    pub fast: bool,
}

/// `verve encode`.
pub fn encode(args: &EncodeArgs, json: bool) -> Result<()> {
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = (args, json);
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
    #[cfg(feature = "ffmpeg")]
    {
        use std::time::Instant;
        use verve_av::encoder::Preset;
        use verve_av::transcode::Output;

        let info = probe_file(&args.file)?;
        let Some(video) = info.primary_video() else {
            anyhow::bail!("no video stream in {}", args.file.display());
        };

        let preset = if args.fast {
            Preset::Veryfast
        } else {
            Preset::Medium
        };
        let mut steps = verve_av::ladder::plan(video, &verve_av::ladder::STANDARD, preset);
        if args.fast {
            steps = verve_av::ladder::fast_path(&steps)
                .cloned()
                .into_iter()
                .collect();
        }

        let outputs: Vec<Output> = steps
            .iter()
            .map(|s| Output {
                path: args.out.join(format!("{}.mp4", s.name)),
                spec: s.spec.clone(),
            })
            .collect();

        let started = Instant::now();
        let reports = verve_av::transcode::run(
            &args.file,
            &outputs,
            std::sync::Arc::new(verve_av::NeverCancel),
        )?;
        let elapsed = started.elapsed();

        if json {
            let rows: Vec<_> = reports
                .iter()
                .zip(&steps)
                .map(|(r, s)| {
                    serde_json::json!({
                        "name": s.name,
                        "path": r.path,
                        "frames": r.frames,
                        "bytes": r.bytes,
                        "provenance": r.provenance,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&rows)?);
            return Ok(());
        }

        for (r, s) in reports.iter().zip(&steps) {
            println!(
                "  {:<6} {:<10} {:>7} frames  {:>9} KB  {}",
                s.name,
                s.spec.resolution.to_string(),
                r.frames,
                r.bytes / 1024,
                r.path.display()
            );
        }
        println!(
            "{} rungs in {:.2}s ({:.1}x realtime)",
            reports.len(),
            elapsed.as_secs_f64(),
            info.duration_ms as f64 / 1000.0 / elapsed.as_secs_f64().max(0.001)
        );
        Ok(())
    }
}

/// Arguments for `verve verify`.
#[derive(Debug, Args)]
pub struct VerifyArgs {
    /// Directory of renditions to check against each other.
    pub dir: PathBuf,
}

/// `verve verify`. Exit 1 on a finding, so CI can branch on it.
pub fn verify(args: &VerifyArgs, json: bool) -> Result<()> {
    use verve_av::verify::Rendition;

    let mut files: Vec<PathBuf> = std::fs::read_dir(&args.dir)
        .with_context(|| format!("reading {}", args.dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "mp4" || e == "m4s"))
        .collect();
    if files.is_empty() {
        anyhow::bail!("no renditions in {}", args.dir.display());
    }
    files.sort();

    // Biggest first, so the reference is the rung most likely to be complete.
    let mut probed: Vec<(String, verve_av::MediaInfo)> = Vec::new();
    for path in &files {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        probed.push((name, probe_file(path)?));
    }
    probed.sort_by_key(|(_, i)| {
        std::cmp::Reverse(
            i.primary_video()
                .map_or(0, |v| u64::from(v.width) * u64::from(v.height)),
        )
    });

    let renditions: Vec<Rendition<'_>> = probed
        .iter()
        .map(|(n, i)| Rendition { name: n, info: i })
        .collect();
    let verdict = verve_av::verify::verify(&renditions);

    if json {
        println!("{}", serde_json::to_string_pretty(&verdict)?);
    } else if verdict.is_ok() {
        println!("{} renditions agree", verdict.checked);
    } else {
        for f in &verdict.findings {
            println!("  {:<8} {:?}", f.rendition, f.problem);
        }
        println!(
            "{} findings across {} renditions",
            verdict.findings.len(),
            verdict.checked
        );
    }

    if verdict.is_ok() {
        Ok(())
    } else {
        anyhow::bail!("verification failed")
    }
}
