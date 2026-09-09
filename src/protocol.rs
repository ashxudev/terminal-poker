//! Versioned protocol envelopes and audience-specific table projections.
//!
//! This module is transport-neutral. It converts untrusted controller intent
//! into the existing authoritative domain command and constructs a fresh view
//! for each recipient. Complete internal hand state is never serializable here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::game::actions::Action;
use crate::game::command::{CommandError, SeatCommand};
use crate::game::deck::Card;
use crate::game::multiway::{MultiwayHand, MultiwayLegalActions, MultiwayPhase, Pot, PotAward};
use crate::game::seat::{SeatId, TableSize};
use crate::game::table::HandParticipation;

pub const PROTOCOL_VERSION: u16 = 4;
pub const MAX_COMMAND_ID_BYTES: usize = 64;
pub const MAX_COMMAND_ENVELOPE_BYTES: usize = 1_024;
pub const MAX_PUBLIC_ERROR_MESSAGE_BYTES: usize = 160;
pub const MAX_RECORDED_COMMANDS_PER_HAND: usize = 256;

#[cfg(test)]
mod showdown_tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TableId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HandId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    pub version: u16,
    pub command_id: String,
    pub table_id: TableId,
    pub hand_id: HandId,
    pub expected_revision: u64,
    pub payload: CommandPayload,
}

impl CommandEnvelope {
    pub fn act(
        command_id: impl Into<String>,
        table_id: TableId,
        expected_revision: u64,
        seat: SeatId,
        action: Action,
    ) -> Self {
        Self::act_for_hand(
            command_id,
            table_id,
            HandId(1),
            expected_revision,
            seat,
            action,
        )
    }

    pub fn act_for_hand(
        command_id: impl Into<String>,
        table_id: TableId,
        hand_id: HandId,
        expected_revision: u64,
        seat: SeatId,
        action: Action,
    ) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            command_id: command_id.into(),
            table_id,
            hand_id,
            expected_revision,
            payload: CommandPayload::Act { seat, action },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandPayload {
    Act { seat: SeatId, action: Action },
    ShowdownPreference { seat: SeatId, always_show: bool },
}

impl CommandPayload {
    pub fn seat(&self) -> SeatId {
        match *self {
            Self::Act { seat, .. } | Self::ShowdownPreference { seat, .. } => seat,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: u16,
    pub command_id: String,
    pub table_id: TableId,
    pub hand_id: HandId,
    pub revision: u64,
    pub event: TableEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TableEvent {
    ShowdownAdvanced,
    ShowdownPreferenceAccepted {
        seat: SeatId,
    },
    ActionAccepted {
        seat: SeatId,
        action: Action,
        phase: MultiwayPhase,
        next_to_act: Option<SeatId>,
        pot_total: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotEnvelope {
    pub version: u16,
    pub table_id: TableId,
    pub hand_id: HandId,
    pub revision: u64,
    pub snapshot: TableProjection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEnvelope {
    pub version: u16,
    pub command_id: Option<String>,
    pub table_id: TableId,
    pub hand_id: HandId,
    pub revision: u64,
    pub error: PublicProtocolError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcknowledgementResult {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AcknowledgementDelivery {
    Processed,
    Replayed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcknowledgementEnvelope {
    pub version: u16,
    pub command_id: Option<String>,
    pub table_id: TableId,
    pub hand_id: HandId,
    pub revision: u64,
    pub result: AcknowledgementResult,
    pub delivery: AcknowledgementDelivery,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum CommandOutcome {
    Accepted { event: EventEnvelope },
    Rejected { error: ErrorEnvelope },
}

impl CommandOutcome {
    pub fn into_result(self) -> Result<EventEnvelope, ErrorEnvelope> {
        match self {
            Self::Accepted { event } => Ok(event),
            Self::Rejected { error } => Err(error),
        }
    }

    pub const fn revision(&self) -> u64 {
        match self {
            Self::Accepted { event } => event.revision,
            Self::Rejected { error } => error.revision,
        }
    }

    fn command_id(&self) -> Option<String> {
        match self {
            Self::Accepted { event } => Some(event.command_id.clone()),
            Self::Rejected { error } => error.command_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmissionReceipt {
    pub acknowledgement: AcknowledgementEnvelope,
    pub outcome: CommandOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodeCommandError {
    pub code: ProtocolErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicProtocolError {
    pub code: ProtocolErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolErrorCode {
    UnsupportedVersion,
    InvalidCommandId,
    WrongTable,
    WrongHand,
    ReservedCommandId,
    CommandIdConflict,
    CommandLedgerFull,
    MessageTooLarge,
    MalformedMessage,
    StaleRevision,
    HandNotActive,
    SeatNotOccupied,
    SeatNotEligible,
    OutOfTurn,
    IllegalAction,
}

impl ProtocolErrorCode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidCommandId => "invalid_command_id",
            Self::WrongTable => "wrong_table",
            Self::WrongHand => "wrong_hand",
            Self::ReservedCommandId => "reserved_command_id",
            Self::CommandIdConflict => "command_id_conflict",
            Self::CommandLedgerFull => "command_ledger_full",
            Self::MessageTooLarge => "message_too_large",
            Self::MalformedMessage => "malformed_message",
            Self::StaleRevision => "stale_revision",
            Self::HandNotActive => "hand_not_active",
            Self::SeatNotOccupied => "seat_not_occupied",
            Self::SeatNotEligible => "seat_not_eligible",
            Self::OutOfTurn => "out_of_turn",
            Self::IllegalAction => "illegal_action",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionAudience {
    Player(SeatId),
    Spectator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProjectionKind {
    Player { seat: SeatId },
    Spectator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableProjection {
    pub showdown: Option<crate::game::multiway::ShowdownProgress>,
    pub mucked: Vec<SeatId>,
    pub shown: Vec<SeatId>,
    pub always_show: bool,
    pub hand_id: HandId,
    pub audience: ProjectionKind,
    pub table_size: TableSize,
    pub phase: MultiwayPhase,
    pub button: SeatId,
    pub small_blind: SeatId,
    pub big_blind: SeatId,
    #[serde(default = "default_small_blind_amount")]
    pub small_blind_amount: u32,
    #[serde(default = "default_big_blind_amount")]
    pub big_blind_amount: u32,
    #[serde(default)]
    pub ante_amount: u32,
    pub to_act: Option<SeatId>,
    pub board: Vec<Card>,
    pub current_wager: u32,
    pub pot_total: u32,
    pub seats: Vec<ProjectedSeat>,
    pub pots: Vec<Pot>,
    pub awards: Vec<PotAward>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legal_actions: Option<MultiwayLegalActions>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectedSeat {
    pub seat: SeatId,
    pub stack: u32,
    pub street_contribution: u32,
    pub hand_contribution: u32,
    pub participation: HandParticipation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hole_cards: Option<Vec<Card>>,
}

const fn default_small_blind_amount() -> u32 {
    1
}

const fn default_big_blind_amount() -> u32 {
    2
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionError {
    PlayerSeatNotOccupied(SeatId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandFingerprint {
    expected_revision: u64,
    payload: CommandPayload,
}

#[derive(Debug, Clone)]
struct RecordedCommand {
    fingerprint: CommandFingerprint,
    outcome: CommandOutcome,
}

#[derive(Debug, Clone)]
pub struct ProtocolAuthority {
    table_id: TableId,
    hand_id: HandId,
    revision: u64,
    hand: MultiwayHand,
    recorded_commands: BTreeMap<String, RecordedCommand>,
}

impl ProtocolAuthority {
    pub fn new_paced(table_id: TableId, hand_id: HandId, mut hand: MultiwayHand) -> Self {
        hand.enable_paced_showdown();
        Self::new(table_id, hand_id, hand)
    }

    pub(crate) fn advance_showdown(&mut self) -> Option<EventEnvelope> {
        if !self.hand.advance_showdown() {
            return None;
        }
        self.revision += 1;
        Some(EventEnvelope {
            version: PROTOCOL_VERSION,
            command_id: format!("srv-showdown-{}", self.revision),
            table_id: self.table_id,
            hand_id: self.hand_id,
            revision: self.revision,
            event: TableEvent::ShowdownAdvanced,
        })
    }
    pub fn new(table_id: TableId, hand_id: HandId, hand: MultiwayHand) -> Self {
        Self {
            table_id,
            hand_id,
            revision: 0,
            hand,
            recorded_commands: BTreeMap::new(),
        }
    }

    pub const fn table_id(&self) -> TableId {
        self.table_id
    }

    pub const fn hand_id(&self) -> HandId {
        self.hand_id
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn hand(&self) -> &MultiwayHand {
        &self.hand
    }

    pub fn recorded_command_count(&self) -> usize {
        self.recorded_commands.len()
    }

    pub fn submit(&mut self, envelope: CommandEnvelope) -> Result<EventEnvelope, ErrorEnvelope> {
        self.submit_with_acknowledgement(envelope)
            .outcome
            .into_result()
    }

    pub fn submit_with_acknowledgement(&mut self, envelope: CommandEnvelope) -> SubmissionReceipt {
        self.submit_from(envelope, false)
    }

    pub(crate) fn submit_server_with_acknowledgement(
        &mut self,
        envelope: CommandEnvelope,
    ) -> SubmissionReceipt {
        self.submit_from(envelope, true)
    }

    fn submit_from(
        &mut self,
        envelope: CommandEnvelope,
        allow_reserved_command_id: bool,
    ) -> SubmissionReceipt {
        let echoed_id = valid_command_id(&envelope.command_id).then(|| envelope.command_id.clone());
        if envelope.version != PROTOCOL_VERSION {
            return self.receipt(
                CommandOutcome::Rejected {
                    error: self.error(
                        echoed_id,
                        ProtocolErrorCode::UnsupportedVersion,
                        format!(
                            "protocol version {} is unsupported; expected {}",
                            envelope.version, PROTOCOL_VERSION
                        ),
                    ),
                },
                AcknowledgementDelivery::Processed,
            );
        }
        if echoed_id.is_none() {
            return self.receipt(
                CommandOutcome::Rejected {
                    error: self.error(
                        None,
                        ProtocolErrorCode::InvalidCommandId,
                        format!(
                            "command ID must contain 1 to {MAX_COMMAND_ID_BYTES} ASCII letters, digits, '-' or '_'"
                        ),
                    ),
                },
                AcknowledgementDelivery::Processed,
            );
        }
        if !allow_reserved_command_id && reserved_server_command_id(&envelope.command_id) {
            return self.receipt(
                CommandOutcome::Rejected {
                    error: self.error(
                        echoed_id,
                        ProtocolErrorCode::ReservedCommandId,
                        "command ID uses the reserved server namespace".to_string(),
                    ),
                },
                AcknowledgementDelivery::Processed,
            );
        }
        if envelope.table_id != self.table_id {
            return self.receipt(
                CommandOutcome::Rejected {
                    error: self.error(
                        echoed_id,
                        ProtocolErrorCode::WrongTable,
                        "command targets a different table".to_string(),
                    ),
                },
                AcknowledgementDelivery::Processed,
            );
        }
        if envelope.hand_id != self.hand_id {
            return self.receipt(
                CommandOutcome::Rejected {
                    error: self.error(
                        echoed_id,
                        ProtocolErrorCode::WrongHand,
                        "command targets a different hand".to_string(),
                    ),
                },
                AcknowledgementDelivery::Processed,
            );
        }

        let fingerprint = CommandFingerprint {
            expected_revision: envelope.expected_revision,
            payload: envelope.payload.clone(),
        };
        if let Some(recorded) = self.recorded_commands.get(&envelope.command_id) {
            if recorded.fingerprint == fingerprint {
                return self.receipt(recorded.outcome.clone(), AcknowledgementDelivery::Replayed);
            }
            return self.receipt(
                CommandOutcome::Rejected {
                    error: self.error(
                        echoed_id,
                        ProtocolErrorCode::CommandIdConflict,
                        "command ID was already used for different intent".to_string(),
                    ),
                },
                AcknowledgementDelivery::Processed,
            );
        }
        if self.recorded_commands.len() >= MAX_RECORDED_COMMANDS_PER_HAND {
            return self.receipt(
                CommandOutcome::Rejected {
                    error: self.error(
                        echoed_id,
                        ProtocolErrorCode::CommandLedgerFull,
                        "command ledger is full for this hand".to_string(),
                    ),
                },
                AcknowledgementDelivery::Processed,
            );
        }

        let outcome = if envelope.expected_revision != self.revision {
            CommandOutcome::Rejected {
                error: self.error(
                    echoed_id,
                    ProtocolErrorCode::StaleRevision,
                    format!(
                        "expected revision {} does not match authoritative revision {}",
                        envelope.expected_revision, self.revision
                    ),
                ),
            }
        } else {
            let action_phase = self.hand.phase;
            let result = match envelope.payload {
                CommandPayload::Act { seat, action } => {
                    self.hand.apply_command(SeatCommand::new(seat, action))
                }
                CommandPayload::ShowdownPreference { seat, always_show } => {
                    self.hand.set_always_show(seat, always_show)
                }
            };
            match result {
                Ok(()) => {
                    self.revision += 1;
                    CommandOutcome::Accepted {
                        event: EventEnvelope {
                            version: PROTOCOL_VERSION,
                            command_id: envelope.command_id.clone(),
                            table_id: self.table_id,
                            hand_id: self.hand_id,
                            revision: self.revision,
                            event: match envelope.payload {
                                CommandPayload::Act { seat, action } => {
                                    TableEvent::ActionAccepted {
                                        seat,
                                        action,
                                        phase: action_phase,
                                        next_to_act: self.hand.to_act,
                                        pot_total: projection_pot_total(&self.hand),
                                    }
                                }
                                CommandPayload::ShowdownPreference { seat, .. } => {
                                    TableEvent::ShowdownPreferenceAccepted { seat }
                                }
                            },
                        },
                    }
                }
                Err(error) => CommandOutcome::Rejected {
                    error: self.error(echoed_id, domain_error_code(&error), error.to_string()),
                },
            }
        };
        self.recorded_commands.insert(
            envelope.command_id,
            RecordedCommand {
                fingerprint,
                outcome: outcome.clone(),
            },
        );
        self.receipt(outcome, AcknowledgementDelivery::Processed)
    }

    pub fn snapshot(
        &self,
        audience: ProjectionAudience,
    ) -> Result<SnapshotEnvelope, ProjectionError> {
        Ok(SnapshotEnvelope {
            version: PROTOCOL_VERSION,
            table_id: self.table_id,
            hand_id: self.hand_id,
            revision: self.revision,
            snapshot: project_hand(&self.hand, self.hand_id, audience)?,
        })
    }

    fn error(
        &self,
        command_id: Option<String>,
        code: ProtocolErrorCode,
        message: String,
    ) -> ErrorEnvelope {
        ErrorEnvelope {
            version: PROTOCOL_VERSION,
            command_id,
            table_id: self.table_id,
            hand_id: self.hand_id,
            revision: self.revision,
            error: PublicProtocolError {
                code,
                message: bounded_public_message(message),
            },
        }
    }

    fn receipt(
        &self,
        outcome: CommandOutcome,
        delivery: AcknowledgementDelivery,
    ) -> SubmissionReceipt {
        let result = match &outcome {
            CommandOutcome::Accepted { .. } => AcknowledgementResult::Accepted,
            CommandOutcome::Rejected { .. } => AcknowledgementResult::Rejected,
        };
        SubmissionReceipt {
            acknowledgement: AcknowledgementEnvelope {
                version: PROTOCOL_VERSION,
                command_id: outcome.command_id(),
                table_id: self.table_id,
                hand_id: self.hand_id,
                revision: outcome.revision(),
                result,
                delivery,
            },
            outcome,
        }
    }
}

pub fn decode_command_json(bytes: &[u8]) -> Result<CommandEnvelope, DecodeCommandError> {
    if bytes.len() > MAX_COMMAND_ENVELOPE_BYTES {
        return Err(DecodeCommandError {
            code: ProtocolErrorCode::MessageTooLarge,
            message: format!("command message exceeds the {MAX_COMMAND_ENVELOPE_BYTES}-byte limit"),
        });
    }
    if bytes.is_empty() {
        return Err(malformed_command_error());
    }
    serde_json::from_slice(bytes).map_err(|_| malformed_command_error())
}

fn malformed_command_error() -> DecodeCommandError {
    DecodeCommandError {
        code: ProtocolErrorCode::MalformedMessage,
        message: format!(
            "command JSON is malformed or does not match protocol v{PROTOCOL_VERSION}"
        ),
    }
}

fn bounded_public_message(message: String) -> String {
    if message.len() <= MAX_PUBLIC_ERROR_MESSAGE_BYTES {
        return message;
    }
    let mut boundary = MAX_PUBLIC_ERROR_MESSAGE_BYTES;
    while !message.is_char_boundary(boundary) {
        boundary -= 1;
    }
    message[..boundary].to_string()
}

pub fn project_hand(
    hand: &MultiwayHand,
    hand_id: HandId,
    audience: ProjectionAudience,
) -> Result<TableProjection, ProjectionError> {
    if let ProjectionAudience::Player(player_seat) = audience {
        if !hand.occupied_seats().any(|seat| seat == player_seat) {
            return Err(ProjectionError::PlayerSeatNotOccupied(player_seat));
        }
    }

    let terminal = matches!(
        hand.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    );
    let contribution_for = |seat: SeatId| {
        if terminal {
            hand.settled_contributions
                .iter()
                .find(|entry| entry.seat == seat)
                .map_or((0, 0), |entry| (0, entry.amount))
        } else {
            let state = hand.seat(seat);
            (state.street_contribution, state.hand_contribution)
        }
    };
    let publicly_revealed = |seat: SeatId| {
        hand.revealed_hands
            .iter()
            .any(|revealed| revealed.seat == seat)
    };
    let seats = hand
        .occupied_seats()
        .map(|seat| {
            let state = hand.seat(seat);
            let (street_contribution, hand_contribution) = contribution_for(seat);
            let viewer_owns_seat = audience == ProjectionAudience::Player(seat);
            ProjectedSeat {
                seat,
                stack: state.stack,
                street_contribution,
                hand_contribution,
                participation: state.participation,
                hole_cards: (viewer_owns_seat || publicly_revealed(seat))
                    .then(|| state.hole_cards.clone()),
            }
        })
        .collect();
    let legal_actions = match audience {
        ProjectionAudience::Player(seat) => hand.legal_actions_for(seat),
        ProjectionAudience::Spectator => None,
    };

    Ok(TableProjection {
        showdown: hand.showdown_progress.clone(),
        mucked: hand.mucked_hands.clone(),
        shown: hand.revealed_hands.iter().map(|shown| shown.seat).collect(),
        always_show: match audience {
            ProjectionAudience::Player(seat) => hand.always_show.contains(&seat),
            ProjectionAudience::Spectator => false,
        },
        hand_id,
        audience: match audience {
            ProjectionAudience::Player(seat) => ProjectionKind::Player { seat },
            ProjectionAudience::Spectator => ProjectionKind::Spectator,
        },
        table_size: hand.table_size,
        phase: hand.phase,
        button: hand.button,
        small_blind: hand.small_blind,
        big_blind: hand.big_blind,
        small_blind_amount: hand.blind_values.small_blind,
        big_blind_amount: hand.blind_values.big_blind,
        ante_amount: hand.blind_values.ante,
        to_act: hand.to_act,
        board: hand.board.clone(),
        current_wager: hand.current_wager,
        pot_total: projection_pot_total(hand),
        seats,
        pots: hand.pots.clone(),
        awards: hand.awards.clone(),
        legal_actions,
    })
}

fn projection_pot_total(hand: &MultiwayHand) -> u32 {
    match hand.phase {
        MultiwayPhase::Showdown => hand.pots.iter().map(|pot| pot.amount).sum(),
        MultiwayPhase::HandComplete => hand.awards.iter().map(|award| award.amount).sum(),
        _ => hand
            .occupied_seats()
            .map(|seat| hand.seat(seat).hand_contribution)
            .sum(),
    }
}

fn valid_command_id(command_id: &str) -> bool {
    !command_id.is_empty()
        && command_id.len() <= MAX_COMMAND_ID_BYTES
        && command_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn reserved_server_command_id(command_id: &str) -> bool {
    command_id.starts_with("srv-")
}

fn domain_error_code(error: &CommandError) -> ProtocolErrorCode {
    match error {
        CommandError::HandNotActive => ProtocolErrorCode::HandNotActive,
        CommandError::SeatNotOccupied(_) => ProtocolErrorCode::SeatNotOccupied,
        CommandError::SeatNotEligible(_) => ProtocolErrorCode::SeatNotEligible,
        CommandError::OutOfTurn { .. } => ProtocolErrorCode::OutOfTurn,
        CommandError::IllegalAction(_) => ProtocolErrorCode::IllegalAction,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deck::Card;

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    fn four_handed_authority() -> ProtocolAuthority {
        let hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(4).unwrap(),
            seat(0),
            &[
                (seat(0), 40),
                (seat(1), 100),
                (seat(2), 200),
                (seat(3), 200),
            ],
            13,
        )
        .unwrap();
        ProtocolAuthority::new(TableId(44), HandId(1), hand)
    }

    fn private_card_json(card: Card) -> String {
        serde_json::to_string(&card).unwrap()
    }

    #[test]
    fn player_and_spectator_snapshots_are_distinct_and_private() {
        let authority = four_handed_authority();
        let player_zero = authority
            .snapshot(ProjectionAudience::Player(seat(0)))
            .unwrap();
        let player_one = authority
            .snapshot(ProjectionAudience::Player(seat(1)))
            .unwrap();
        let spectator = authority.snapshot(ProjectionAudience::Spectator).unwrap();

        assert!(player_zero.snapshot.seats[0].hole_cards.is_some());
        assert!(player_zero.snapshot.seats[1].hole_cards.is_none());
        assert!(player_one.snapshot.seats[0].hole_cards.is_none());
        assert!(player_one.snapshot.seats[1].hole_cards.is_some());
        assert!(spectator
            .snapshot
            .seats
            .iter()
            .all(|seat| seat.hole_cards.is_none()));
        assert!(spectator.snapshot.legal_actions.is_none());

        let player_zero_json = serde_json::to_string(&player_zero).unwrap();
        for hidden in &authority.hand().seat(seat(1)).hole_cards {
            assert!(!player_zero_json.contains(&private_card_json(*hidden)));
        }
        assert_no_sensitive_field_names(&player_zero_json);
        assert_no_sensitive_field_names(&serde_json::to_string(&spectator).unwrap());
    }

    #[test]
    fn accepted_command_advances_revision_once_and_emits_public_event() {
        let mut authority = four_handed_authority();
        let event = authority
            .submit(CommandEnvelope::act(
                "cmd-0001",
                TableId(44),
                0,
                seat(3),
                Action::AllIn(200),
            ))
            .unwrap();
        assert_eq!(authority.revision(), 1);
        assert_eq!(event.revision, 1);
        assert_eq!(event.command_id, "cmd-0001");
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("action_accepted"));
        assert_no_sensitive_field_names(&json);
    }

    #[test]
    fn boundary_rejections_do_not_advance_revision_or_mutate_hand() {
        let cases = [
            CommandEnvelope {
                version: 999,
                ..CommandEnvelope::act("cmd-version", TableId(44), 0, seat(3), Action::AllIn(200))
            },
            CommandEnvelope::act("bad id!", TableId(44), 0, seat(3), Action::AllIn(200)),
            CommandEnvelope::act("cmd-table", TableId(45), 0, seat(3), Action::AllIn(200)),
            CommandEnvelope::act("cmd-stale", TableId(44), 7, seat(3), Action::AllIn(200)),
            CommandEnvelope::act("cmd-turn", TableId(44), 0, seat(0), Action::AllIn(40)),
        ];
        for command in cases {
            let mut authority = four_handed_authority();
            let before = serde_json::to_string(
                &authority
                    .snapshot(ProjectionAudience::Player(seat(0)))
                    .unwrap(),
            )
            .unwrap();
            assert!(authority.submit(command).is_err());
            let after = serde_json::to_string(
                &authority
                    .snapshot(ProjectionAudience::Player(seat(0)))
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(authority.revision(), 0);
            assert_eq!(before, after);
        }
    }

    #[test]
    fn stale_error_echoes_current_revision_without_private_state() {
        let mut authority = four_handed_authority();
        let error = authority
            .submit(CommandEnvelope::act(
                "cmd-stale",
                TableId(44),
                99,
                seat(3),
                Action::AllIn(200),
            ))
            .unwrap_err();
        assert_eq!(error.error.code, ProtocolErrorCode::StaleRevision);
        assert_eq!(error.revision, 0);
        assert_eq!(error.command_id.as_deref(), Some("cmd-stale"));
        assert_no_sensitive_field_names(&serde_json::to_string(&error).unwrap());
    }

    #[test]
    fn showdown_reveals_only_authoritative_public_hands() {
        let mut authority = four_handed_authority();
        for (index, (seat_index, amount)) in [(3, 200), (0, 40), (1, 100), (2, 200)]
            .into_iter()
            .enumerate()
        {
            authority
                .submit(CommandEnvelope::act(
                    format!("cmd-{index}"),
                    TableId(44),
                    index as u64,
                    seat(seat_index),
                    Action::AllIn(amount),
                ))
                .unwrap();
        }
        let spectator = authority.snapshot(ProjectionAudience::Spectator).unwrap();
        assert_eq!(spectator.revision, 4);
        assert_eq!(spectator.snapshot.phase, MultiwayPhase::Showdown);
        assert!(spectator
            .snapshot
            .seats
            .iter()
            .all(|seat| seat.hole_cards.is_some()));
        assert_eq!(spectator.snapshot.pots.len(), 3);
        assert_eq!(spectator.snapshot.awards.len(), 3);
    }

    #[test]
    fn folded_hand_stays_private_to_spectators_after_completion() {
        let hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(2).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100)],
            22,
        )
        .unwrap();
        let mut authority = ProtocolAuthority::new(TableId(1), HandId(2), hand);
        let actor = authority.hand().to_act.unwrap();
        authority
            .submit(CommandEnvelope::act_for_hand(
                "fold-1",
                TableId(1),
                HandId(2),
                0,
                actor,
                Action::Fold,
            ))
            .unwrap();
        let spectator = authority.snapshot(ProjectionAudience::Spectator).unwrap();
        assert_eq!(spectator.snapshot.phase, MultiwayPhase::HandComplete);
        assert!(spectator
            .snapshot
            .seats
            .iter()
            .all(|seat| seat.hole_cards.is_none()));
    }

    #[test]
    fn exact_accepted_retry_replays_original_outcome_without_second_mutation() {
        let mut authority = four_handed_authority();
        let command = CommandEnvelope::act(
            "retry-accepted",
            TableId(44),
            0,
            seat(3),
            Action::AllIn(200),
        );
        let first = authority.submit_with_acknowledgement(command.clone());
        let after_first = authority
            .snapshot(ProjectionAudience::Player(seat(0)))
            .unwrap();
        let retry = authority.submit_with_acknowledgement(command);

        assert_eq!(
            first.acknowledgement.delivery,
            AcknowledgementDelivery::Processed
        );
        assert_eq!(
            retry.acknowledgement.delivery,
            AcknowledgementDelivery::Replayed
        );
        assert_eq!(first.outcome, retry.outcome);
        assert_eq!(authority.revision(), 1);
        assert_eq!(authority.hand().action_history.len(), 1);
        assert_eq!(authority.recorded_command_count(), 1);
        assert_eq!(
            after_first,
            authority
                .snapshot(ProjectionAudience::Player(seat(0)))
                .unwrap()
        );
    }

    #[test]
    fn exact_rejected_retry_returns_original_rejection_after_state_advances() {
        let mut authority = four_handed_authority();
        let rejected =
            CommandEnvelope::act("retry-rejected", TableId(44), 0, seat(0), Action::AllIn(40));
        let first = authority.submit_with_acknowledgement(rejected.clone());
        authority
            .submit(CommandEnvelope::act(
                "accepted-after-rejection",
                TableId(44),
                0,
                seat(3),
                Action::AllIn(200),
            ))
            .unwrap();
        let retry = authority.submit_with_acknowledgement(rejected);

        assert_eq!(first.outcome, retry.outcome);
        assert_eq!(first.acknowledgement.revision, 0);
        assert_eq!(retry.acknowledgement.revision, 0);
        assert_eq!(
            retry.acknowledgement.delivery,
            AcknowledgementDelivery::Replayed
        );
        assert_eq!(authority.revision(), 1);
        assert_eq!(authority.hand().action_history.len(), 1);
    }

    #[test]
    fn conflicting_command_id_reuse_fails_closed() {
        let mut authority = four_handed_authority();
        authority
            .submit(CommandEnvelope::act(
                "conflict-id",
                TableId(44),
                0,
                seat(3),
                Action::AllIn(200),
            ))
            .unwrap();
        let before = authority
            .snapshot(ProjectionAudience::Player(seat(0)))
            .unwrap();
        let conflict = authority.submit_with_acknowledgement(CommandEnvelope::act(
            "conflict-id",
            TableId(44),
            0,
            seat(0),
            Action::AllIn(40),
        ));

        assert_eq!(
            outcome_error_code(&conflict.outcome),
            Some(ProtocolErrorCode::CommandIdConflict)
        );
        assert_eq!(conflict.acknowledgement.revision, 1);
        assert_eq!(authority.revision(), 1);
        assert_eq!(authority.recorded_command_count(), 1);
        assert_eq!(
            before,
            authority
                .snapshot(ProjectionAudience::Player(seat(0)))
                .unwrap()
        );
    }

    #[test]
    fn full_command_ledger_refuses_new_ids_without_evicting_retry_protection() {
        let mut authority = four_handed_authority();
        let first = CommandEnvelope::act("stale-000", TableId(44), 99, seat(3), Action::AllIn(200));
        for index in 0..MAX_RECORDED_COMMANDS_PER_HAND {
            let command = CommandEnvelope::act(
                format!("stale-{index:03}"),
                TableId(44),
                99,
                seat(3),
                Action::AllIn(200),
            );
            assert!(matches!(
                authority.submit_with_acknowledgement(command).outcome,
                CommandOutcome::Rejected { .. }
            ));
        }
        assert_eq!(
            authority.recorded_command_count(),
            MAX_RECORDED_COMMANDS_PER_HAND
        );
        let overflow = authority.submit_with_acknowledgement(CommandEnvelope::act(
            "new-after-full",
            TableId(44),
            0,
            seat(3),
            Action::AllIn(200),
        ));
        assert_eq!(
            outcome_error_code(&overflow.outcome),
            Some(ProtocolErrorCode::CommandLedgerFull)
        );
        let replay = authority.submit_with_acknowledgement(first);
        assert_eq!(
            replay.acknowledgement.delivery,
            AcknowledgementDelivery::Replayed
        );
        assert_eq!(authority.revision(), 0);
    }

    #[test]
    fn bounded_decoder_fails_closed_for_hostile_shapes() {
        let command = CommandEnvelope::act("decode-1", TableId(44), 0, seat(3), Action::AllIn(200));
        let valid = serde_json::to_vec(&command).unwrap();
        assert_eq!(decode_command_json(&valid).unwrap(), command);

        let oversized = vec![b' '; MAX_COMMAND_ENVELOPE_BYTES + 1];
        let too_large = decode_command_json(&oversized).unwrap_err();
        assert_eq!(too_large.code, ProtocolErrorCode::MessageTooLarge);

        let mut unknown_field: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        unknown_field["replacement_state"] = serde_json::json!({"stack": 999999});
        let mut missing_field: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        missing_field.as_object_mut().unwrap().remove("table_id");
        let mut unknown_kind: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        unknown_kind["payload"]["kind"] = serde_json::json!("replace_state");
        let mut invalid_type: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        invalid_type["expected_revision"] = serde_json::json!("zero");
        let duplicate_field = String::from_utf8(valid.clone()).unwrap().replacen(
            &format!("\"version\":{PROTOCOL_VERSION}"),
            &format!("\"version\":{PROTOCOL_VERSION},\"version\":{PROTOCOL_VERSION}"),
            1,
        );
        let hostile = [
            b"{".to_vec(),
            b"{}".to_vec(),
            serde_json::to_vec(&unknown_field).unwrap(),
            serde_json::to_vec(&missing_field).unwrap(),
            serde_json::to_vec(&unknown_kind).unwrap(),
            serde_json::to_vec(&invalid_type).unwrap(),
            duplicate_field.into_bytes(),
        ];
        for bytes in hostile {
            let error = decode_command_json(&bytes).unwrap_err();
            assert_eq!(error.code, ProtocolErrorCode::MalformedMessage);
            assert_eq!(
                error.message,
                format!("command JSON is malformed or does not match protocol v{PROTOCOL_VERSION}")
            );
            assert!(error.message.len() <= MAX_PUBLIC_ERROR_MESSAGE_BYTES);
        }
    }

    #[test]
    fn structurally_valid_future_version_decodes_then_rejects_without_retention() {
        let mut authority = four_handed_authority();
        let command = CommandEnvelope {
            version: PROTOCOL_VERSION + 1,
            ..CommandEnvelope::act("future-v2", TableId(44), 0, seat(3), Action::AllIn(200))
        };
        let decoded = decode_command_json(&serde_json::to_vec(&command).unwrap()).unwrap();
        let receipt = authority.submit_with_acknowledgement(decoded);
        assert_eq!(
            outcome_error_code(&receipt.outcome),
            Some(ProtocolErrorCode::UnsupportedVersion)
        );
        assert_eq!(authority.recorded_command_count(), 0);
        assert_eq!(authority.revision(), 0);
    }

    #[test]
    fn envelopes_round_trip_with_explicit_version_and_kind() {
        let command = CommandEnvelope::act("cmd-42", TableId(9), 3, seat(2), Action::Call(7));
        let json = serde_json::to_string(&command).unwrap();
        assert!(json.contains(&format!("\"version\":{PROTOCOL_VERSION}")));
        assert!(json.contains("\"kind\":\"act\""));
        assert_eq!(
            serde_json::from_str::<CommandEnvelope>(&json).unwrap(),
            command
        );

        let mut authority = four_handed_authority();
        let receipt = authority.submit_with_acknowledgement(CommandEnvelope::act(
            "ack-42",
            TableId(44),
            0,
            seat(3),
            Action::AllIn(200),
        ));
        let receipt_json = serde_json::to_string(&receipt).unwrap();
        assert_eq!(
            serde_json::from_str::<SubmissionReceipt>(&receipt_json).unwrap(),
            receipt
        );
        assert_no_sensitive_field_names(&receipt_json);
    }

    #[test]
    fn hand_identity_is_required_and_wrong_hand_fails_before_retention() {
        let command = CommandEnvelope::act_for_hand(
            "hand-identity",
            TableId(44),
            HandId(2),
            0,
            seat(3),
            Action::AllIn(200),
        );
        let json = serde_json::to_string(&command).unwrap();
        assert!(json.contains("\"hand_id\":2"));
        let mut authority = four_handed_authority();
        let receipt = authority.submit_with_acknowledgement(command);
        assert_eq!(
            outcome_error_code(&receipt.outcome),
            Some(ProtocolErrorCode::WrongHand)
        );
        assert_eq!(receipt.acknowledgement.hand_id, HandId(1));
        assert_eq!(authority.recorded_command_count(), 0);
        assert_eq!(authority.revision(), 0);

        let missing_hand = br#"{
            "version":1,
            "command_id":"missing-hand",
            "table_id":44,
            "expected_revision":0,
            "payload":{"kind":"act","seat":3,"action":{"AllIn":200}}
        }"#;
        assert_eq!(
            decode_command_json(missing_hand).unwrap_err().code,
            ProtocolErrorCode::MalformedMessage
        );
    }

    #[test]
    fn reserved_server_command_namespace_cannot_be_claimed_by_a_client() {
        let command = CommandEnvelope::act_for_hand(
            "srv-timeout-h1-g1",
            TableId(44),
            HandId(1),
            0,
            seat(3),
            Action::AllIn(200),
        );
        let mut authority = four_handed_authority();
        let rejected = authority.submit_with_acknowledgement(command.clone());
        assert_eq!(
            outcome_error_code(&rejected.outcome),
            Some(ProtocolErrorCode::ReservedCommandId)
        );
        assert_eq!(authority.recorded_command_count(), 0);
        assert_eq!(authority.revision(), 0);

        let accepted = authority.submit_server_with_acknowledgement(command);
        assert!(
            matches!(accepted.outcome, CommandOutcome::Accepted { ref event }
            if event.hand_id == HandId(1) && event.revision == 1)
        );
        assert_eq!(accepted.acknowledgement.hand_id, HandId(1));
    }

    fn outcome_error_code(outcome: &CommandOutcome) -> Option<ProtocolErrorCode> {
        match outcome {
            CommandOutcome::Accepted { .. } => None,
            CommandOutcome::Rejected { error } => Some(error.error.code),
        }
    }

    fn assert_no_sensitive_field_names(json: &str) {
        for forbidden in ["deck", "unused", "shuffle", "seed", "rng", "random"] {
            assert!(
                !json.to_ascii_lowercase().contains(forbidden),
                "{forbidden}"
            );
        }
    }
}
