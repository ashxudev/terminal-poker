//! Seeded adversarial Monte Carlo evaluation over independent ring hands.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::game::multiway::MultiwayPhase;
use crate::game::seat::{SeatId, TableSize, MAX_TABLE_SEATS, MIN_TABLE_SEATS};
use crate::game::state::BIG_BLIND;
use crate::protocol::{HandId, TableId};

use super::arena::{ArenaConfig, ArenaRecordingMode, TrainingArena};
use super::deal::DealPlanV1;
use super::policy::{
    CheckCallPolicy, EquityPotOddsPolicy, FoldCheckPolicy, JamPolicy, Policy, PotPressurePolicy,
    RandomLegalPolicy,
};

pub const ADVERSARIAL_REPORT_VERSION: u16 = 1;
pub const MAX_EVALUATION_DEALS_PER_TABLE: u32 = 1_000_000;
pub const MAX_EVALUATION_TABLES: u16 = 1_024;
pub const MAX_EQUITY_SAMPLES: u32 = 100_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdversarialPolicy {
    FoldCheck,
    CheckCall,
    PotPressure,
    Jam,
    EquityPotOdds,
    RandomLegal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdversarialEvaluationConfig {
    /// Unique deal seeds evaluated at every hero seat on each table replica.
    pub deals_per_table: u32,
    pub table_counts: Vec<u16>,
    pub seat_counts: Vec<u8>,
    pub hero_policies: Vec<AdversarialPolicy>,
    pub opponent_policies: Vec<AdversarialPolicy>,
    pub starting_stack: u32,
    pub equity_samples_per_decision: u32,
    pub base_seed: u64,
}

impl Default for AdversarialEvaluationConfig {
    fn default() -> Self {
        Self {
            deals_per_table: 100,
            table_counts: vec![1, 4],
            seat_counts: vec![2, 6, 9],
            hero_policies: vec![AdversarialPolicy::EquityPotOdds],
            opponent_policies: vec![
                AdversarialPolicy::FoldCheck,
                AdversarialPolicy::CheckCall,
                AdversarialPolicy::PotPressure,
                AdversarialPolicy::Jam,
            ],
            starting_stack: 100,
            equity_samples_per_decision: 64,
            base_seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdversarialCase {
    pub hero_policy: AdversarialPolicy,
    pub opponent_policy: AdversarialPolicy,
    pub seats_per_table: u8,
    pub table_replicas: u16,
    pub distinct_deals_per_table: u32,
    /// Independent deal/table blocks used for the confidence interval.
    pub independent_deal_blocks: u64,
    pub paired_seat_rotation_episodes: u64,
    pub completed_episodes: u64,
    pub failed_episodes: u64,
    pub accepted_actions: u64,
    pub showdown_episodes: u64,
    pub hero_positive_episodes: u64,
    pub hero_break_even_episodes: u64,
    pub hero_bust_episodes: u64,
    pub hero_total_chip_delta: i64,
    pub hero_mean_chip_delta: f64,
    pub hero_bb_per_100: f64,
    pub hero_bb_per_100_ci95_low: f64,
    pub hero_bb_per_100_ci95_high: f64,
    pub elapsed_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TournamentCapability {
    pub available: bool,
    pub reason: String,
    pub required_features: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdversarialEvaluationReport {
    pub version: u16,
    pub config: AdversarialEvaluationConfig,
    pub cases: Vec<AdversarialCase>,
    pub execution_model: String,
    pub tournament: TournamentCapability,
    pub limitations: Vec<String>,
}

impl AdversarialEvaluationReport {
    pub fn total_failures(&self) -> u64 {
        self.cases.iter().map(|case| case.failed_episodes).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdversarialEvaluationError {
    InvalidDealCount(u32),
    InvalidTableCount(u16),
    DuplicateTableCount(u16),
    InvalidSeatCount(u8),
    DuplicateSeatCount(u8),
    MissingHeroPolicies,
    MissingOpponentPolicies,
    DuplicateHeroPolicy(AdversarialPolicy),
    DuplicateOpponentPolicy(AdversarialPolicy),
    InvalidStartingStack,
    InvalidEquitySamples(u32),
}

impl Display for AdversarialEvaluationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDealCount(count) => write!(
                formatter,
                "deals per table must be between 1 and {MAX_EVALUATION_DEALS_PER_TABLE}, found {count}"
            ),
            Self::InvalidTableCount(count) => write!(
                formatter,
                "table count must be between 1 and {MAX_EVALUATION_TABLES}, found {count}"
            ),
            Self::DuplicateTableCount(count) => write!(formatter, "table count {count} is duplicated"),
            Self::InvalidSeatCount(count) => write!(
                formatter,
                "seat count must be between {MIN_TABLE_SEATS} and {MAX_TABLE_SEATS}, found {count}"
            ),
            Self::DuplicateSeatCount(count) => write!(formatter, "seat count {count} is duplicated"),
            Self::MissingHeroPolicies => write!(formatter, "at least one hero policy is required"),
            Self::MissingOpponentPolicies => {
                write!(formatter, "at least one opponent policy is required")
            }
            Self::DuplicateHeroPolicy(policy) => {
                write!(formatter, "hero policy {policy:?} is duplicated")
            }
            Self::DuplicateOpponentPolicy(policy) => {
                write!(formatter, "opponent policy {policy:?} is duplicated")
            }
            Self::InvalidStartingStack => write!(formatter, "starting stack must be positive"),
            Self::InvalidEquitySamples(samples) => write!(
                formatter,
                "equity samples must be between 1 and {MAX_EQUITY_SAMPLES}, found {samples}"
            ),
        }
    }
}

impl Error for AdversarialEvaluationError {}

#[derive(Debug, Default)]
struct RunningStats {
    count: u64,
    mean: f64,
    squared_deviation_sum: f64,
}

impl RunningStats {
    fn push(&mut self, value: f64) {
        self.count += 1;
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta_after = value - self.mean;
        self.squared_deviation_sum += delta * delta_after;
    }

    fn standard_error(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let variance = self.squared_deviation_sum / (self.count - 1) as f64;
        variance.sqrt() / (self.count as f64).sqrt()
    }
}

pub fn run_adversarial_evaluation(
    config: AdversarialEvaluationConfig,
) -> Result<AdversarialEvaluationReport, AdversarialEvaluationError> {
    validate_config(&config)?;
    let mut cases = Vec::new();
    for &hero in &config.hero_policies {
        for &opponent in &config.opponent_policies {
            for &seats in &config.seat_counts {
                for &tables in &config.table_counts {
                    cases.push(run_case(&config, hero, opponent, seats, tables));
                }
            }
        }
    }

    Ok(AdversarialEvaluationReport {
        version: ADVERSARIAL_REPORT_VERSION,
        config,
        cases,
        execution_model: "independent authoritative ring hands; table replicas execute sequentially with disjoint deterministic seed streams".to_string(),
        tournament: TournamentCapability {
            available: false,
            reason: "the current build has no tournament controller; ring-hand outcomes are not tournament standings".to_string(),
            required_features: vec![
                "blind-level schedule".to_string(),
                "stack carry-over and elimination ordering".to_string(),
                "table balancing and breaking".to_string(),
                "final-table consolidation and winner declaration".to_string(),
            ],
        },
        limitations: vec![
            "opponents are homogeneous within each case".to_string(),
            "each episode starts from equal stacks; this is not a multi-hand ring session".to_string(),
            "equity policy assumes uniformly random legal opponent holdings".to_string(),
            "95% intervals are normal approximations over independent deal/table blocks after averaging each paired seat rotation".to_string(),
        ],
    })
}

fn validate_config(config: &AdversarialEvaluationConfig) -> Result<(), AdversarialEvaluationError> {
    if config.deals_per_table == 0 || config.deals_per_table > MAX_EVALUATION_DEALS_PER_TABLE {
        return Err(AdversarialEvaluationError::InvalidDealCount(
            config.deals_per_table,
        ));
    }
    if config.starting_stack == 0 {
        return Err(AdversarialEvaluationError::InvalidStartingStack);
    }
    if config.equity_samples_per_decision == 0
        || config.equity_samples_per_decision > MAX_EQUITY_SAMPLES
    {
        return Err(AdversarialEvaluationError::InvalidEquitySamples(
            config.equity_samples_per_decision,
        ));
    }
    if config.hero_policies.is_empty() {
        return Err(AdversarialEvaluationError::MissingHeroPolicies);
    }
    if config.opponent_policies.is_empty() {
        return Err(AdversarialEvaluationError::MissingOpponentPolicies);
    }

    reject_duplicates(&config.table_counts, |count| {
        AdversarialEvaluationError::DuplicateTableCount(*count)
    })?;
    for &count in &config.table_counts {
        if count == 0 || count > MAX_EVALUATION_TABLES {
            return Err(AdversarialEvaluationError::InvalidTableCount(count));
        }
    }
    reject_duplicates(&config.seat_counts, |count| {
        AdversarialEvaluationError::DuplicateSeatCount(*count)
    })?;
    for &count in &config.seat_counts {
        if !(MIN_TABLE_SEATS..=MAX_TABLE_SEATS).contains(&count) {
            return Err(AdversarialEvaluationError::InvalidSeatCount(count));
        }
    }
    reject_duplicates(&config.hero_policies, |policy| {
        AdversarialEvaluationError::DuplicateHeroPolicy(*policy)
    })?;
    reject_duplicates(&config.opponent_policies, |policy| {
        AdversarialEvaluationError::DuplicateOpponentPolicy(*policy)
    })?;
    Ok(())
}

fn reject_duplicates<T, F>(values: &[T], duplicate: F) -> Result<(), AdversarialEvaluationError>
where
    T: Copy + Ord,
    F: Fn(&T) -> AdversarialEvaluationError,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(*value) {
            return Err(duplicate(value));
        }
    }
    Ok(())
}

fn run_case(
    config: &AdversarialEvaluationConfig,
    hero_policy: AdversarialPolicy,
    opponent_policy: AdversarialPolicy,
    seats_per_table: u8,
    table_replicas: u16,
) -> AdversarialCase {
    let started = Instant::now();
    let requested =
        u64::from(table_replicas) * u64::from(config.deals_per_table) * u64::from(seats_per_table);
    let mut completed = 0;
    let mut failed = 0;
    let mut accepted_actions = 0;
    let mut showdowns = 0;
    let mut positive = 0;
    let mut break_even = 0;
    let mut busts = 0;
    let mut total_delta = 0i64;
    let mut stats = RunningStats::default();
    let mut first_error = None;

    for table_index in 0..table_replicas {
        for deal_index in 0..config.deals_per_table {
            let deal_seed = derive_seed(config.base_seed, table_index, deal_index, 0, 0);
            let button = deal_index % u32::from(seats_per_table);
            let mut block_delta = 0i64;
            let mut block_completed = 0u8;
            for hero_seat_index in 0..seats_per_table {
                match run_episode(
                    config,
                    hero_policy,
                    opponent_policy,
                    seats_per_table,
                    table_index,
                    deal_index,
                    hero_seat_index,
                    button as u8,
                    deal_seed,
                ) {
                    Ok((delta, actions, terminal_phase)) => {
                        completed += 1;
                        accepted_actions += actions;
                        showdowns += u64::from(terminal_phase == MultiwayPhase::Showdown);
                        positive += u64::from(delta > 0);
                        break_even += u64::from(delta == 0);
                        busts += u64::from(delta <= -i64::from(config.starting_stack));
                        total_delta += delta;
                        block_delta += delta;
                        block_completed += 1;
                    }
                    Err(error) => {
                        failed += 1;
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                }
            }
            if block_completed == seats_per_table {
                stats.push(block_delta as f64 / f64::from(seats_per_table));
            }
        }
    }

    let bb100_scale = 100.0 / f64::from(BIG_BLIND);
    let mean_delta = if completed == 0 {
        0.0
    } else {
        total_delta as f64 / completed as f64
    };
    let bb_per_100 = mean_delta * bb100_scale;
    let margin = 1.96 * stats.standard_error() * bb100_scale;
    AdversarialCase {
        hero_policy,
        opponent_policy,
        seats_per_table,
        table_replicas,
        distinct_deals_per_table: config.deals_per_table,
        independent_deal_blocks: stats.count,
        paired_seat_rotation_episodes: requested,
        completed_episodes: completed,
        failed_episodes: failed,
        accepted_actions,
        showdown_episodes: showdowns,
        hero_positive_episodes: positive,
        hero_break_even_episodes: break_even,
        hero_bust_episodes: busts,
        hero_total_chip_delta: total_delta,
        hero_mean_chip_delta: mean_delta,
        hero_bb_per_100: bb_per_100,
        hero_bb_per_100_ci95_low: bb_per_100 - margin,
        hero_bb_per_100_ci95_high: bb_per_100 + margin,
        elapsed_seconds: started.elapsed().as_secs_f64(),
        first_error,
    }
}

#[allow(clippy::too_many_arguments)]
fn run_episode(
    config: &AdversarialEvaluationConfig,
    hero_policy: AdversarialPolicy,
    opponent_policy: AdversarialPolicy,
    seats_per_table: u8,
    table_index: u16,
    deal_index: u32,
    hero_seat_index: u8,
    button_index: u8,
    deal_seed: u64,
) -> Result<(i64, u64, MultiwayPhase), String> {
    let table_size = TableSize::new(seats_per_table).map_err(|error| error.to_string())?;
    let hero_seat = SeatId::new(hero_seat_index).map_err(|error| error.to_string())?;
    let button = SeatId::new(button_index).map_err(|error| error.to_string())?;
    let stacks = table_size
        .seats()
        .map(|seat| (seat, config.starting_stack))
        .collect();
    let mut arena = TrainingArena::new(
        ArenaConfig {
            table_id: TableId(u64::from(table_index) + 1),
            hand_id: HandId(u64::from(deal_index) + 1),
            table_size,
            button,
            stacks,
            forced_posts: Vec::new(),
            max_actions: crate::ring_history::MAX_HISTORY_ACTIONS,
            recording: ArenaRecordingMode::Minimal,
        },
        DealPlanV1::seeded(deal_seed),
    )
    .map_err(|error| error.to_string())?;

    let mut policies = BTreeMap::new();
    for seat in table_size.seats() {
        let kind = if seat == hero_seat {
            hero_policy
        } else {
            opponent_policy
        };
        let policy_seed = derive_seed(
            config.base_seed ^ policy_discriminator(kind),
            table_index,
            deal_index,
            hero_seat_index,
            seat.as_u8(),
        );
        policies.insert(
            seat,
            make_policy(kind, policy_seed, config.equity_samples_per_decision),
        );
    }
    let summary = arena
        .run_to_terminal_summary(&mut policies)
        .map_err(|error| error.to_string())?;
    let hero_delta = summary
        .chip_deltas
        .iter()
        .find_map(|(seat, delta)| (*seat == hero_seat).then_some(*delta))
        .ok_or_else(|| "terminal summary omitted the hero seat".to_string())?;
    Ok((
        hero_delta,
        summary.accepted_actions as u64,
        summary.terminal_phase,
    ))
}

fn make_policy(kind: AdversarialPolicy, seed: u64, equity_samples: u32) -> Box<dyn Policy> {
    match kind {
        AdversarialPolicy::FoldCheck => Box::<FoldCheckPolicy>::default(),
        AdversarialPolicy::CheckCall => Box::<CheckCallPolicy>::default(),
        AdversarialPolicy::PotPressure => Box::<PotPressurePolicy>::default(),
        AdversarialPolicy::Jam => Box::<JamPolicy>::default(),
        AdversarialPolicy::EquityPotOdds => {
            Box::new(EquityPotOddsPolicy::seeded(seed, equity_samples))
        }
        AdversarialPolicy::RandomLegal => Box::new(RandomLegalPolicy::seeded(seed)),
    }
}

fn policy_discriminator(kind: AdversarialPolicy) -> u64 {
    match kind {
        AdversarialPolicy::FoldCheck => 0x11,
        AdversarialPolicy::CheckCall => 0x22,
        AdversarialPolicy::PotPressure => 0x33,
        AdversarialPolicy::Jam => 0x44,
        AdversarialPolicy::EquityPotOdds => 0x55,
        AdversarialPolicy::RandomLegal => 0x66,
    }
}

fn derive_seed(
    base: u64,
    table_index: u16,
    deal_index: u32,
    hero_seat: u8,
    policy_seat: u8,
) -> u64 {
    let mut value = base ^ (u64::from(table_index) << 48);
    value ^= u64::from(deal_index).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    value ^= u64::from(hero_seat).wrapping_mul(0xD1B5_4A32_D192_ED03);
    value ^= u64::from(policy_seat).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^= value >> 30;
    value = value.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_covers_heads_up_six_max_nine_max_and_table_replicas() {
        let report = run_adversarial_evaluation(AdversarialEvaluationConfig {
            deals_per_table: 2,
            table_counts: vec![1, 2],
            seat_counts: vec![2, 6, 9],
            hero_policies: vec![AdversarialPolicy::EquityPotOdds],
            opponent_policies: vec![
                AdversarialPolicy::FoldCheck,
                AdversarialPolicy::CheckCall,
                AdversarialPolicy::PotPressure,
                AdversarialPolicy::Jam,
                AdversarialPolicy::RandomLegal,
            ],
            starting_stack: 100,
            equity_samples_per_decision: 4,
            base_seed: 73,
        })
        .unwrap();

        assert_eq!(report.cases.len(), 30);
        assert_eq!(report.total_failures(), 0);
        assert!(!report.tournament.available);
        for case in report.cases {
            let expected = u64::from(case.table_replicas)
                * u64::from(case.distinct_deals_per_table)
                * u64::from(case.seats_per_table);
            assert_eq!(case.paired_seat_rotation_episodes, expected);
            assert_eq!(case.completed_episodes, expected);
            assert!(
                case.hero_positive_episodes + case.hero_break_even_episodes
                    <= case.completed_episodes
            );
            assert!(case.hero_bb_per_100.is_finite());
        }
    }

    #[test]
    fn evaluation_is_seed_deterministic_apart_from_timing() {
        let config = AdversarialEvaluationConfig {
            deals_per_table: 3,
            table_counts: vec![1],
            seat_counts: vec![2],
            hero_policies: vec![AdversarialPolicy::EquityPotOdds],
            opponent_policies: vec![AdversarialPolicy::Jam],
            starting_stack: 100,
            equity_samples_per_decision: 8,
            base_seed: 91,
        };
        let first = run_adversarial_evaluation(config.clone()).unwrap();
        let second = run_adversarial_evaluation(config).unwrap();
        let first_case = &first.cases[0];
        let second_case = &second.cases[0];
        assert_eq!(
            first_case.hero_total_chip_delta,
            second_case.hero_total_chip_delta
        );
        assert_eq!(first_case.accepted_actions, second_case.accepted_actions);
        assert_eq!(
            first_case.hero_bust_episodes,
            second_case.hero_bust_episodes
        );
    }

    #[test]
    fn evaluation_rejects_unbounded_or_duplicate_dimensions() {
        let invalid = AdversarialEvaluationConfig {
            deals_per_table: 0,
            ..AdversarialEvaluationConfig::default()
        };
        assert_eq!(
            run_adversarial_evaluation(invalid).unwrap_err(),
            AdversarialEvaluationError::InvalidDealCount(0)
        );

        let duplicate = AdversarialEvaluationConfig {
            seat_counts: vec![6, 6],
            ..AdversarialEvaluationConfig::default()
        };
        assert_eq!(
            run_adversarial_evaluation(duplicate).unwrap_err(),
            AdversarialEvaluationError::DuplicateSeatCount(6)
        );
    }
}
