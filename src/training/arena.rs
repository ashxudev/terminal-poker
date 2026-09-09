//! Fast, synchronous one-hand arena over the protocol authority.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::game::actions::Action;
use crate::game::multiway::{ForcedPost, MultiwayConfigError, MultiwayHand, MultiwayPhase};
use crate::game::seat::{SeatId, TableSize};
use crate::protocol::{
    CommandEnvelope, EventEnvelope, HandId, ProjectionAudience, ProtocolAuthority,
    ProtocolErrorCode, SnapshotEnvelope, TableId,
};
use crate::ring_history::{HistoryError, SafeRingHandHistory, MAX_HISTORY_ACTIONS};

use super::action::{map_policy_action, PolicyActionError, PolicyActionV1};
use super::deal::{DealPlanError, DealPlanV1};
use super::observation::{PolicyObservationError, PolicyObservationV1};
use super::policy::Policy;

pub const TRAINING_EPISODE_VERSION: u16 = 1;
pub const TRAINING_RUN_SUMMARY_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArenaRecordingMode {
    Minimal,
    #[default]
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArenaConfig {
    pub table_id: TableId,
    pub hand_id: HandId,
    pub table_size: TableSize,
    pub button: SeatId,
    pub stacks: Vec<(SeatId, u32)>,
    pub forced_posts: Vec<ForcedPost>,
    pub max_actions: usize,
    pub recording: ArenaRecordingMode,
}

impl ArenaConfig {
    pub fn heads_up(starting_stack: u32) -> Self {
        Self {
            table_id: TableId(1),
            hand_id: HandId(1),
            table_size: TableSize::new(2).expect("heads-up table size is valid"),
            button: SeatId::new(0).expect("heads-up button is valid"),
            stacks: vec![
                (
                    SeatId::new(0).expect("heads-up seat is valid"),
                    starting_stack,
                ),
                (
                    SeatId::new(1).expect("heads-up seat is valid"),
                    starting_stack,
                ),
            ],
            forced_posts: Vec::new(),
            max_actions: MAX_HISTORY_ACTIONS,
            recording: ArenaRecordingMode::Full,
        }
    }

    pub fn with_recording(mut self, recording: ArenaRecordingMode) -> Self {
        self.recording = recording;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingDecisionV1 {
    pub observation: PolicyObservationV1,
    pub policy_action: PolicyActionV1,
    pub authoritative_action: Action,
    pub accepted_event: EventEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingEpisodeV1 {
    pub version: u16,
    /// Private training records. This collection must not enter public history.
    pub decisions: Vec<TrainingDecisionV1>,
    pub terminal_public_snapshot: SnapshotEnvelope,
    pub safe_history: SafeRingHandHistory,
    pub chip_deltas: Vec<(SeatId, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingRunSummaryV1 {
    pub version: u16,
    pub accepted_actions: usize,
    pub terminal_phase: MultiwayPhase,
    pub chip_deltas: Vec<(SeatId, i64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArenaError {
    DealPlan(DealPlanError),
    InvalidHand(MultiwayConfigError),
    Projection,
    Observation(PolicyObservationError),
    PolicyAction(PolicyActionError),
    AuthorityRejected(ProtocolErrorCode),
    MissingPolicy(SeatId),
    HandAlreadyTerminal,
    HandNotTerminal,
    ActionLimitReached(usize),
    SafeHistory(HistoryError),
}

impl Display for ArenaError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DealPlan(error) => write!(formatter, "invalid training deal: {error}"),
            Self::InvalidHand(error) => write!(formatter, "invalid training hand: {error}"),
            Self::Projection => write!(formatter, "authorized projection construction failed"),
            Self::Observation(error) => write!(formatter, "invalid policy observation: {error}"),
            Self::PolicyAction(error) => write!(formatter, "invalid policy output: {error}"),
            Self::AuthorityRejected(code) => {
                write!(
                    formatter,
                    "authoritative engine rejected mapped action: {}",
                    code.name()
                )
            }
            Self::MissingPolicy(seat) => {
                write!(
                    formatter,
                    "no policy is installed for seat {}",
                    seat.as_u8()
                )
            }
            Self::HandAlreadyTerminal => write!(formatter, "training hand is already terminal"),
            Self::HandNotTerminal => {
                write!(formatter, "training hand has not reached a terminal state")
            }
            Self::ActionLimitReached(limit) => {
                write!(formatter, "training hand reached its {limit}-action limit")
            }
            Self::SafeHistory(error) => {
                write!(
                    formatter,
                    "safe terminal history construction failed: {error:?}"
                )
            }
        }
    }
}

impl Error for ArenaError {}

pub struct TrainingArena {
    authority: ProtocolAuthority,
    initial_stacks: BTreeMap<SeatId, u32>,
    accepted_events: Vec<EventEnvelope>,
    decisions: Vec<TrainingDecisionV1>,
    max_actions: usize,
    recording: ArenaRecordingMode,
}

impl TrainingArena {
    pub fn new(config: ArenaConfig, deal: DealPlanV1) -> Result<Self, ArenaError> {
        if config.max_actions == 0 || config.max_actions > MAX_HISTORY_ACTIONS {
            return Err(ArenaError::ActionLimitReached(config.max_actions));
        }
        let deck = deal.into_deck().map_err(ArenaError::DealPlan)?;
        let hand = MultiwayHand::new_with_deck_for_training(
            config.table_size,
            config.button,
            &config.stacks,
            &config.forced_posts,
            deck,
        )
        .map_err(ArenaError::InvalidHand)?;
        Ok(Self {
            authority: ProtocolAuthority::new(config.table_id, config.hand_id, hand),
            initial_stacks: config.stacks.into_iter().collect(),
            accepted_events: Vec::new(),
            decisions: Vec::new(),
            max_actions: config.max_actions,
            recording: config.recording,
        })
    }

    pub fn current_observation(&self) -> Result<Option<PolicyObservationV1>, ArenaError> {
        let public = self
            .authority
            .snapshot(ProjectionAudience::Spectator)
            .map_err(|_| ArenaError::Projection)?;
        let Some(actor) = public.snapshot.to_act else {
            return Ok(None);
        };
        let private = self
            .authority
            .snapshot(ProjectionAudience::Player(actor))
            .map_err(|_| ArenaError::Projection)?;
        PolicyObservationV1::from_authorized(&private, &self.accepted_events)
            .map(Some)
            .map_err(ArenaError::Observation)
    }

    pub fn step(
        &mut self,
        policy_action: PolicyActionV1,
    ) -> Result<TrainingDecisionV1, ArenaError> {
        if self.accepted_events.len() >= self.max_actions {
            return Err(ArenaError::ActionLimitReached(self.max_actions));
        }
        let observation = self
            .current_observation()?
            .ok_or(ArenaError::HandAlreadyTerminal)?;
        let authoritative_action =
            map_policy_action(&observation, policy_action).map_err(ArenaError::PolicyAction)?;
        let command = CommandEnvelope::act_for_hand(
            format!("train-{}", observation.revision + 1),
            observation.table_id,
            observation.hand_id,
            observation.revision,
            observation.acting_seat,
            authoritative_action,
        );
        let accepted_event = self
            .authority
            .submit(command)
            .map_err(|error| ArenaError::AuthorityRejected(error.error.code))?;
        self.accepted_events.push(accepted_event.clone());
        let decision = TrainingDecisionV1 {
            observation,
            policy_action,
            authoritative_action,
            accepted_event,
        };
        if self.recording == ArenaRecordingMode::Full {
            self.decisions.push(decision.clone());
        }
        Ok(decision)
    }

    pub fn run_to_terminal(
        &mut self,
        policies: &mut BTreeMap<SeatId, Box<dyn Policy>>,
    ) -> Result<TrainingEpisodeV1, ArenaError> {
        while let Some(observation) = self.current_observation()? {
            let policy = policies
                .get_mut(&observation.acting_seat)
                .ok_or(ArenaError::MissingPolicy(observation.acting_seat))?;
            let action = policy
                .select_action(&observation)
                .map_err(ArenaError::PolicyAction)?;
            self.step(action)?;
        }
        self.episode()
    }

    pub fn run_to_terminal_summary(
        &mut self,
        policies: &mut BTreeMap<SeatId, Box<dyn Policy>>,
    ) -> Result<TrainingRunSummaryV1, ArenaError> {
        while let Some(observation) = self.current_observation()? {
            let policy = policies
                .get_mut(&observation.acting_seat)
                .ok_or(ArenaError::MissingPolicy(observation.acting_seat))?;
            let action = policy
                .select_action(&observation)
                .map_err(ArenaError::PolicyAction)?;
            self.step(action)?;
        }
        let terminal = self.terminal_snapshot()?;
        Ok(TrainingRunSummaryV1 {
            version: TRAINING_RUN_SUMMARY_VERSION,
            accepted_actions: self.accepted_events.len(),
            terminal_phase: terminal.snapshot.phase,
            chip_deltas: self.chip_deltas(&terminal),
        })
    }

    pub fn episode(&self) -> Result<TrainingEpisodeV1, ArenaError> {
        let terminal = self.terminal_snapshot()?;
        let safe_history =
            SafeRingHandHistory::from_public_terminal(&terminal, &self.accepted_events)
                .map_err(ArenaError::SafeHistory)?;
        let chip_deltas = self.chip_deltas(&terminal);
        Ok(TrainingEpisodeV1 {
            version: TRAINING_EPISODE_VERSION,
            decisions: self.decisions.clone(),
            terminal_public_snapshot: terminal,
            safe_history,
            chip_deltas,
        })
    }

    fn terminal_snapshot(&self) -> Result<SnapshotEnvelope, ArenaError> {
        let terminal = self
            .authority
            .snapshot(ProjectionAudience::Spectator)
            .map_err(|_| ArenaError::Projection)?;
        if !matches!(
            terminal.snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            return Err(ArenaError::HandNotTerminal);
        }
        Ok(terminal)
    }

    fn chip_deltas(&self, terminal: &SnapshotEnvelope) -> Vec<(SeatId, i64)> {
        terminal
            .snapshot
            .seats
            .iter()
            .map(|seat| {
                let initial = self.initial_stacks.get(&seat.seat).copied().unwrap_or(0);
                (seat.seat, i64::from(seat.stack) - i64::from(initial))
            })
            .collect()
    }

    pub fn accepted_events(&self) -> &[EventEnvelope] {
        &self.accepted_events
    }
}
