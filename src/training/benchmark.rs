//! Deterministic environment-throughput benchmarks and lower-bound forecasts.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::thread;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::game::seat::SeatId;
use crate::protocol::HandId;

use super::arena::{ArenaConfig, ArenaRecordingMode, TrainingArena};
use super::deal::DealPlanV1;
use super::policy::{CheckCallPolicy, Policy, RandomLegalPolicy};

pub const BENCHMARK_REPORT_VERSION: u16 = 1;
pub const MAX_BENCHMARK_HANDS_PER_CASE: u64 = 100_000_000;
pub const MAX_BENCHMARK_WORKERS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkPolicy {
    CheckCall,
    RandomLegal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkRecording {
    Minimal,
    FullJson,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkConfig {
    pub hands_per_case: u64,
    pub worker_counts: Vec<usize>,
    pub policies: Vec<BenchmarkPolicy>,
    pub recordings: Vec<BenchmarkRecording>,
    pub starting_stack: u32,
    pub base_seed: u64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            hands_per_case: 100_000,
            worker_counts: default_worker_counts(),
            policies: vec![BenchmarkPolicy::CheckCall, BenchmarkPolicy::RandomLegal],
            recordings: vec![BenchmarkRecording::Minimal, BenchmarkRecording::FullJson],
            starting_stack: 100,
            base_seed: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentProjections {
    pub one_million_decisions_seconds: Option<f64>,
    pub ten_million_decisions_seconds: Option<f64>,
    pub one_hundred_million_decisions_seconds: Option<f64>,
    pub one_billion_decisions_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkCase {
    pub policy: BenchmarkPolicy,
    pub recording: BenchmarkRecording,
    pub workers: usize,
    pub requested_hands: u64,
    pub completed_hands: u64,
    pub failed_hands: u64,
    pub accepted_actions: u64,
    pub serialized_trajectory_bytes: u64,
    pub elapsed_seconds: f64,
    pub hands_per_second: f64,
    pub decisions_per_second: f64,
    pub environment_only_projections: EnvironmentProjections,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BenchmarkReport {
    pub version: u16,
    pub logical_parallelism: usize,
    pub config: BenchmarkConfig,
    pub cases: Vec<BenchmarkCase>,
    pub unmeasured_components: Vec<String>,
}

impl BenchmarkReport {
    pub fn total_failures(&self) -> u64 {
        self.cases.iter().map(|case| case.failed_hands).sum()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchmarkError {
    InvalidHandCount(u64),
    InvalidStartingStack,
    MissingPolicies,
    MissingRecordingModes,
    InvalidWorkerCount(usize),
    DuplicateWorkerCount(usize),
    WorkerPanicked,
}

impl Display for BenchmarkError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHandCount(count) => write!(
                formatter,
                "hands per case must be between 1 and {MAX_BENCHMARK_HANDS_PER_CASE}, found {count}"
            ),
            Self::InvalidStartingStack => write!(formatter, "starting stack must be positive"),
            Self::MissingPolicies => write!(formatter, "at least one benchmark policy is required"),
            Self::MissingRecordingModes => {
                write!(
                    formatter,
                    "at least one benchmark recording mode is required"
                )
            }
            Self::InvalidWorkerCount(count) => write!(
                formatter,
                "worker count must be between 1 and {MAX_BENCHMARK_WORKERS}, found {count}"
            ),
            Self::DuplicateWorkerCount(count) => {
                write!(formatter, "worker count {count} is duplicated")
            }
            Self::WorkerPanicked => write!(formatter, "a benchmark worker panicked"),
        }
    }
}

impl Error for BenchmarkError {}

#[derive(Default)]
struct WorkerResult {
    completed_hands: u64,
    failed_hands: u64,
    accepted_actions: u64,
    serialized_trajectory_bytes: u64,
    first_error: Option<String>,
}

pub fn default_worker_counts() -> Vec<usize> {
    let available = thread::available_parallelism().map_or(1, usize::from);
    let ceiling = available.min(16);
    let mut counts = Vec::new();
    let mut next = 1;
    while next <= ceiling {
        counts.push(next);
        next *= 2;
    }
    if counts.last().copied() != Some(ceiling) {
        counts.push(ceiling);
    }
    counts
}

pub fn run_benchmark(config: BenchmarkConfig) -> Result<BenchmarkReport, BenchmarkError> {
    validate_config(&config)?;
    let logical_parallelism = thread::available_parallelism().map_or(1, usize::from);
    let mut cases = Vec::new();
    for &policy in &config.policies {
        for &recording in &config.recordings {
            for &workers in &config.worker_counts {
                cases.push(run_case(&config, policy, recording, workers)?);
            }
        }
    }
    Ok(BenchmarkReport {
        version: BENCHMARK_REPORT_VERSION,
        logical_parallelism,
        config,
        cases,
        unmeasured_components: vec![
            "mathematical oracle generation is not implemented".to_string(),
            "CFR/MCCFR traversal is not implemented".to_string(),
            "neural inference, distillation, and PPO are not implemented".to_string(),
            "filesystem dataset publication is excluded from this in-memory benchmark".to_string(),
        ],
    })
}

fn validate_config(config: &BenchmarkConfig) -> Result<(), BenchmarkError> {
    if config.hands_per_case == 0 || config.hands_per_case > MAX_BENCHMARK_HANDS_PER_CASE {
        return Err(BenchmarkError::InvalidHandCount(config.hands_per_case));
    }
    if config.starting_stack == 0 {
        return Err(BenchmarkError::InvalidStartingStack);
    }
    if config.policies.is_empty() {
        return Err(BenchmarkError::MissingPolicies);
    }
    if config.recordings.is_empty() {
        return Err(BenchmarkError::MissingRecordingModes);
    }
    let mut seen = BTreeSet::new();
    for &workers in &config.worker_counts {
        if workers == 0 || workers > MAX_BENCHMARK_WORKERS {
            return Err(BenchmarkError::InvalidWorkerCount(workers));
        }
        if !seen.insert(workers) {
            return Err(BenchmarkError::DuplicateWorkerCount(workers));
        }
    }
    if config.worker_counts.is_empty() {
        return Err(BenchmarkError::InvalidWorkerCount(0));
    }
    Ok(())
}

fn run_case(
    config: &BenchmarkConfig,
    policy: BenchmarkPolicy,
    recording: BenchmarkRecording,
    workers: usize,
) -> Result<BenchmarkCase, BenchmarkError> {
    let started = Instant::now();
    let mut handles = Vec::with_capacity(workers);
    for worker_index in 0..workers {
        let hands = config.hands_per_case;
        let starting_stack = config.starting_stack;
        let base_seed = config.base_seed;
        handles.push(thread::spawn(move || {
            run_worker(
                worker_index,
                workers,
                hands,
                starting_stack,
                base_seed,
                policy,
                recording,
            )
        }));
    }

    let mut aggregate = WorkerResult::default();
    for handle in handles {
        let result = handle.join().map_err(|_| BenchmarkError::WorkerPanicked)?;
        aggregate.completed_hands += result.completed_hands;
        aggregate.failed_hands += result.failed_hands;
        aggregate.accepted_actions += result.accepted_actions;
        aggregate.serialized_trajectory_bytes += result.serialized_trajectory_bytes;
        if aggregate.first_error.is_none() {
            aggregate.first_error = result.first_error;
        }
    }
    let elapsed_seconds = started.elapsed().as_secs_f64().max(f64::EPSILON);
    let hands_per_second = aggregate.completed_hands as f64 / elapsed_seconds;
    let decisions_per_second = aggregate.accepted_actions as f64 / elapsed_seconds;
    Ok(BenchmarkCase {
        policy,
        recording,
        workers,
        requested_hands: config.hands_per_case,
        completed_hands: aggregate.completed_hands,
        failed_hands: aggregate.failed_hands,
        accepted_actions: aggregate.accepted_actions,
        serialized_trajectory_bytes: aggregate.serialized_trajectory_bytes,
        elapsed_seconds,
        hands_per_second,
        decisions_per_second,
        environment_only_projections: projections(decisions_per_second),
        first_error: aggregate.first_error,
    })
}

fn run_worker(
    worker_index: usize,
    workers: usize,
    hands: u64,
    starting_stack: u32,
    base_seed: u64,
    policy: BenchmarkPolicy,
    recording: BenchmarkRecording,
) -> WorkerResult {
    let mut result = WorkerResult::default();
    let mut hand_index = worker_index as u64;
    while hand_index < hands {
        match run_hand(hand_index, starting_stack, base_seed, policy, recording) {
            Ok((decisions, bytes)) => {
                result.completed_hands += 1;
                result.accepted_actions += decisions;
                result.serialized_trajectory_bytes += bytes;
            }
            Err(error) => {
                result.failed_hands += 1;
                if result.first_error.is_none() {
                    result.first_error = Some(error);
                }
            }
        }
        hand_index += workers as u64;
    }
    result
}

fn run_hand(
    hand_index: u64,
    starting_stack: u32,
    base_seed: u64,
    policy: BenchmarkPolicy,
    recording: BenchmarkRecording,
) -> Result<(u64, u64), String> {
    let deal_seed = base_seed.wrapping_add(hand_index);
    let arena_recording = match recording {
        BenchmarkRecording::Minimal => ArenaRecordingMode::Minimal,
        BenchmarkRecording::FullJson => ArenaRecordingMode::Full,
    };
    let mut arena_config = ArenaConfig::heads_up(starting_stack).with_recording(arena_recording);
    arena_config.hand_id = HandId(hand_index.wrapping_add(1));
    let mut arena = TrainingArena::new(arena_config, DealPlanV1::seeded(deal_seed))
        .map_err(|error| error.to_string())?;
    let mut policies = policies_for_hand(policy, base_seed, hand_index);
    match recording {
        BenchmarkRecording::Minimal => {
            let summary = arena
                .run_to_terminal_summary(&mut policies)
                .map_err(|error| error.to_string())?;
            Ok((summary.accepted_actions as u64, 0))
        }
        BenchmarkRecording::FullJson => {
            let episode = arena
                .run_to_terminal(&mut policies)
                .map_err(|error| error.to_string())?;
            let decisions = episode.decisions.len() as u64;
            let bytes = serde_json::to_vec(&episode)
                .map_err(|error| error.to_string())?
                .len() as u64;
            Ok((decisions, bytes))
        }
    }
}

fn policies_for_hand(
    policy: BenchmarkPolicy,
    base_seed: u64,
    hand_index: u64,
) -> BTreeMap<SeatId, Box<dyn Policy>> {
    let first = SeatId::new(0).expect("heads-up seat is valid");
    let second = SeatId::new(1).expect("heads-up seat is valid");
    match policy {
        BenchmarkPolicy::CheckCall => BTreeMap::from([
            (first, Box::<CheckCallPolicy>::default() as Box<dyn Policy>),
            (second, Box::<CheckCallPolicy>::default() as Box<dyn Policy>),
        ]),
        BenchmarkPolicy::RandomLegal => {
            let hand_seed = base_seed
                .wrapping_add(hand_index.rotate_left(17))
                .wrapping_add(0x9E37_79B9_7F4A_7C15);
            BTreeMap::from([
                (
                    first,
                    Box::new(RandomLegalPolicy::seeded(hand_seed)) as Box<dyn Policy>,
                ),
                (
                    second,
                    Box::new(RandomLegalPolicy::seeded(hand_seed ^ 0xD1B5_4A32_D192_ED03))
                        as Box<dyn Policy>,
                ),
            ])
        }
    }
}

fn projections(decisions_per_second: f64) -> EnvironmentProjections {
    let estimate = |decisions: f64| {
        (decisions_per_second.is_finite() && decisions_per_second > 0.0)
            .then_some(decisions / decisions_per_second)
    };
    EnvironmentProjections {
        one_million_decisions_seconds: estimate(1_000_000.0),
        ten_million_decisions_seconds: estimate(10_000_000.0),
        one_hundred_million_decisions_seconds: estimate(100_000_000.0),
        one_billion_decisions_seconds: estimate(1_000_000_000.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_is_deterministic_across_worker_partitions_and_reports_overheads() {
        let report = run_benchmark(BenchmarkConfig {
            hands_per_case: 12,
            worker_counts: vec![1, 2],
            policies: vec![BenchmarkPolicy::CheckCall],
            recordings: vec![BenchmarkRecording::Minimal, BenchmarkRecording::FullJson],
            starting_stack: 100,
            base_seed: 73,
        })
        .unwrap();
        assert_eq!(report.cases.len(), 4);
        assert_eq!(report.total_failures(), 0);
        for case in &report.cases {
            assert_eq!(case.completed_hands, 12);
            assert_eq!(case.accepted_actions, 96);
            assert!(case.hands_per_second > 0.0);
            assert!(case.decisions_per_second > 0.0);
            assert!(case
                .environment_only_projections
                .one_billion_decisions_seconds
                .is_some());
            match case.recording {
                BenchmarkRecording::Minimal => {
                    assert_eq!(case.serialized_trajectory_bytes, 0)
                }
                BenchmarkRecording::FullJson => {
                    assert!(case.serialized_trajectory_bytes > 0)
                }
            }
        }
        let full_bytes = report
            .cases
            .iter()
            .filter(|case| case.recording == BenchmarkRecording::FullJson)
            .map(|case| case.serialized_trajectory_bytes)
            .collect::<BTreeSet<_>>();
        assert_eq!(full_bytes.len(), 1);
    }

    #[test]
    fn benchmark_configuration_is_bounded() {
        let config = BenchmarkConfig {
            hands_per_case: 0,
            ..BenchmarkConfig::default()
        };
        assert_eq!(
            run_benchmark(config).unwrap_err(),
            BenchmarkError::InvalidHandCount(0)
        );

        let config = BenchmarkConfig {
            worker_counts: vec![2, 2],
            ..BenchmarkConfig::default()
        };
        assert_eq!(
            run_benchmark(config).unwrap_err(),
            BenchmarkError::DuplicateWorkerCount(2)
        );
    }
}
