use std::error::Error;
use std::io;

use clap::{Parser, ValueEnum};
use terminal_poker::training::{
    default_worker_counts, run_benchmark, BenchmarkConfig, BenchmarkPolicy, BenchmarkRecording,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PolicySelection {
    CheckCall,
    RandomLegal,
    Both,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RecordingSelection {
    Minimal,
    FullJson,
    Both,
}

#[derive(Debug, Parser)]
#[command(about = "Benchmark deterministic poker-policy environment throughput")]
struct Args {
    /// Hands executed for every policy/recording/worker case.
    #[arg(long, default_value_t = 100_000)]
    hands_per_case: u64,

    /// Comma-separated worker counts, or "auto" for powers of two up to 16.
    #[arg(long, default_value = "auto")]
    workers: String,

    #[arg(long, value_enum, default_value_t = PolicySelection::Both)]
    policy: PolicySelection,

    #[arg(long, value_enum, default_value_t = RecordingSelection::Both)]
    recording: RecordingSelection,

    #[arg(long, default_value_t = 100)]
    stack: u32,

    #[arg(long, default_value_t = 1)]
    seed: u64,

    /// Emit compact rather than pretty JSON.
    #[arg(long)]
    compact: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let config = BenchmarkConfig {
        hands_per_case: args.hands_per_case,
        worker_counts: parse_workers(&args.workers)?,
        policies: match args.policy {
            PolicySelection::CheckCall => vec![BenchmarkPolicy::CheckCall],
            PolicySelection::RandomLegal => vec![BenchmarkPolicy::RandomLegal],
            PolicySelection::Both => {
                vec![BenchmarkPolicy::CheckCall, BenchmarkPolicy::RandomLegal]
            }
        },
        recordings: match args.recording {
            RecordingSelection::Minimal => vec![BenchmarkRecording::Minimal],
            RecordingSelection::FullJson => vec![BenchmarkRecording::FullJson],
            RecordingSelection::Both => {
                vec![BenchmarkRecording::Minimal, BenchmarkRecording::FullJson]
            }
        },
        starting_stack: args.stack,
        base_seed: args.seed,
    };
    let report = run_benchmark(config)?;
    let json = if args.compact {
        serde_json::to_string(&report)?
    } else {
        serde_json::to_string_pretty(&report)?
    };
    println!("{json}");
    if report.total_failures() > 0 {
        return Err(io::Error::other(format!(
            "benchmark completed with {} failed hands",
            report.total_failures()
        ))
        .into());
    }
    Ok(())
}

fn parse_workers(value: &str) -> Result<Vec<usize>, Box<dyn Error>> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(default_worker_counts());
    }
    let counts = value
        .split(',')
        .map(str::trim)
        .map(|entry| entry.parse::<usize>())
        .collect::<Result<Vec<_>, _>>()?;
    Ok(counts)
}
