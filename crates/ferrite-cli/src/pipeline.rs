//! Local pipeline commands. No Temporal, no scheduler, no Postgres — you debug
//! an encode on a laptop with the code the fleet runs.

use anyhow::{Context as _, Result};
use clap::Args;
use std::path::PathBuf;

/// Arguments for `ferrite probe`.
#[derive(Debug, Args)]
pub struct ProbeArgs {
    /// The file to read.
    pub file: PathBuf,
}

/// Read a file, or say why this build cannot.
fn probe_file(path: &std::path::Path) -> Result<ferrite_av::MediaInfo> {
    #[cfg(feature = "ffmpeg")]
    {
        Ok(ferrite_av::probe(path)?)
    }
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = path;
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
}

/// `ferrite probe`.
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

/// `ferrite doctor`. What this build can actually do.
pub fn doctor(json: bool) -> Result<()> {
    use ferrite_av::{BackendRegistry, VideoCodecName};

    ferrite_av::init()?;
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
            "ffmpeg": ferrite_av::ffmpeg_version(),
            "has_ffmpeg": ferrite_av::has_ffmpeg(),
            "backends": registry.backends().iter().map(|b| b.id().as_str()).collect::<Vec<_>>(),
            "codecs": codecs.iter().map(|(c, ok)| (*c, *ok)).collect::<std::collections::BTreeMap<_, _>>(),
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    println!("ffmpeg      {}", ferrite_av::ffmpeg_version());
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
    if !ferrite_av::has_ffmpeg() {
        println!("\nthis build has no FFmpeg; local pipeline commands will refuse to run");
    }
    Ok(())
}

/// `ferrite split`.
pub fn split(args: &ProbeArgs, json: bool) -> Result<()> {
    let info = probe_file(&args.file)?;
    let plan = ferrite_av::split::plan(
        &info.keyframes_ms,
        info.duration_ms,
        ferrite_av::split::TARGET_CHUNK_MS,
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

/// `ferrite ladder`.
pub fn ladder(args: &ProbeArgs, json: bool) -> Result<()> {
    use ferrite_av::encoder::Preset;

    let info = probe_file(&args.file)?;
    let Some(video) = info.primary_video() else {
        anyhow::bail!("no video stream in {}", args.file.display());
    };
    let steps = ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, Preset::Medium);
    let fast = ferrite_av::ladder::fast_path(&steps).map(|s| s.name);

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

/// Arguments for `ferrite encode`.
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

/// `ferrite encode`.
pub fn encode(args: &EncodeArgs, json: bool) -> Result<()> {
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = (args, json);
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
    #[cfg(feature = "ffmpeg")]
    {
        use ferrite_av::encoder::Preset;
        use ferrite_av::transcode::Output;
        use std::time::Instant;

        let info = probe_file(&args.file)?;
        let Some(video) = info.primary_video() else {
            anyhow::bail!("no video stream in {}", args.file.display());
        };

        let preset = if args.fast {
            Preset::Veryfast
        } else {
            Preset::Medium
        };
        let mut steps = ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, preset);
        if args.fast {
            steps = ferrite_av::ladder::fast_path(&steps)
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
        let reports = ferrite_av::transcode::run(
            &args.file,
            &outputs,
            std::sync::Arc::new(ferrite_av::NeverCancel),
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

/// Arguments for `ferrite verify`.
#[derive(Debug, Args)]
pub struct VerifyArgs {
    /// Directory of renditions to check against each other.
    pub dir: PathBuf,
}

/// `ferrite verify`. Exit 1 on a finding, so CI can branch on it.
pub fn verify(args: &VerifyArgs, json: bool) -> Result<()> {
    use ferrite_av::verify::Rendition;

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
    let mut probed: Vec<(String, ferrite_av::MediaInfo)> = Vec::new();
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
    let verdict = ferrite_av::verify::verify(&renditions);

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

/// Arguments for `ferrite package`.
#[derive(Debug, Args)]
pub struct PackageArgs {
    /// Directory of encoded renditions.
    pub dir: PathBuf,
    /// Where to write segments and manifests.
    #[arg(short, long, default_value = "cmaf")]
    pub out: PathBuf,
    /// File to take the audio track from. Audio is never chunked per rung.
    #[arg(long)]
    pub audio: Option<PathBuf>,
}

/// `ferrite package`.
pub fn package(args: &PackageArgs, json: bool) -> Result<()> {
    use ferrite_av::package::{Input, Track};

    let mut files: Vec<PathBuf> = std::fs::read_dir(&args.dir)
        .with_context(|| format!("reading {}", args.dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "mp4"))
        .collect();
    if files.is_empty() {
        anyhow::bail!("no renditions in {}", args.dir.display());
    }
    files.sort();

    let mut inputs: Vec<Input> = files
        .iter()
        .map(|p| Input {
            name: p
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            path: p.clone(),
            kind: Track::Video,
            language: None,
        })
        .collect();

    if let Some(source) = &args.audio {
        // Normalise first: an MP3 soundtrack in an MP4 is something the
        // packager refuses outright, so passing source audio through fails.
        let track = args.out.join("audio.m4a");
        match ferrite_av::audio::encode(source, &track, &Default::default())? {
            Some(_) => inputs.push(Input {
                name: "audio".into(),
                path: track,
                kind: Track::Audio,
                language: None,
            }),
            None => eprintln!("note: {} has no audio track", source.display()),
        }
    }

    let packaged = ferrite_av::package::run(&inputs, &args.out)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&packaged)?);
    } else {
        println!("  hls   {}", packaged.hls.display());
        println!("  dash  {}", packaged.dash.display());
        println!("{} renditions packaged", packaged.renditions.len());
    }
    Ok(())
}

/// Arguments for `ferrite sheet`.
#[derive(Debug, Args)]
pub struct SheetArgs {
    /// The file to sample.
    pub file: PathBuf,
    /// Where to write the JPEG.
    #[arg(short, long, default_value = "contactsheet.jpg")]
    pub out: PathBuf,
}

/// `ferrite sheet`.
pub fn sheet(args: &SheetArgs, json: bool) -> Result<()> {
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = (args, json);
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
    #[cfg(feature = "ffmpeg")]
    {
        let sheet = ferrite_av::sheet::build(&args.file, &args.out)?;

        if json {
            println!("{}", serde_json::to_string_pretty(&sheet)?);
            return Ok(());
        }

        let bytes = std::fs::metadata(&sheet.path).map(|m| m.len()).unwrap_or(0);
        println!("{} ({} KB)", sheet.path.display(), bytes / 1024);
        println!("{} frames hashed", sheet.samples.len());
        for s in sheet.samples.iter().take(4) {
            println!(
                "  {:>3}  {:>8.3}s  {:016x}",
                s.index,
                s.time_ms as f64 / 1000.0,
                s.phash
            );
        }
        if sheet.samples.len() > 4 {
            println!("  ...");
        }
        Ok(())
    }
}

/// Arguments for `ferrite quality`.
#[derive(Debug, Args)]
pub struct QualityArgs {
    /// The reference. Use the mezzanine, not another encode.
    pub reference: PathBuf,
    /// The encode being judged.
    pub distorted: PathBuf,
    /// libvmaf model. `version=vmaf_4k_v0.6.1` for 4K sources.
    #[arg(long, default_value = ferrite_av::quality::DEFAULT_MODEL)]
    pub model: String,
    /// Score every Nth frame. Never quote a subsampled score as final.
    #[arg(long, default_value_t = 1)]
    pub subsample: u32,
    /// Fail if mean VMAF falls below this.
    #[arg(long)]
    pub min_vmaf: Option<f64>,
}

/// `ferrite quality`.
pub fn quality(args: &QualityArgs, json: bool) -> Result<()> {
    use ferrite_av::quality::{Options, measure};

    let options = Options {
        model: args.model.clone(),
        subsample: args.subsample,
        threads: 0,
    };
    let metrics = measure(&args.reference, &args.distorted, &options)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&metrics)?);
    } else {
        let row = |name: &str, p: Option<ferrite_av::quality::Pooled>| {
            if let Some(p) = p {
                println!("  {name:<12} {:>8.4}   worst {:>8.4}", p.mean, p.min);
            }
        };
        println!("{} frames", metrics.frames);
        row("VMAF", Some(metrics.vmaf));
        row("PSNR-Y", Some(metrics.psnr_y));
        row("SSIM", metrics.ssim);
        row("MS-SSIM", metrics.ms_ssim);
        row("CIEDE2000", metrics.ciede2000);
        row("CAMBI", metrics.cambi);
        if args.subsample > 1 {
            println!("subsampled 1/{} — not a final number", args.subsample);
        }
    }

    match args.min_vmaf {
        Some(threshold) if !metrics.passes(threshold) => {
            anyhow::bail!("VMAF {:.2} is below {threshold:.2}", metrics.vmaf.mean)
        }
        _ => Ok(()),
    }
}

/// Arguments for `ferrite bench`.
#[derive(Debug, Args)]
pub struct BenchArgs {
    /// Directory of corpus files.
    #[arg(default_value = "testdata/corpus")]
    pub corpus: PathBuf,
    /// Where to write the report.
    #[arg(short, long, default_value = "bench.json")]
    pub out: PathBuf,
    /// Where to put renditions. Deleted afterwards unless --keep.
    #[arg(long, default_value = "bench-work")]
    pub work: PathBuf,
    /// Score every Nth frame. Never quote a subsampled run as a release number.
    #[arg(long, default_value_t = 1)]
    pub subsample: u32,
    /// Keep the encoded renditions for inspection.
    #[arg(long)]
    pub keep: bool,
    /// Skip quality measurement. Structural checks only, much faster.
    #[arg(long)]
    pub no_quality: bool,
}

/// `ferrite bench`.
pub fn bench(args: &BenchArgs, json: bool) -> Result<()> {
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = (args, json);
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
    #[cfg(feature = "ffmpeg")]
    {
        use ferrite_av::bench::{Entry, Report, Rung, Scores};
        use ferrite_av::encoder::Preset;
        use ferrite_av::quality::Options;
        use ferrite_av::transcode::Output;
        use ferrite_av::verify::Rendition;
        use std::time::Instant;

        let preset = Preset::Medium;
        let mut files: Vec<PathBuf> = std::fs::read_dir(&args.corpus)
            .with_context(|| format!("reading {}", args.corpus.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && !p
                        .file_name()
                        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
            })
            .collect();
        if files.is_empty() {
            anyhow::bail!("no corpus files in {}", args.corpus.display());
        }
        files.sort();

        let mut report = Report::new(
            ferrite_av::ffmpeg_version(),
            preset.as_str().to_string(),
            args.subsample,
        );

        for file in &files {
            let name = file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if !json {
                println!("{name}");
            }

            let info = match probe_file(file) {
                Ok(info) => info,
                Err(e) => {
                    // A corpus of awkward files must produce a clear error,
                    // never a hang and never a silent skip.
                    report.entries.push(Entry {
                        file: name,
                        duration_ms: 0,
                        warnings: Vec::new(),
                        rungs: Vec::new(),
                        findings: vec![format!("unreadable: {e}")],
                        encode_seconds: 0.0,
                    });
                    continue;
                }
            };

            let Some(video) = info.primary_video() else {
                report.entries.push(Entry {
                    file: name,
                    duration_ms: info.duration_ms,
                    warnings: info.warnings.iter().map(ToString::to_string).collect(),
                    rungs: Vec::new(),
                    findings: vec!["no video stream".into()],
                    encode_seconds: 0.0,
                });
                continue;
            };

            let work = args.work.join(name.replace('.', "_"));
            let steps = ferrite_av::ladder::plan(video, &ferrite_av::ladder::STANDARD, preset);
            let outputs: Vec<Output> = steps
                .iter()
                .map(|s| Output {
                    path: work.join(format!("{}.mp4", s.name)),
                    spec: s.spec.clone(),
                })
                .collect();

            let started = Instant::now();
            let reports = ferrite_av::transcode::run(
                file,
                &outputs,
                std::sync::Arc::new(ferrite_av::NeverCancel),
            )?;
            let encode_seconds = started.elapsed().as_secs_f64();

            let probed: Vec<ferrite_av::MediaInfo> = reports
                .iter()
                .map(|r| probe_file(&r.path))
                .collect::<Result<_>>()?;
            let renditions: Vec<Rendition<'_>> = steps
                .iter()
                .zip(&probed)
                .map(|(s, i)| Rendition {
                    name: s.name,
                    info: i,
                })
                .collect();
            let verdict = ferrite_av::verify::verify(&renditions);

            let mut rungs = Vec::with_capacity(steps.len());
            for (step, encoded) in steps.iter().zip(&reports) {
                let scores = if args.no_quality {
                    None
                } else {
                    let options = Options {
                        subsample: args.subsample,
                        ..Options::default()
                    };
                    ferrite_av::quality::measure(file, &encoded.path, &options)
                        .ok()
                        .map(|m| Scores::from_metrics(&m))
                };
                rungs.push(Rung {
                    name: step.name.to_string(),
                    width: step.spec.resolution.width,
                    height: step.spec.resolution.height,
                    frames: encoded.frames,
                    bytes: encoded.bytes,
                    scores,
                });
            }

            report.entries.push(Entry {
                file: name,
                duration_ms: info.duration_ms,
                warnings: info.warnings.iter().map(ToString::to_string).collect(),
                rungs,
                findings: verdict
                    .findings
                    .iter()
                    .map(|f| format!("{} {:?}", f.rendition, f.problem))
                    .collect(),
                encode_seconds,
            });

            if !args.keep {
                let _ = std::fs::remove_dir_all(&work);
            }
        }

        if !args.keep {
            let _ = std::fs::remove_dir_all(&args.work);
        }
        std::fs::write(&args.out, serde_json::to_string_pretty(&report)?)?;

        let findings = report.findings();
        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            for entry in &report.entries {
                for rung in &entry.rungs {
                    let vmaf = rung
                        .scores
                        .map_or("—".to_string(), |s| format!("{:.2}", s.vmaf));
                    println!(
                        "  {:<20} {:<6} {:>5}x{:<5} {:>7} KB  VMAF {vmaf}",
                        entry.file,
                        rung.name,
                        rung.width,
                        rung.height,
                        rung.bytes / 1024
                    );
                }
            }
            println!("{} files → {}", report.entries.len(), args.out.display());
        }

        if findings.is_empty() {
            Ok(())
        } else {
            for f in &findings {
                eprintln!("  {f}");
            }
            anyhow::bail!("{} structural findings", findings.len())
        }
    }
}

/// Arguments for `ferrite compare`.
#[derive(Debug, Args)]
pub struct CompareArgs {
    /// The baseline report.
    pub before: PathBuf,
    /// The new report.
    pub after: PathBuf,
    /// Allowed VMAF movement before it counts as a regression.
    #[arg(long)]
    pub vmaf_tolerance: Option<f64>,
}

/// `ferrite compare`. Exit 1 on a regression — this diff is the CI gate.
pub fn compare(args: &CompareArgs, json: bool) -> Result<()> {
    use ferrite_av::bench::{Report, Tolerance};

    let read = |path: &PathBuf| -> Result<Report> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    };

    let before = read(&args.before)?;
    let after = read(&args.after)?;

    let mut tolerance = Tolerance::default();
    if let Some(v) = args.vmaf_tolerance {
        tolerance.vmaf = v;
    }
    let diff = ferrite_av::bench::compare(&before, &after, &tolerance);

    if json {
        println!("{}", serde_json::to_string_pretty(&diff)?);
    } else {
        if before.preset != after.preset || before.ffmpeg != after.ffmpeg {
            println!(
                "settings changed: {} {} → {} {}",
                before.preset, before.ffmpeg, after.preset, after.ffmpeg
            );
        }
        for r in &diff.regressions {
            println!(
                "  {:<20} {:<6} {:<10} {:.4} → {:.4}  ({:+.4})",
                r.file, r.rung, r.metric, r.before, r.after, r.delta
            );
        }
        for m in &diff.missing {
            println!("  missing  {m}");
        }
        for a in &diff.added {
            println!("  added    {a}");
        }
        if diff.is_clean() {
            println!("no regressions");
        }
    }

    if diff.is_clean() {
        Ok(())
    } else {
        anyhow::bail!(
            "{} regressions, {} missing",
            diff.regressions.len(),
            diff.missing.len()
        )
    }
}

/// Arguments for `ferrite job`.
#[derive(Debug, Args)]
pub struct JobArgs {
    /// The file to convert.
    pub file: PathBuf,
    /// Where to write the single output.
    #[arg(short, long, default_value = "output.mp4")]
    pub out: PathBuf,
    /// Target height. Never upscales past the source.
    #[arg(long)]
    pub height: Option<u32>,
    /// Quality target, 0–51. Lower is better.
    #[arg(long, default_value_t = 23)]
    pub crf: u8,
}

/// `ferrite job`.
pub fn job(args: &JobArgs, json: bool) -> Result<()> {
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = (args, json);
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
    #[cfg(feature = "ffmpeg")]
    {
        use ferrite_worker::job::{self, Request};

        let request = Request {
            height: args.height,
            crf: args.crf,
            ..Request::default()
        };
        let done = job::run(&args.file, &args.out, &request)?;

        if json {
            println!("{}", serde_json::to_string_pretty(&done)?);
            return Ok(());
        }

        println!("  output      {}", done.path.display());
        println!("  size        {}x{}", done.width, done.height);
        println!("  frames      {}", done.frames);
        println!("  bytes       {} KB", done.bytes / 1024);
        println!(
            "  mezzanine   {}",
            if done.mezzanine {
                "required"
            } else {
                "not needed"
            }
        );
        println!("  sheet       {}", done.contact_sheet.display());
        println!(
            "  hashes      {} frames sampled for the blocklist",
            done.hashes.len()
        );
        Ok(())
    }
}

/// Arguments for `ferrite run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// The source to publish.
    pub file: PathBuf,
    /// Where the playable asset goes.
    #[arg(short, long, default_value = "asset")]
    pub out: PathBuf,
    /// Encode only the rung that makes it playable.
    #[arg(long)]
    pub fast: bool,
}

/// `ferrite run`.
pub fn run(args: &RunArgs, json: bool) -> Result<()> {
    #[cfg(not(feature = "ffmpeg"))]
    {
        let _ = (args, json);
        anyhow::bail!("this build has no FFmpeg; rebuild with --features ffmpeg")
    }
    #[cfg(feature = "ffmpeg")]
    {
        use ferrite_av::encoder::Preset;
        use ferrite_worker::asset::{self, Request};
        use std::time::Instant;

        let request = Request {
            preset: if args.fast {
                Preset::Veryfast
            } else {
                Preset::Medium
            },
            fast_only: args.fast,
        };

        let started = Instant::now();
        let published = asset::run(&args.file, &args.out, &request)?;
        let elapsed = started.elapsed();

        if json {
            println!("{}", serde_json::to_string_pretty(&published)?);
            return Ok(());
        }

        for r in &published.renditions {
            println!(
                "  {:<6} {:>5}x{:<5} {:>7} frames  {:>8} KB",
                r.name,
                r.width,
                r.height,
                r.frames,
                r.bytes / 1024
            );
        }
        println!(
            "  audio  {}",
            if published.audio {
                "aac stereo"
            } else {
                "none"
            }
        );
        println!("  sheet  {}", published.contact_sheet.display());
        println!("  thumbs {}", published.thumbnails);
        println!("  hls    {}", published.hls.display());
        println!("  dash   {}", published.dash.display());
        println!(
            "{:.2}s for {:.1}s of video ({:.1}x realtime)",
            elapsed.as_secs_f64(),
            published.duration_ms as f64 / 1000.0,
            published.duration_ms as f64 / 1000.0 / elapsed.as_secs_f64().max(0.001)
        );
        Ok(())
    }
}

/// Arguments for `ferrite conform`.
#[derive(Debug, Args)]
pub struct ConformArgs {
    /// The manifest, as the validator will fetch it. Must be reachable from
    /// inside the validator container — see scripts/dashif.sh.
    pub manifest_url: String,
    /// Where the validator listens.
    #[arg(long, default_value = ferrite_av::conform::DEFAULT_ENDPOINT)]
    pub endpoint: String,
    /// Also read every box. Much slower, and where join bugs show up.
    #[arg(long)]
    pub segments: bool,
}

/// `ferrite conform`. Exit 1 on anything that breaks the spec.
pub fn conform(args: &ConformArgs, json: bool) -> Result<()> {
    use ferrite_av::conform::{self, Severity};

    let verdict = conform::check(&args.endpoint, &args.manifest_url, args.segments)?;

    if json {
        println!("{}", serde_json::to_string_pretty(&verdict)?);
    } else {
        for f in &verdict.findings {
            let mark = match f.severity {
                Severity::Error => "error",
                Severity::Warning => "warn ",
                Severity::Info => "info ",
            };
            println!("  {mark}  {:<40} {}", f.section, f.message);
        }
        let errors = verdict.at_least(Severity::Error).count();
        println!(
            "{} — {} checks, {errors} errors, {} findings",
            if verdict.is_conformant() {
                "conformant"
            } else {
                "NOT conformant"
            },
            verdict.tests_run,
            verdict.findings.len()
        );
        if verdict.tests_run == 0 {
            println!("the validator ran no checks — it could not reach the manifest");
        }
    }

    if verdict.is_conformant() {
        Ok(())
    } else {
        anyhow::bail!("{} is not conformant", args.manifest_url)
    }
}
