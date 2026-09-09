//! Projection-native, deterministic poker-policy training tools.
//!
//! This module is an adapter over the authoritative game and protocol layers.
//! It does not define alternate poker rules and is not used by production table
//! construction.

pub mod action;
pub mod arena;
pub mod benchmark;
pub mod deal;
pub mod evaluation;
pub mod observation;
pub mod policy;

pub use action::{legal_policy_actions, map_policy_action, PolicyActionError, PolicyActionV1};
pub use arena::{
    ArenaConfig, ArenaError, ArenaRecordingMode, TrainingArena, TrainingDecisionV1,
    TrainingEpisodeV1, TrainingRunSummaryV1, TRAINING_EPISODE_VERSION,
    TRAINING_RUN_SUMMARY_VERSION,
};
pub use benchmark::{
    default_worker_counts, run_benchmark, BenchmarkCase, BenchmarkConfig, BenchmarkError,
    BenchmarkPolicy, BenchmarkRecording, BenchmarkReport, EnvironmentProjections,
    BENCHMARK_REPORT_VERSION,
};
pub use deal::{
    DealPlanError, DealPlanV1, WeightedHoleCombo, WeightedRangeV1, DEAL_PLAN_VERSION,
    WEIGHTED_RANGE_VERSION,
};
pub use evaluation::{
    run_adversarial_evaluation, AdversarialCase, AdversarialEvaluationConfig,
    AdversarialEvaluationError, AdversarialEvaluationReport, AdversarialPolicy,
    TournamentCapability, ADVERSARIAL_REPORT_VERSION,
};
pub use observation::{
    PolicyObservationError, PolicyObservationV1, PolicyPublicActionV1, PolicySeatV1,
    POLICY_OBSERVATION_VERSION,
};
pub use policy::{
    estimate_uniform_equity, CheckCallPolicy, EquityPotOddsPolicy, FoldCheckPolicy, JamPolicy,
    Policy, PotPressurePolicy, RandomLegalPolicy,
};

#[cfg(test)]
mod tests;
