//! Versioned policy input derived only from authorized projections and events.

use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::game::actions::Action;
use crate::game::deck::Card;
use crate::game::multiway::{MultiwayLegalActions, MultiwayPhase};
use crate::game::seat::{SeatId, TableSize};
use crate::game::state::BIG_BLIND;
use crate::game::table::HandParticipation;
use crate::protocol::{
    EventEnvelope, HandId, ProjectionKind, SnapshotEnvelope, TableEvent, TableId, PROTOCOL_VERSION,
};
use crate::ring_history::MAX_HISTORY_ACTIONS;

pub const POLICY_OBSERVATION_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyPublicActionV1 {
    pub revision: u64,
    pub seat: SeatId,
    pub phase: MultiwayPhase,
    pub action: Action,
    pub next_to_act: Option<SeatId>,
    pub pot_after: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySeatV1 {
    pub seat: SeatId,
    pub stack: u32,
    pub street_contribution: u32,
    pub hand_contribution: u32,
    pub participation: HandParticipation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyObservationV1 {
    pub version: u16,
    pub table_id: TableId,
    pub hand_id: HandId,
    pub revision: u64,
    pub acting_seat: SeatId,
    pub table_size: TableSize,
    pub phase: MultiwayPhase,
    pub button: SeatId,
    pub small_blind: SeatId,
    pub big_blind: SeatId,
    pub big_blind_amount: u32,
    pub hole_cards: [Card; 2],
    pub board: Vec<Card>,
    pub seats: Vec<PolicySeatV1>,
    pub current_wager: u32,
    pub pot_total: u32,
    pub amount_to_call: u32,
    pub effective_stack: u32,
    /// Effective stack divided by pot, scaled by 1,000. `None` means zero pot.
    pub stack_to_pot_milli: Option<u32>,
    /// Call cost divided by pot after calling, scaled by 1,000,000.
    pub pot_odds_millionths: Option<u32>,
    pub legal_actions: MultiwayLegalActions,
    pub action_history: Vec<PolicyPublicActionV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyObservationError {
    UnsupportedProtocolVersion(u16),
    SpectatorProjection,
    AudienceMismatch,
    ActorNotFound,
    OwnCardsUnavailable,
    OwnCardCount(usize),
    NotActing,
    LegalActionsUnavailable,
    TooManyActions(usize),
    IncompleteHistory { revision: u64, actions: usize },
    WrongEventScope,
    NonMonotonicEventRevision,
}

impl Display for PolicyObservationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedProtocolVersion(version) => {
                write!(formatter, "protocol version {version} is unsupported")
            }
            Self::SpectatorProjection => write!(formatter, "policy input requires a player view"),
            Self::AudienceMismatch => {
                write!(
                    formatter,
                    "projection audience does not match its acting seat"
                )
            }
            Self::ActorNotFound => write!(formatter, "acting seat is absent from its projection"),
            Self::OwnCardsUnavailable => {
                write!(formatter, "acting player's private cards are unavailable")
            }
            Self::OwnCardCount(count) => {
                write!(
                    formatter,
                    "acting player must have two private cards, found {count}"
                )
            }
            Self::NotActing => write!(formatter, "player projection is not the current actor"),
            Self::LegalActionsUnavailable => {
                write!(
                    formatter,
                    "acting player projection has no legal-action mask"
                )
            }
            Self::TooManyActions(count) => write!(
                formatter,
                "policy history exceeds the {MAX_HISTORY_ACTIONS}-action bound: {count}"
            ),
            Self::IncompleteHistory { revision, actions } => write!(
                formatter,
                "policy history has {actions} actions for authoritative revision {revision}"
            ),
            Self::WrongEventScope => {
                write!(formatter, "accepted event targets another protocol or hand")
            }
            Self::NonMonotonicEventRevision => {
                write!(formatter, "accepted event revisions are not contiguous")
            }
        }
    }
}

impl Error for PolicyObservationError {}

impl PolicyObservationV1 {
    pub fn from_authorized(
        envelope: &SnapshotEnvelope,
        accepted_events: &[EventEnvelope],
    ) -> Result<Self, PolicyObservationError> {
        if envelope.version != PROTOCOL_VERSION {
            return Err(PolicyObservationError::UnsupportedProtocolVersion(
                envelope.version,
            ));
        }
        if accepted_events.len() > MAX_HISTORY_ACTIONS {
            return Err(PolicyObservationError::TooManyActions(
                accepted_events.len(),
            ));
        }
        if envelope.revision != accepted_events.len() as u64 {
            return Err(PolicyObservationError::IncompleteHistory {
                revision: envelope.revision,
                actions: accepted_events.len(),
            });
        }
        let acting_seat = match envelope.snapshot.audience {
            ProjectionKind::Player { seat } => seat,
            ProjectionKind::Spectator => return Err(PolicyObservationError::SpectatorProjection),
        };
        if envelope.snapshot.to_act != Some(acting_seat) {
            return Err(PolicyObservationError::NotActing);
        }
        let legal_actions = envelope
            .snapshot
            .legal_actions
            .clone()
            .ok_or(PolicyObservationError::LegalActionsUnavailable)?;
        let actor = envelope
            .snapshot
            .seats
            .iter()
            .find(|seat| seat.seat == acting_seat)
            .ok_or(PolicyObservationError::ActorNotFound)?;
        let cards = actor
            .hole_cards
            .as_ref()
            .ok_or(PolicyObservationError::OwnCardsUnavailable)?;
        if cards.len() != 2 {
            return Err(PolicyObservationError::OwnCardCount(cards.len()));
        }
        let hole_cards = [cards[0], cards[1]];
        let amount_to_call = envelope
            .snapshot
            .current_wager
            .saturating_sub(actor.street_contribution);
        let largest_opponent_stack = envelope
            .snapshot
            .seats
            .iter()
            .filter(|seat| seat.seat != acting_seat)
            .filter(|seat| {
                matches!(
                    seat.participation,
                    HandParticipation::Live | HandParticipation::AllIn
                )
            })
            .map(|seat| seat.stack)
            .max()
            .unwrap_or(0);
        let effective_stack = actor.stack.min(largest_opponent_stack);
        let stack_to_pot_milli = (envelope.snapshot.pot_total > 0)
            .then(|| scaled_ratio(effective_stack, envelope.snapshot.pot_total, 1_000));
        let pot_after_call = envelope.snapshot.pot_total.saturating_add(amount_to_call);
        let pot_odds_millionths = (amount_to_call > 0 && pot_after_call > 0)
            .then(|| scaled_ratio(amount_to_call, pot_after_call, 1_000_000));

        let mut action_history = Vec::with_capacity(accepted_events.len());
        for (index, event) in accepted_events.iter().enumerate() {
            if event.version != PROTOCOL_VERSION
                || event.table_id != envelope.table_id
                || event.hand_id != envelope.hand_id
                || event.revision > envelope.revision
            {
                return Err(PolicyObservationError::WrongEventScope);
            }
            if event.revision != index as u64 + 1 {
                return Err(PolicyObservationError::NonMonotonicEventRevision);
            }
            let TableEvent::ActionAccepted {
                seat,
                action,
                phase,
                next_to_act,
                pot_total,
            } = event.event
            else {
                continue;
            };
            action_history.push(PolicyPublicActionV1 {
                revision: event.revision,
                seat,
                phase,
                action,
                next_to_act,
                pot_after: pot_total,
            });
        }

        Ok(Self {
            version: POLICY_OBSERVATION_VERSION,
            table_id: envelope.table_id,
            hand_id: envelope.hand_id,
            revision: envelope.revision,
            acting_seat,
            table_size: envelope.snapshot.table_size,
            phase: envelope.snapshot.phase,
            button: envelope.snapshot.button,
            small_blind: envelope.snapshot.small_blind,
            big_blind: envelope.snapshot.big_blind,
            big_blind_amount: BIG_BLIND,
            hole_cards,
            board: envelope.snapshot.board.clone(),
            seats: envelope
                .snapshot
                .seats
                .iter()
                .map(|seat| PolicySeatV1 {
                    seat: seat.seat,
                    stack: seat.stack,
                    street_contribution: seat.street_contribution,
                    hand_contribution: seat.hand_contribution,
                    participation: seat.participation,
                })
                .collect(),
            current_wager: envelope.snapshot.current_wager,
            pot_total: envelope.snapshot.pot_total,
            amount_to_call,
            effective_stack,
            stack_to_pot_milli,
            pot_odds_millionths,
            legal_actions,
            action_history,
        })
    }

    pub fn acting_seat_state(&self) -> &PolicySeatV1 {
        self.seats
            .iter()
            .find(|seat| seat.seat == self.acting_seat)
            .expect("validated observations contain the acting seat")
    }
}

fn scaled_ratio(numerator: u32, denominator: u32, scale: u32) -> u32 {
    let scaled = u64::from(numerator) * u64::from(scale);
    u32::try_from(scaled / u64::from(denominator)).unwrap_or(u32::MAX)
}
