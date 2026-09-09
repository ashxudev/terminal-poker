//! Projection-driven state for a remote terminal client.
//!
//! The client owns presentation state and an authorized protocol projection. It
//! cannot hold or mutate the authoritative hand. Every poker-state change enters
//! through a snapshot, subscription update, or submission response.

use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::authorized_table::{
    ActionDeadline, AuthorizedTableResponse, SubscriptionReason, SubscriptionUpdate,
};
use crate::game::actions::Action;
use crate::game::multiway::MultiwayLegalActions;
use crate::game::seat::SeatId;
use crate::protocol::{
    AcknowledgementResult, CommandEnvelope, HandId, ProjectionKind, SnapshotEnvelope, TableId,
    PROTOCOL_VERSION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientConnectionState {
    Connected,
    AwaitingResynchronization,
    Disconnected,
}

impl ClientConnectionState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connected => "CONNECTED",
            Self::AwaitingResynchronization => "AWAITING RESYNC",
            Self::Disconnected => "DISCONNECTED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCommand {
    pub command_id: String,
    pub expected_revision: u64,
    pub action: Option<Action>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateDisposition {
    Applied,
    DuplicateOrStale,
    ResynchronizationRequired { expected: u64, received: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionClientErrorCode {
    NoSnapshot,
    UnsupportedVersion,
    IdentityChanged,
    AudienceChanged,
    SpectatorCannotAct,
    NotAuthorizedToAct,
    IllegalAction,
    CommandPending,
    AwaitingResynchronization,
    Disconnected,
    ResponseCommandMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionClientError {
    pub code: ProjectionClientErrorCode,
    pub message: String,
}

impl ProjectionClientError {
    fn new(code: ProjectionClientErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl Display for ProjectionClientError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.code, self.message)
    }
}

impl Error for ProjectionClientError {}

#[derive(Debug, Clone)]
pub struct ProjectionClient {
    snapshot: SnapshotEnvelope,
    connection: ClientConnectionState,
    pending: Option<PendingCommand>,
    deadline: Option<ActionDeadline>,
    last_stream_sequence: u64,
    received_broadcast: bool,
    activity: Vec<String>,
}

impl ProjectionClient {
    pub fn bootstrap_from_update(
        update: SubscriptionUpdate,
    ) -> Result<Self, ProjectionClientError> {
        let mut client = Self::bootstrap(update.snapshot, update.stream_sequence)?;
        client.deadline = update.deadline;
        client.received_broadcast = !matches!(update.reason, SubscriptionReason::Initial);
        Ok(client)
    }

    pub fn bootstrap(
        snapshot: SnapshotEnvelope,
        stream_sequence: u64,
    ) -> Result<Self, ProjectionClientError> {
        validate_version(&snapshot)?;
        Ok(Self {
            snapshot,
            connection: ClientConnectionState::Connected,
            pending: None,
            deadline: None,
            last_stream_sequence: stream_sequence,
            received_broadcast: false,
            activity: vec![format!(
                "SYNC  initial projection / stream {stream_sequence} / authoritative revision 0"
            )],
        })
    }

    pub const fn snapshot(&self) -> &SnapshotEnvelope {
        &self.snapshot
    }

    pub const fn connection(&self) -> ClientConnectionState {
        self.connection
    }

    pub const fn pending(&self) -> Option<&PendingCommand> {
        self.pending.as_ref()
    }

    pub const fn deadline(&self) -> Option<ActionDeadline> {
        self.deadline
    }

    pub const fn last_stream_sequence(&self) -> u64 {
        self.last_stream_sequence
    }

    pub fn activity(&self) -> &[String] {
        &self.activity
    }

    pub fn controls_enabled(&self) -> bool {
        if self.connection != ClientConnectionState::Connected || self.pending.is_some() {
            return false;
        }
        matches!(self.snapshot.snapshot.audience, ProjectionKind::Player { seat }
            if self.snapshot.snapshot.to_act == Some(seat)
                && self.snapshot.snapshot.legal_actions.is_some())
    }

    pub fn prepare_showdown_preference(
        &mut self,
        command_id: impl Into<String>,
        always_show: bool,
    ) -> Result<CommandEnvelope, ProjectionClientError> {
        if self.connection != ClientConnectionState::Connected || self.pending.is_some() {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::NotAuthorizedToAct,
                "wait for an authoritative connection before changing showdown preference",
            ));
        }
        let ProjectionKind::Player { seat } = self.snapshot.snapshot.audience else {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::SpectatorCannotAct,
                "spectators have no showdown preference",
            ));
        };
        if self.snapshot.snapshot.showdown.is_some()
            || matches!(
                self.snapshot.snapshot.phase,
                crate::game::multiway::MultiwayPhase::Showdown
                    | crate::game::multiway::MultiwayPhase::HandComplete
            )
        {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::NotAuthorizedToAct,
                "select showdown preference before the reveal begins",
            ));
        }
        let command_id = command_id.into();
        let command = CommandEnvelope {
            version: PROTOCOL_VERSION,
            command_id: command_id.clone(),
            table_id: self.snapshot.table_id,
            hand_id: self.snapshot.hand_id,
            expected_revision: self.snapshot.revision,
            payload: crate::protocol::CommandPayload::ShowdownPreference { seat, always_show },
        };
        self.pending = Some(PendingCommand {
            command_id,
            expected_revision: self.snapshot.revision,
            action: None,
        });
        Ok(command)
    }

    pub fn prepare_action(
        &mut self,
        command_id: impl Into<String>,
        action: Action,
    ) -> Result<CommandEnvelope, ProjectionClientError> {
        match self.connection {
            ClientConnectionState::Connected => {}
            ClientConnectionState::AwaitingResynchronization => {
                return Err(ProjectionClientError::new(
                    ProjectionClientErrorCode::AwaitingResynchronization,
                    "authoritative controls remain disabled until a fresh snapshot arrives",
                ))
            }
            ClientConnectionState::Disconnected => {
                return Err(ProjectionClientError::new(
                    ProjectionClientErrorCode::Disconnected,
                    "disconnected clients cannot submit gameplay intentions",
                ))
            }
        }
        if self.pending.is_some() {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::CommandPending,
                "one gameplay command is already awaiting authority",
            ));
        }
        let seat = match self.snapshot.snapshot.audience {
            ProjectionKind::Player { seat } => seat,
            ProjectionKind::Spectator => {
                return Err(ProjectionClientError::new(
                    ProjectionClientErrorCode::SpectatorCannotAct,
                    "spectator projections never enable gameplay controls",
                ))
            }
        };
        if self.snapshot.snapshot.to_act != Some(seat) {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::NotAuthorizedToAct,
                "the latest authoritative projection does not assign action to this seat",
            ));
        }
        let legal = self
            .snapshot
            .snapshot
            .legal_actions
            .as_ref()
            .ok_or_else(|| {
                ProjectionClientError::new(
                    ProjectionClientErrorCode::NotAuthorizedToAct,
                    "the latest projection contains no legal actions for this audience",
                )
            })?;
        if !action_is_legal(legal, action) {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::IllegalAction,
                "the intention is outside the authoritative legal-action bounds",
            ));
        }
        let command_id = command_id.into();
        let command = CommandEnvelope::act_for_hand(
            command_id.clone(),
            self.snapshot.table_id,
            self.snapshot.hand_id,
            self.snapshot.revision,
            seat,
            action,
        );
        self.pending = Some(PendingCommand {
            command_id: command_id.clone(),
            expected_revision: self.snapshot.revision,
            action: Some(action),
        });
        self.activity.push(format!(
            "INTENT  {command_id} / S{} / {:?} / pending revision {}",
            seat.as_u8(),
            action,
            self.snapshot.revision
        ));
        Ok(command)
    }

    pub fn apply_response(
        &mut self,
        response: AuthorizedTableResponse,
    ) -> Result<(), ProjectionClientError> {
        self.validate_identity_and_audience(&response.snapshot)?;
        let pending = self.pending.as_ref().ok_or_else(|| {
            ProjectionClientError::new(
                ProjectionClientErrorCode::ResponseCommandMismatch,
                "received a command response while no command was pending",
            )
        })?;
        if response.receipt.acknowledgement.command_id.as_deref()
            != Some(pending.command_id.as_str())
        {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::ResponseCommandMismatch,
                "the response command ID does not match the pending intention",
            ));
        }
        let accepted = response.receipt.acknowledgement.result == AcknowledgementResult::Accepted;
        let command_id = pending.command_id.clone();
        if response.snapshot.revision >= self.snapshot.revision {
            self.snapshot = response.snapshot;
        }
        self.deadline = response.deadline;
        self.last_stream_sequence = self.last_stream_sequence.max(response.stream_sequence);
        self.pending = None;
        self.activity.push(format!(
            "AUTHORITY  {command_id} / {} / revision {}",
            if accepted { "accepted" } else { "rejected" },
            self.snapshot.revision
        ));
        Ok(())
    }

    pub fn apply_update(
        &mut self,
        update: SubscriptionUpdate,
    ) -> Result<UpdateDisposition, ProjectionClientError> {
        self.validate_identity_and_audience(&update.snapshot)?;
        if update.stream_sequence <= self.last_stream_sequence {
            self.activity.push(format!(
                "STREAM  ignored duplicate/stale sequence {}",
                update.stream_sequence
            ));
            return Ok(UpdateDisposition::DuplicateOrStale);
        }
        if self.received_broadcast && update.stream_sequence != self.last_stream_sequence + 1 {
            let expected = self.last_stream_sequence + 1;
            let received = update.stream_sequence;
            self.connection = ClientConnectionState::AwaitingResynchronization;
            self.pending = None;
            self.activity.push(format!(
                "STREAM  gap / expected {expected} / received {received} / resync required"
            ));
            return Ok(UpdateDisposition::ResynchronizationRequired { expected, received });
        }
        if update.snapshot.revision < self.snapshot.revision {
            self.activity.push(format!(
                "STREAM  ignored revision regression {} < {}",
                update.snapshot.revision, self.snapshot.revision
            ));
            self.last_stream_sequence = update.stream_sequence;
            self.received_broadcast = true;
            return Ok(UpdateDisposition::DuplicateOrStale);
        }
        if let Some(event) = &update.event {
            if self.pending.as_ref().is_some_and(|pending| {
                pending.command_id == event.command_id || event.revision > pending.expected_revision
            }) {
                self.pending = None;
            }
        } else if update.snapshot.revision > self.snapshot.revision {
            self.pending = None;
        }
        self.snapshot = update.snapshot;
        self.deadline = update.deadline;
        self.last_stream_sequence = update.stream_sequence;
        self.received_broadcast = !matches!(update.reason, SubscriptionReason::Initial);
        self.connection = ClientConnectionState::Connected;
        self.activity.push(format!(
            "STREAM  sequence {} / revision {} / {}",
            self.last_stream_sequence,
            self.snapshot.revision,
            reason_label(&update.reason)
        ));
        Ok(UpdateDisposition::Applied)
    }

    pub fn resynchronize(
        &mut self,
        snapshot: SnapshotEnvelope,
        stream_sequence: u64,
    ) -> Result<(), ProjectionClientError> {
        self.validate_identity_and_audience(&snapshot)?;
        if snapshot.revision < self.snapshot.revision {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::IdentityChanged,
                "resynchronization cannot regress the authoritative revision",
            ));
        }
        self.snapshot = snapshot;
        self.last_stream_sequence = stream_sequence;
        self.received_broadcast = true;
        self.pending = None;
        self.connection = ClientConnectionState::Connected;
        self.activity.push(format!(
            "SYNC  restored at stream {stream_sequence} / revision {}",
            self.snapshot.revision
        ));
        Ok(())
    }

    pub fn resynchronize_from_update(
        &mut self,
        update: SubscriptionUpdate,
    ) -> Result<(), ProjectionClientError> {
        let deadline = update.deadline;
        self.resynchronize(update.snapshot, update.stream_sequence)?;
        self.deadline = deadline;
        Ok(())
    }

    pub fn mark_disconnected(&mut self) {
        self.connection = ClientConnectionState::Disconnected;
        self.pending = None;
        self.activity
            .push("CONNECTION  disconnected / controls disabled".to_string());
    }

    fn validate_identity_and_audience(
        &self,
        snapshot: &SnapshotEnvelope,
    ) -> Result<(), ProjectionClientError> {
        validate_version(snapshot)?;
        if snapshot.table_id != self.snapshot.table_id || snapshot.hand_id != self.snapshot.hand_id
        {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::IdentityChanged,
                "authoritative table or hand identity changed without a new client session",
            ));
        }
        if snapshot.snapshot.audience != self.snapshot.snapshot.audience {
            return Err(ProjectionClientError::new(
                ProjectionClientErrorCode::AudienceChanged,
                "a client cannot change its authorized projection audience",
            ));
        }
        Ok(())
    }
}

fn validate_version(snapshot: &SnapshotEnvelope) -> Result<(), ProjectionClientError> {
    if snapshot.version != PROTOCOL_VERSION {
        return Err(ProjectionClientError::new(
            ProjectionClientErrorCode::UnsupportedVersion,
            "the client supports only the current protocol version",
        ));
    }
    Ok(())
}

fn action_is_legal(legal: &MultiwayLegalActions, action: Action) -> bool {
    match action {
        Action::Fold => legal.can_fold,
        Action::Check => legal.can_check,
        Action::Call(amount) => legal.call_amount == Some(amount),
        Action::Bet(amount) => legal
            .min_bet_to
            .is_some_and(|minimum| amount >= minimum && amount < legal.all_in_to),
        Action::Raise(amount) => legal
            .min_raise_to
            .is_some_and(|minimum| amount >= minimum && amount < legal.all_in_to),
        Action::AllIn(amount) => amount == legal.all_in_to,
    }
}

fn reason_label(reason: &SubscriptionReason) -> &'static str {
    match reason {
        SubscriptionReason::Initial => "initial",
        SubscriptionReason::ActionAccepted => "action accepted",
        SubscriptionReason::DeadlineWarning { .. } => "deadline warning",
        SubscriptionReason::TimeoutAction { .. } => "timeout action",
        SubscriptionReason::ConnectionStateChanged { .. } => "connection changed",
    }
}

pub fn passive_action(legal: &MultiwayLegalActions) -> Action {
    if legal.can_check {
        Action::Check
    } else if let Some(amount) = legal.call_amount {
        Action::Call(amount)
    } else if legal.can_fold {
        Action::Fold
    } else {
        Action::AllIn(legal.all_in_to)
    }
}

pub fn all_in_action(legal: &MultiwayLegalActions) -> Action {
    Action::AllIn(legal.all_in_to)
}

pub fn snapshot_identity(snapshot: &SnapshotEnvelope) -> (TableId, HandId) {
    (snapshot.table_id, snapshot.hand_id)
}

pub fn player_seat(snapshot: &SnapshotEnvelope) -> Option<SeatId> {
    match snapshot.snapshot.audience {
        ProjectionKind::Player { seat } => Some(seat),
        ProjectionKind::Spectator => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::authorized_table::{AuthorizedTableRuntime, GuestSessionId, SessionRole};
    use crate::game::multiway::MultiwayHand;
    use crate::game::seat::TableSize;
    use crate::protocol::{ProtocolAuthority, TableId};

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    fn session(index: u8) -> GuestSessionId {
        GuestSessionId::new(format!("client-{index}")).unwrap()
    }

    fn runtime() -> (
        AuthorizedTableRuntime,
        crate::authorized_table::AuthorizedTableHandle,
    ) {
        let hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(3).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100), (seat(2), 100)],
            7,
        )
        .unwrap();
        let runtime =
            AuthorizedTableRuntime::spawn(ProtocolAuthority::new(TableId(7), HandId(1), hand))
                .unwrap();
        let handle = runtime.handle();
        for index in 0..3 {
            handle
                .bind(
                    session(index),
                    TableId(7),
                    HandId(1),
                    SessionRole::Player { seat: seat(index) },
                )
                .unwrap();
        }
        (runtime, handle)
    }

    #[test]
    fn client_prepares_only_authoritative_legal_actions_and_waits_for_response() {
        let (_runtime, handle) = runtime();
        let actor = handle
            .snapshot(session(0))
            .unwrap()
            .snapshot
            .to_act
            .unwrap();
        let snapshot = handle.snapshot(session(actor.as_u8())).unwrap();
        let mut client = ProjectionClient::bootstrap(snapshot, 0).unwrap();
        let legal = client.snapshot().snapshot.legal_actions.as_ref().unwrap();
        let action = passive_action(legal);
        let command = client.prepare_action("client-command", action).unwrap();
        assert!(!client.controls_enabled());
        assert_eq!(
            client.prepare_action("second", action).unwrap_err().code,
            ProjectionClientErrorCode::CommandPending
        );
        let response = handle.submit(session(actor.as_u8()), command).unwrap();
        client.apply_response(response).unwrap();
        assert!(client.pending().is_none());
        assert_eq!(client.snapshot().revision, 1);
    }

    #[test]
    fn duplicate_stale_and_gapped_output_never_optimistically_mutate() {
        let (_runtime, handle) = runtime();
        let actor = handle
            .snapshot(session(0))
            .unwrap()
            .snapshot
            .to_act
            .unwrap();
        let subscription = handle.subscribe(session(actor.as_u8())).unwrap();
        let initial = subscription.recv().unwrap();
        let mut client =
            ProjectionClient::bootstrap(initial.snapshot.clone(), initial.stream_sequence).unwrap();
        let action = passive_action(client.snapshot().snapshot.legal_actions.as_ref().unwrap());
        let command = client.prepare_action("first", action).unwrap();
        handle.submit(session(actor.as_u8()), command).unwrap();
        let update = subscription.recv().unwrap();
        assert_eq!(
            client.apply_update(update.clone()).unwrap(),
            UpdateDisposition::Applied
        );
        let accepted = client.snapshot().clone();
        assert_eq!(
            client.apply_update(update.clone()).unwrap(),
            UpdateDisposition::DuplicateOrStale
        );
        assert_eq!(client.snapshot(), &accepted);

        let mut gap = update;
        gap.stream_sequence += 2;
        assert!(matches!(
            client.apply_update(gap).unwrap(),
            UpdateDisposition::ResynchronizationRequired { .. }
        ));
        assert_eq!(client.snapshot(), &accepted);
        assert_eq!(
            client.connection(),
            ClientConnectionState::AwaitingResynchronization
        );
        let current = handle.snapshot(session(actor.as_u8())).unwrap();
        client
            .resynchronize(current, handle.metrics().unwrap().stream_sequence)
            .unwrap();
        assert_eq!(client.connection(), ClientConnectionState::Connected);
    }

    #[test]
    fn audience_and_identity_cannot_change_inside_one_client() {
        let (_runtime, handle) = runtime();
        let snapshot = handle.snapshot(session(0)).unwrap();
        let mut client = ProjectionClient::bootstrap(snapshot.clone(), 0).unwrap();
        let mut wrong = snapshot;
        wrong.snapshot.audience = ProjectionKind::Player { seat: seat(1) };
        assert_eq!(
            client.resynchronize(wrong, 0).unwrap_err().code,
            ProjectionClientErrorCode::AudienceChanged
        );
    }
}
