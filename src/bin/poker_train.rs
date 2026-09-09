use std::collections::BTreeMap;
use std::error::Error;

use clap::Parser;
use terminal_poker::game::seat::SeatId;
use terminal_poker::training::{ArenaConfig, CheckCallPolicy, DealPlanV1, Policy, TrainingArena};

#[derive(Debug, Parser)]
#[command(about = "Run one deterministic projection-native training hand")]
struct Args {
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[arg(long, default_value_t = 100)]
    stack: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let mut arena = TrainingArena::new(
        ArenaConfig::heads_up(args.stack),
        DealPlanV1::seeded(args.seed),
    )?;
    let mut policies: BTreeMap<SeatId, Box<dyn Policy>> = BTreeMap::new();
    policies.insert(SeatId::new(0)?, Box::<CheckCallPolicy>::default());
    policies.insert(SeatId::new(1)?, Box::<CheckCallPolicy>::default());
    let episode = arena.run_to_terminal(&mut policies)?;

    println!("{}", serde_json::to_string_pretty(&episode.safe_history)?);
    Ok(())
}
