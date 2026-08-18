//! `verve`. Three groups: run the pipeline locally, verify output, operate the
//! system. Local pipeline commands need no cluster — that is deliberate.

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

    /// Confirm a destructive action without prompting.
    #[arg(long, global = true)]
    yes: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print MediaInfo as JSON — codecs, duration, keyframes, problems.
    Probe(pipeline::ProbeArgs),

    /// Make the cleaned-up copy: fixed frame rate, sane timestamps, rotation baked in.
    Mezzanine(pipeline::StubArgs),
    /// Print the split plan: where every cut lands and why.
    Split(pipeline::StubArgs),
    /// Run the ladder locally, decode-once.
    Encode(pipeline::StubArgs),
    /// CMAF + HLS + DASH.
    Package(pipeline::StubArgs),
    /// The whole pipeline end to end, locally.
    Run(pipeline::StubArgs),

    /// VMAF, PSNR, SSIM, MS-SSIM, CIEDE2000, CAMBI.
    Quality(pipeline::StubArgs),
    /// HLS + DASH + CMAF standards checks.
    Conform(pipeline::StubArgs),
    /// Our structural checks — frame counts, keyframe alignment, gaps.
    Verify(pipeline::StubArgs),
    /// Run everything over the corpus and write a report.
    Bench(pipeline::StubArgs),
    /// Diff two bench runs. The CI gate.
    Compare(pipeline::StubArgs),

    /// What this build links against and which codecs it can produce.
    Doctor,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let telemetry = verve_telemetry::init(verve_telemetry::Config::from_env("verve-cli"));
    let _guard = match telemetry {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("warning: telemetry disabled: {e}");
            None
        }
    };

    let result = match cli.command {
        Command::Probe(args) => pipeline::probe(&args, cli.json),
        Command::Doctor => pipeline::doctor(cli.json),
        Command::Mezzanine(a) => pipeline::not_yet("mezzanine", &a),
        Command::Split(a) => pipeline::not_yet("split", &a),
        Command::Encode(a) => pipeline::not_yet("encode", &a),
        Command::Package(a) => pipeline::not_yet("package", &a),
        Command::Run(a) => pipeline::not_yet("run", &a),
        Command::Quality(a) => pipeline::not_yet("quality", &a),
        Command::Conform(a) => pipeline::not_yet("conform", &a),
        Command::Verify(a) => pipeline::not_yet("verify", &a),
        Command::Bench(a) => pipeline::not_yet("bench", &a),
        Command::Compare(a) => pipeline::not_yet("compare", &a),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(EXIT_CHECK_FAILED)
        }
    }
}
