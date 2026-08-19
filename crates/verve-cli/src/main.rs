//! `verve`. Local pipeline commands need no cluster — no Temporal, no
//! scheduler, no Postgres. You debug an encode on a laptop with fleet code.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

mod pipeline;

/// `0` pass, `1` a check failed, `2` bad usage. So CI can branch on it.
const EXIT_CHECK_FAILED: u8 = 1;

#[derive(Debug, Parser)]
#[command(name = "verve", version, about = "verve — video pipeline")]
struct Cli {
    /// Machine-readable output. Human-readable is the default.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print MediaInfo as JSON — codecs, duration, keyframes, problems.
    Probe(pipeline::ProbeArgs),
    /// Print the split plan: where every cut lands and why.
    Split(pipeline::ProbeArgs),
    /// Print the ladder: which rungs this source gets, and why.
    Ladder(pipeline::ProbeArgs),
    /// Run the ladder locally, decode-once.
    Encode(pipeline::EncodeArgs),
    /// Structural checks — frame counts, keyframe alignment, duration.
    Verify(pipeline::VerifyArgs),
    /// CMAF + HLS + DASH over one segment set.
    Package(pipeline::PackageArgs),
    /// Contact sheet plus a perceptual hash per sampled frame.
    Sheet(pipeline::SheetArgs),
    /// VMAF, PSNR, SSIM, MS-SSIM, CIEDE2000, CAMBI.
    Quality(pipeline::QualityArgs),
    /// Run the corpus through the pipeline and write a report.
    Bench(pipeline::BenchArgs),
    /// Diff two bench reports. This diff is the CI gate.
    Compare(pipeline::CompareArgs),
    /// What this build links against and which codecs it can produce.
    Doctor,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let _guard = match verve_telemetry::init(verve_telemetry::Config::from_env("verve-cli")) {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("warning: telemetry disabled: {e}");
            None
        }
    };

    let result = match cli.command {
        Command::Probe(args) => pipeline::probe(&args, cli.json),
        Command::Split(args) => pipeline::split(&args, cli.json),
        Command::Ladder(args) => pipeline::ladder(&args, cli.json),
        Command::Encode(args) => pipeline::encode(&args, cli.json),
        Command::Verify(args) => pipeline::verify(&args, cli.json),
        Command::Package(args) => pipeline::package(&args, cli.json),
        Command::Sheet(args) => pipeline::sheet(&args, cli.json),
        Command::Quality(args) => pipeline::quality(&args, cli.json),
        Command::Bench(args) => pipeline::bench(&args, cli.json),
        Command::Compare(args) => pipeline::compare(&args, cli.json),
        Command::Doctor => pipeline::doctor(cli.json),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(EXIT_CHECK_FAILED)
        }
    }
}
