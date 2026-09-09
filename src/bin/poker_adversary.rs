use std::error::Error;
use std::io;

use clap::{Parser, ValueEnum};
use terminal_poker::training::{
    run_adversarial_evaluation, AdversarialEvaluationConfig, AdversarialPolicy,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PolicyArg {
    FoldCheck,
    CheckCall,
    PotPressure,
    Jam,
    EquityPotOdds,
    RandomLegal,
}

impl From<PolicyArg> for AdversarialPolicy {
    fn from(value: PolicyArg) -> Self {
        match value {
            PolicyArg::FoldCheck => Self::FoldCheck,
            PolicyArg::CheckCall => Self::CheckCall,
            PolicyArg::PotPressure => Self::PotPressure,
            PolicyArg::Jam => Self::Jam,
            PolicyArg::EquityPotOdds => Self::EquityPotOdds,
            PolicyArg::RandomLegal => Self::RandomLegal,
        }
    }
}

#[derive(Debug, Parser)]
#[command(
    about = "Compare projection-native policies over seeded heads-up and multiway ring hands"
)]
struct Args {
    /// Unique deals per independent table replica; each deal rotates the hero through every seat.
    #[arg(long, default_value_t = 100)]
    deals_per_table: u32,

    /// Comma-separated independent table replica counts.
    #[arg(long, value_delimiter = ',', default_value = "1,4")]
    tables: Vec<u16>,

    /// Comma-separated occupancies from 2 through 9.
    #[arg(long, value_delimiter = ',', default_value = "2,6,9")]
    seats: Vec<u8>,

    /// Comma-separated candidate policies.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "equity-pot-odds"
    )]
    heroes: Vec<PolicyArg>,

    /// Comma-separated homogeneous adversarial fields.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        default_value = "fold-check,check-call,pot-pressure,jam"
    )]
    opponents: Vec<PolicyArg>,

    #[arg(long, default_value_t = 100)]
    stack: u32,

    /// Uniform-range Monte Carlo samples for every equity-policy decision.
    #[arg(long, default_value_t = 64)]
    equity_samples: u32,

    #[arg(long, default_value_t = 1)]
    seed: u64,

    /// Emit compact rather than pretty JSON.
    #[arg(long)]
    compact: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let report = run_adversarial_evaluation(AdversarialEvaluationConfig {
        deals_per_table: args.deals_per_table,
        table_counts: args.tables,
        seat_counts: args.seats,
        hero_policies: args.heroes.into_iter().map(Into::into).collect(),
        opponent_policies: args.opponents.into_iter().map(Into::into).collect(),
        starting_stack: args.stack,
        equity_samples_per_decision: args.equity_samples,
        base_seed: args.seed,
    })?;
    let json = if args.compact {
        serde_json::to_string(&report)?
    } else {
        serde_json::to_string_pretty(&report)?
    };
    println!("{json}");
    if report.total_failures() > 0 {
        return Err(io::Error::other(format!(
            "evaluation completed with {} failed episodes",
            report.total_failures()
        ))
        .into());
    }
    Ok(())
}
