//! Transport-neutral authorization, authoritative logical time, and private subscriptions.
//!
//! The runtime is the remote-safe seam in front of `TableActor`. Callers provide
//! an opaque server-issued session identifier; the runtime derives table, hand,
//! seat, and audience from its private binding registry. Poker mutation remains
//! exclusively owned by the inner serialized table actor.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};

use crate::game::actions::Action;
use crate::game::seat::SeatId;
use crate::protocol::{
    AcknowledgementDelivery, AcknowledgementResult, CommandEnvelope, EventEnvelope, HandId,
    ProjectionAudience, ProtocolAuthority, SnapshotEnvelope, SubmissionReceipt, TableId,
};
use crate::table_actor::{TableActor, TableActorError, TableActorHandle, TableActorMetrics};

pub const AUTHORIZED_RUNTIME_MAILBOX_CAPACITY: usize = 64;
pub const SUBSCRIPTION_BUFFER_CAPACITY: usize = 4;
pub const ACTION_TIMEOUT_TICKS: u64 = 60;
pub const ACTION_WARNING_TICKS: u64 = 10;
pub const MAX_GUEST_SESSION_ID_BYTES: usize = 64;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GuestSessionId(String);

impl GuestSessionId {
    pub fn new(value: impl Into<String>) -> Result<Self, AuthorizedTableError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > MAX_GUEST_SESSION_ID_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(AuthorizedTableError::new(
                AuthorizedTableErrorCode::InvalidSessionId,
                format!(
                    "guest session ID must contain 1 to {MAX_GUEST_SESSION_ID_BYTES} safe ASCII bytes"
                ),
            ));
        }
        Ok(Self(value))
    }

    pub fn random() -> Self {
        let mut bytes = [0u8; 16];
        OsRng.fill_bytes(&mut bytes);
        let mut value = String::with_capacity(42);
        value.push_str("principal-");
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut value, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(value)
    }

    pub(crate) fn stable_value(&self) -> &str {
        &self.0
    }
}

impl Debug for GuestSessionId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("GuestSessionId(<redacted>)")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionRole {
    Player { seat: SeatId },
    Spectator,
}

impl SessionRole {
    const fn audience(self) -> ProjectionAudience {
        match self {
            Self::Player { seat } => ProjectionAudience::Player(seat),
            Self::Spectator => ProjectionAudience::Spectator,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorizedTableErrorCode {
    InvalidSessionId,
    UnknownSession,
    SessionAlreadyBound,
    SeatAlreadyOwned,
    SessionDisconnected,
    SpectatorCannotAct,
    UnauthorizedSeat,
    WrongTable,
    WrongHand,
    SubscriptionAlreadyExists,
    ClockRegression,
    DeadlineUnavailable,
    RuntimeClosed,
    InternalAuthorityFailure,
}

impl AuthorizedTableErrorCode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::InvalidSessionId => "invalid_session_id",
            Self::UnknownSession => "unknown_session",
            Self::SessionAlreadyBound => "session_already_bound",
            Self::SeatAlreadyOwned => "seat_already_owned",
            Self::SessionDisconnected => "session_disconnected",
            Self::SpectatorCannotAct => "spectator_cannot_act",
            Self::UnauthorizedSeat => "unauthorized_seat",
            Self::WrongTable => "wrong_table",
            Self::WrongHand => "wrong_hand",
            Self::SubscriptionAlreadyExists => "subscription_already_exists",
            Self::ClockRegression => "clock_regression",
            Self::DeadlineUnavailable => "deadline_unavailable",
            Self::RuntimeClosed => "runtime_closed",
            Self::InternalAuthorityFailure => "internal_authority_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedTableError {
    pub code: AuthorizedTableErrorCode,
    pub message: String,
}

impl AuthorizedTableError {
    fn new(code: AuthorizedTableErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn runtime_closed() -> Self {
        Self::new(
            AuthorizedTableErrorCode::RuntimeClosed,
            "authorized table runtime is closed",
        )
    }

    fn authority_failure() -> Self {
        Self::new(
            AuthorizedTableErrorCode::InternalAuthorityFailure,
            "table authority could not complete the request",
        )
    }
}

impl Display for AuthorizedTableError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.name(), self.message)
    }
}

impl Error for AuthorizedTableError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionDeadline {
    pub seat: SeatId,
    pub warning_tick: u64,
    pub due_tick: u64,
    pub warning_emitted: bool,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubscriptionReason {
    Initial,
    ActionAccepted,
    DeadlineWarning {
        seat: SeatId,
        remaining_ticks: u64,
    },
    TimeoutAction {
        seat: SeatId,
        action: Action,
    },
    ConnectionStateChanged {
        seat: Option<SeatId>,
        connected: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionUpdate {
    pub stream_sequence: u64,
    pub reason: SubscriptionReason,
    pub event: Option<EventEnvelope>,
    pub snapshot: SnapshotEnvelope,
    pub deadline: Option<ActionDeadline>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedTableResponse {
    pub receipt: SubmissionReceipt,
    pub snapshot: SnapshotEnvelope,
    pub deadline: Option<ActionDeadline>,
    pub stream_sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TickResult {
    pub now_tick: u64,
    pub warning_emitted: bool,
    pub timeout_event: Option<EventEnvelope>,
    pub timeout_action: Option<Action>,
    pub deadline: Option<ActionDeadline>,
    pub stream_sequence: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedTableMetrics {
    pub actor: TableActorMetrics,
    pub active_bindings: u64,
    pub connected_bindings: u64,
    pub authorization_rejections: u64,
    pub disconnects: u64,
    pub deadline_warnings: u64,
    pub timeout_actions: u64,
    pub subscription_requests: u64,
    pub subscription_deliveries: u64,
    pub slow_subscribers_dropped: u64,
    pub stream_sequence: u64,
    pub now_tick: u64,
}

pub struct AuthorizedTableSubscription {
    receiver: Receiver<SubscriptionUpdate>,
}

impl AuthorizedTableSubscription {
    pub fn recv(&self) -> Result<SubscriptionUpdate, AuthorizedTableError> {
        self.receiver.recv().map_err(|_| {
            AuthorizedTableError::new(
                AuthorizedTableErrorCode::RuntimeClosed,
                "subscription is closed",
            )
        })
    }

    pub fn try_recv(&self) -> Result<SubscriptionUpdate, TryRecvError> {
        self.receiver.try_recv()
    }
}

#[derive(Debug, Clone)]
pub struct AuthorizedTableHandle {
    sender: SyncSender<RuntimeRequest>,
}

impl AuthorizedTableHandle {
    pub fn bind(
        &self,
        session: GuestSessionId,
        table_id: TableId,
        hand_id: HandId,
        role: SessionRole,
    ) -> Result<(), AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::Bind {
            session,
            table_id,
            hand_id,
            role,
            respond_to,
        })?
    }

    pub fn submit(
        &self,
        session: GuestSessionId,
        command: CommandEnvelope,
    ) -> Result<AuthorizedTableResponse, AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::Submit {
            session,
            command,
            respond_to,
        })?
    }

    pub fn snapshot(
        &self,
        session: GuestSessionId,
    ) -> Result<SnapshotEnvelope, AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::Snapshot {
            session,
            respond_to,
        })?
    }

    pub(crate) fn bound_snapshot(
        &self,
        session: GuestSessionId,
    ) -> Result<SnapshotEnvelope, AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::BoundSnapshot {
            session,
            respond_to,
        })?
    }

    pub(crate) fn safe_history_material(
        &self,
    ) -> Result<(SnapshotEnvelope, Vec<EventEnvelope>), AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::SafeHistoryMaterial { respond_to })?
    }

    pub fn subscribe(
        &self,
        session: GuestSessionId,
    ) -> Result<AuthorizedTableSubscription, AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::Subscribe {
            session,
            respond_to,
        })?
    }

    pub fn disconnect(&self, session: GuestSessionId) -> Result<(), AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::Disconnect {
            session,
            respond_to,
        })?
    }

    pub fn reconnect(&self, session: GuestSessionId) -> Result<(), AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::Reconnect {
            session,
            respond_to,
        })?
    }

    pub fn advance_time(&self, now_tick: u64) -> Result<TickResult, AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::AdvanceTime {
            now_tick,
            respond_to,
        })?
    }

    pub fn metrics(&self) -> Result<AuthorizedTableMetrics, AuthorizedTableError> {
        self.request(|respond_to| RuntimeRequest::Metrics { respond_to })
    }

    fn request<T>(
        &self,
        build: impl FnOnce(mpsc::Sender<T>) -> RuntimeRequest,
    ) -> Result<T, AuthorizedTableError> {
        let (response_sender, response_receiver) = mpsc::channel();
        self.sender
            .send(build(response_sender))
            .map_err(|_| AuthorizedTableError::runtime_closed())?;
        response_receiver
            .recv()
            .map_err(|_| AuthorizedTableError::runtime_closed())
    }
}

#[derive(Debug)]
pub struct AuthorizedTableRuntime {
    handle: AuthorizedTableHandle,
    worker: Option<JoinHandle<()>>,
}

impl AuthorizedTableRuntime {
    pub fn spawn(authority: ProtocolAuthority) -> Result<Self, AuthorizedTableError> {
        let table_id = authority.table_id();
        let hand_id = authority.hand_id();
        let table =
            TableActor::spawn(authority).map_err(|_| AuthorizedTableError::authority_failure())?;
        let table_handle = table.handle();
        let (sender, receiver) = mpsc::sync_channel(AUTHORIZED_RUNTIME_MAILBOX_CAPACITY);
        let worker = thread::Builder::new()
            .name(format!("authorized-table-{}", table_id.0))
            .spawn(move || run_runtime(receiver, table, table_handle, table_id, hand_id))
            .map_err(|_| AuthorizedTableError::runtime_closed())?;
        Ok(Self {
            handle: AuthorizedTableHandle { sender },
            worker: Some(worker),
        })
    }

    pub fn handle(&self) -> AuthorizedTableHandle {
        self.handle.clone()
    }

    pub fn shutdown(mut self) -> Result<(), AuthorizedTableError> {
        self.stop_worker()
    }

    fn stop_worker(&mut self) -> Result<(), AuthorizedTableError> {
        if let Some(worker) = self.worker.take() {
            let _ = self.handle.sender.send(RuntimeRequest::Shutdown);
            worker.join().map_err(|_| {
                AuthorizedTableError::new(
                    AuthorizedTableErrorCode::RuntimeClosed,
                    "authorized table worker panicked",
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for AuthorizedTableRuntime {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}

enum RuntimeRequest {
    Bind {
        session: GuestSessionId,
        table_id: TableId,
        hand_id: HandId,
        role: SessionRole,
        respond_to: mpsc::Sender<Result<(), AuthorizedTableError>>,
    },
    Submit {
        session: GuestSessionId,
        command: CommandEnvelope,
        respond_to: mpsc::Sender<Result<AuthorizedTableResponse, AuthorizedTableError>>,
    },
    Snapshot {
        session: GuestSessionId,
        respond_to: mpsc::Sender<Result<SnapshotEnvelope, AuthorizedTableError>>,
    },
    BoundSnapshot {
        session: GuestSessionId,
        respond_to: mpsc::Sender<Result<SnapshotEnvelope, AuthorizedTableError>>,
    },
    SafeHistoryMaterial {
        respond_to:
            mpsc::Sender<Result<(SnapshotEnvelope, Vec<EventEnvelope>), AuthorizedTableError>>,
    },
    Subscribe {
        session: GuestSessionId,
        respond_to: mpsc::Sender<Result<AuthorizedTableSubscription, AuthorizedTableError>>,
    },
    Disconnect {
        session: GuestSessionId,
        respond_to: mpsc::Sender<Result<(), AuthorizedTableError>>,
    },
    Reconnect {
        session: GuestSessionId,
        respond_to: mpsc::Sender<Result<(), AuthorizedTableError>>,
    },
    AdvanceTime {
        now_tick: u64,
        respond_to: mpsc::Sender<Result<TickResult, AuthorizedTableError>>,
    },
    Metrics {
        respond_to: mpsc::Sender<AuthorizedTableMetrics>,
    },
    Shutdown,
}

#[derive(Debug, Clone, Copy)]
struct SessionBinding {
    role: SessionRole,
    connected: bool,
}

struct RuntimeState {
    table: TableActorHandle,
    table_id: TableId,
    hand_id: HandId,
    sessions: BTreeMap<GuestSessionId, SessionBinding>,
    subscribers: BTreeMap<GuestSessionId, SyncSender<SubscriptionUpdate>>,
    now_tick: u64,
    deadline: Option<ActionDeadline>,
    deadline_generation: u64,
    stream_sequence: u64,
    authorization_rejections: u64,
    disconnects: u64,
    deadline_warnings: u64,
    timeout_actions: u64,
    subscription_requests: u64,
    subscription_deliveries: u64,
    slow_subscribers_dropped: u64,
    accepted_events: Vec<EventEnvelope>,
    next_showdown_step: Option<Instant>,
}

fn run_runtime(
    receiver: Receiver<RuntimeRequest>,
    table_actor: TableActor,
    table: TableActorHandle,
    table_id: TableId,
    hand_id: HandId,
) {
    let mut state = RuntimeState {
        table,
        table_id,
        hand_id,
        sessions: BTreeMap::new(),
        subscribers: BTreeMap::new(),
        now_tick: 0,
        deadline: None,
        deadline_generation: 0,
        stream_sequence: 0,
        authorization_rejections: 0,
        disconnects: 0,
        deadline_warnings: 0,
        timeout_actions: 0,
        subscription_requests: 0,
        subscription_deliveries: 0,
        slow_subscribers_dropped: 0,
        accepted_events: Vec::new(),
        next_showdown_step: None,
    };
    let _ = state.reschedule_deadline();

    loop {
        if state.progress_showdown().is_err() {
            break;
        }
        let request = match receiver.recv_timeout(Duration::from_millis(20)) {
            Ok(request) => request,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        match request {
            RuntimeRequest::Bind {
                session,
                table_id,
                hand_id,
                role,
                respond_to,
            } => {
                let _ = respond_to.send(state.bind(session, table_id, hand_id, role));
            }
            RuntimeRequest::Submit {
                session,
                command,
                respond_to,
            } => {
                let _ = respond_to.send(state.submit(session, command));
            }
            RuntimeRequest::Snapshot {
                session,
                respond_to,
            } => {
                let _ = respond_to.send(state.snapshot(session));
            }
            RuntimeRequest::BoundSnapshot {
                session,
                respond_to,
            } => {
                let _ = respond_to.send(state.bound_snapshot(session));
            }
            RuntimeRequest::SafeHistoryMaterial { respond_to } => {
                let _ = respond_to.send(state.safe_history_material());
            }
            RuntimeRequest::Subscribe {
                session,
                respond_to,
            } => {
                let _ = respond_to.send(state.subscribe(session));
            }
            RuntimeRequest::Disconnect {
                session,
                respond_to,
            } => {
                let _ = respond_to.send(state.disconnect(session));
            }
            RuntimeRequest::Reconnect {
                session,
                respond_to,
            } => {
                let _ = respond_to.send(state.reconnect(session));
            }
            RuntimeRequest::AdvanceTime {
                now_tick,
                respond_to,
            } => {
                let _ = respond_to.send(state.advance_time(now_tick));
            }
            RuntimeRequest::Metrics { respond_to } => {
                let _ = respond_to.send(state.metrics());
            }
            RuntimeRequest::Shutdown => break,
        }
    }
    let _ = table_actor.shutdown();
}

impl RuntimeState {
    fn progress_showdown(&mut self) -> Result<(), AuthorizedTableError> {
        // Clock belongs to the server runtime. No terminal or player can
        // accelerate dealing, reveal another hand, or settle a pot.
        let pending = self
            .table
            .snapshot(ProjectionAudience::Spectator)
            .map_err(|_| AuthorizedTableError::authority_failure())?
            .snapshot
            .showdown;
        let Some(_) = pending else {
            self.next_showdown_step = None;
            return Ok(());
        };
        let now = Instant::now();
        let due = self
            .next_showdown_step
            .get_or_insert(now + Duration::from_millis(1_500));
        if now < *due {
            return Ok(());
        }
        if let Some(event) = self
            .table
            .advance_showdown()
            .map_err(|_| AuthorizedTableError::authority_failure())?
        {
            self.accepted_events.push(event.clone());
            self.reschedule_deadline()?;
            self.broadcast(SubscriptionReason::ActionAccepted, Some(event))?;
        }
        self.next_showdown_step = Some(now + Duration::from_millis(1_500));
        Ok(())
    }
    fn bind(
        &mut self,
        session: GuestSessionId,
        table_id: TableId,
        hand_id: HandId,
        role: SessionRole,
    ) -> Result<(), AuthorizedTableError> {
        if table_id != self.table_id {
            return self.authorization_error(
                AuthorizedTableErrorCode::WrongTable,
                "session binding targets a different table",
            );
        }
        if hand_id != self.hand_id {
            return self.authorization_error(
                AuthorizedTableErrorCode::WrongHand,
                "session binding targets a different hand",
            );
        }
        if self.sessions.contains_key(&session) {
            return self.authorization_error(
                AuthorizedTableErrorCode::SessionAlreadyBound,
                "guest session already has a table role",
            );
        }
        if let SessionRole::Player { seat } = role {
            if self
                .sessions
                .values()
                .any(|binding| binding.role == SessionRole::Player { seat })
            {
                return self.authorization_error(
                    AuthorizedTableErrorCode::SeatAlreadyOwned,
                    "player seat already has a guest session owner",
                );
            }
            self.table
                .snapshot(ProjectionAudience::Player(seat))
                .map_err(|_| {
                    AuthorizedTableError::new(
                        AuthorizedTableErrorCode::UnauthorizedSeat,
                        "player seat is not available in this hand",
                    )
                })?;
        }
        self.sessions.insert(
            session,
            SessionBinding {
                role,
                connected: true,
            },
        );
        Ok(())
    }

    fn submit(
        &mut self,
        session: GuestSessionId,
        command: CommandEnvelope,
    ) -> Result<AuthorizedTableResponse, AuthorizedTableError> {
        let binding = self.connected_binding(&session)?;
        let seat = match binding.role {
            SessionRole::Player { seat } => seat,
            SessionRole::Spectator => {
                return self.authorization_error(
                    AuthorizedTableErrorCode::SpectatorCannotAct,
                    "spectator sessions cannot submit gameplay commands",
                )
            }
        };
        if command.table_id != self.table_id {
            return self.authorization_error(
                AuthorizedTableErrorCode::WrongTable,
                "command targets a different table",
            );
        }
        if command.hand_id != self.hand_id {
            return self.authorization_error(
                AuthorizedTableErrorCode::WrongHand,
                "command targets a different hand",
            );
        }
        let command_seat = command.payload.seat();
        if command_seat != seat {
            return self.authorization_error(
                AuthorizedTableErrorCode::UnauthorizedSeat,
                "command seat is not owned by this guest session",
            );
        }

        let response = self
            .table
            .submit(command, binding.role.audience())
            .map_err(|_| AuthorizedTableError::authority_failure())?;
        let accepted_new = response.receipt.acknowledgement.delivery
            == AcknowledgementDelivery::Processed
            && response.receipt.acknowledgement.result == AcknowledgementResult::Accepted;
        if accepted_new {
            let event = accepted_event(&response.receipt);
            if event.as_ref().is_some_and(|event| {
                matches!(
                    event.event,
                    crate::protocol::TableEvent::ActionAccepted { .. }
                )
            }) {
                self.reschedule_deadline()?;
            }
            if let Some(accepted) = event.as_ref() {
                self.accepted_events.push(accepted.clone());
            }
            self.broadcast(SubscriptionReason::ActionAccepted, event)?;
        }
        Ok(AuthorizedTableResponse {
            receipt: response.receipt,
            snapshot: response.snapshot,
            deadline: self.deadline,
            stream_sequence: self.stream_sequence,
        })
    }

    fn snapshot(
        &mut self,
        session: GuestSessionId,
    ) -> Result<SnapshotEnvelope, AuthorizedTableError> {
        let binding = self.connected_binding(&session)?;
        self.table
            .snapshot(binding.role.audience())
            .map_err(|_| AuthorizedTableError::authority_failure())
    }

    fn bound_snapshot(
        &mut self,
        session: GuestSessionId,
    ) -> Result<SnapshotEnvelope, AuthorizedTableError> {
        let binding = self.sessions.get(&session).copied().ok_or_else(|| {
            self.authorization_rejections += 1;
            AuthorizedTableError::new(
                AuthorizedTableErrorCode::UnknownSession,
                "guest session is not bound to this table",
            )
        })?;
        self.table
            .snapshot(binding.role.audience())
            .map_err(|_| AuthorizedTableError::authority_failure())
    }

    fn public_snapshot(&mut self) -> Result<SnapshotEnvelope, AuthorizedTableError> {
        self.table
            .snapshot(ProjectionAudience::Spectator)
            .map_err(|_| AuthorizedTableError::authority_failure())
    }

    fn safe_history_material(
        &mut self,
    ) -> Result<(SnapshotEnvelope, Vec<EventEnvelope>), AuthorizedTableError> {
        Ok((self.public_snapshot()?, self.accepted_events.clone()))
    }

    fn subscribe(
        &mut self,
        session: GuestSessionId,
    ) -> Result<AuthorizedTableSubscription, AuthorizedTableError> {
        let binding = self.connected_binding(&session)?;
        if self.subscribers.contains_key(&session) {
            return self.authorization_error(
                AuthorizedTableErrorCode::SubscriptionAlreadyExists,
                "guest session already has an active table subscription",
            );
        }
        let snapshot = self
            .table
            .snapshot(binding.role.audience())
            .map_err(|_| AuthorizedTableError::authority_failure())?;
        let (sender, receiver) = mpsc::sync_channel(SUBSCRIPTION_BUFFER_CAPACITY);
        // A private bootstrap observes the current public broadcast cursor; it
        // must not create a sequence number that every other subscriber can
        // never receive. Only `broadcast` advances the table-wide stream.
        sender
            .try_send(SubscriptionUpdate {
                stream_sequence: self.stream_sequence,
                reason: SubscriptionReason::Initial,
                event: None,
                snapshot,
                deadline: self.deadline,
            })
            .map_err(|_| AuthorizedTableError::runtime_closed())?;
        self.subscribers.insert(session, sender);
        self.subscription_requests += 1;
        self.subscription_deliveries += 1;
        Ok(AuthorizedTableSubscription { receiver })
    }

    fn disconnect(&mut self, session: GuestSessionId) -> Result<(), AuthorizedTableError> {
        let binding = self.sessions.get_mut(&session).ok_or_else(|| {
            self.authorization_rejections += 1;
            AuthorizedTableError::new(
                AuthorizedTableErrorCode::UnknownSession,
                "guest session is not bound to this table",
            )
        })?;
        if !binding.connected {
            return Ok(());
        }
        binding.connected = false;
        let role = binding.role;
        self.subscribers.remove(&session);
        self.disconnects += 1;
        let seat = match role {
            SessionRole::Player { seat } => Some(seat),
            SessionRole::Spectator => None,
        };
        self.broadcast(
            SubscriptionReason::ConnectionStateChanged {
                seat,
                connected: false,
            },
            None,
        )?;
        Ok(())
    }

    fn reconnect(&mut self, session: GuestSessionId) -> Result<(), AuthorizedTableError> {
        let binding = self.sessions.get_mut(&session).ok_or_else(|| {
            self.authorization_rejections += 1;
            AuthorizedTableError::new(
                AuthorizedTableErrorCode::UnknownSession,
                "guest session is not bound to this table",
            )
        })?;
        if binding.connected {
            return Ok(());
        }
        binding.connected = true;
        let role = binding.role;
        let seat = match role {
            SessionRole::Player { seat } => Some(seat),
            SessionRole::Spectator => None,
        };
        self.broadcast(
            SubscriptionReason::ConnectionStateChanged {
                seat,
                connected: true,
            },
            None,
        )?;
        Ok(())
    }

    fn advance_time(&mut self, now_tick: u64) -> Result<TickResult, AuthorizedTableError> {
        if now_tick < self.now_tick {
            return self.authorization_error(
                AuthorizedTableErrorCode::ClockRegression,
                "authoritative logical time cannot move backwards",
            );
        }
        self.now_tick = now_tick;
        let Some(deadline) = self.deadline else {
            return Ok(self.tick_result(false, None, None));
        };

        if now_tick >= deadline.due_tick {
            return self.expire_deadline(deadline);
        }

        let mut warned = false;
        if now_tick >= deadline.warning_tick && !deadline.warning_emitted {
            let mut updated = deadline;
            updated.warning_emitted = true;
            self.deadline = Some(updated);
            self.deadline_warnings += 1;
            warned = true;
            self.broadcast(
                SubscriptionReason::DeadlineWarning {
                    seat: deadline.seat,
                    remaining_ticks: deadline.due_tick - now_tick,
                },
                None,
            )?;
        }
        Ok(self.tick_result(warned, None, None))
    }

    fn expire_deadline(
        &mut self,
        deadline: ActionDeadline,
    ) -> Result<TickResult, AuthorizedTableError> {
        let private = self
            .table
            .snapshot(ProjectionAudience::Player(deadline.seat))
            .map_err(|_| AuthorizedTableError::authority_failure())?;
        let legal = private.snapshot.legal_actions.ok_or_else(|| {
            AuthorizedTableError::new(
                AuthorizedTableErrorCode::DeadlineUnavailable,
                "deadline actor has no legal action",
            )
        })?;
        let action = if legal.can_check {
            Action::Check
        } else if legal.can_fold {
            Action::Fold
        } else {
            return Err(AuthorizedTableError::new(
                AuthorizedTableErrorCode::DeadlineUnavailable,
                "deadline actor can neither check nor fold",
            ));
        };
        let command = CommandEnvelope::act_for_hand(
            format!("srv-timeout-h{}-g{}", self.hand_id.0, deadline.generation),
            self.table_id,
            self.hand_id,
            private.revision,
            deadline.seat,
            action,
        );
        let response = self
            .table
            .submit_server(command, ProjectionAudience::Spectator)
            .map_err(|_| AuthorizedTableError::authority_failure())?;
        if response.receipt.acknowledgement.result != AcknowledgementResult::Accepted
            || response.receipt.acknowledgement.delivery != AcknowledgementDelivery::Processed
        {
            return Err(AuthorizedTableError::authority_failure());
        }
        let event = accepted_event(&response.receipt)
            .ok_or_else(AuthorizedTableError::authority_failure)?;
        self.accepted_events.push(event.clone());
        self.timeout_actions += 1;
        self.reschedule_deadline()?;
        self.broadcast(
            SubscriptionReason::TimeoutAction {
                seat: deadline.seat,
                action,
            },
            Some(event.clone()),
        )?;
        Ok(self.tick_result(false, Some(event), Some(action)))
    }

    fn reschedule_deadline(&mut self) -> Result<(), AuthorizedTableError> {
        let snapshot = self
            .table
            .snapshot(ProjectionAudience::Spectator)
            .map_err(|_| AuthorizedTableError::authority_failure())?;
        self.deadline = snapshot.snapshot.to_act.map(|seat| {
            self.deadline_generation += 1;
            let due_tick = self.now_tick + ACTION_TIMEOUT_TICKS;
            ActionDeadline {
                seat,
                warning_tick: due_tick - ACTION_WARNING_TICKS,
                due_tick,
                warning_emitted: false,
                generation: self.deadline_generation,
            }
        });
        Ok(())
    }

    fn broadcast(
        &mut self,
        reason: SubscriptionReason,
        event: Option<EventEnvelope>,
    ) -> Result<(), AuthorizedTableError> {
        self.stream_sequence += 1;
        let sequence = self.stream_sequence;
        let sessions = self.subscribers.keys().cloned().collect::<Vec<_>>();
        let mut remove = Vec::new();
        for session in sessions {
            let Some(binding) = self.sessions.get(&session).copied() else {
                remove.push(session);
                continue;
            };
            if !binding.connected {
                remove.push(session);
                continue;
            }
            let snapshot = self
                .table
                .snapshot(binding.role.audience())
                .map_err(|_| AuthorizedTableError::authority_failure())?;
            let update = SubscriptionUpdate {
                stream_sequence: sequence,
                reason: reason.clone(),
                event: event.clone(),
                snapshot,
                deadline: self.deadline,
            };
            let Some(sender) = self.subscribers.get(&session) else {
                continue;
            };
            match sender.try_send(update) {
                Ok(()) => self.subscription_deliveries += 1,
                Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                    self.slow_subscribers_dropped += 1;
                    remove.push(session);
                }
            }
        }
        for session in remove {
            self.subscribers.remove(&session);
        }
        Ok(())
    }

    fn connected_binding(
        &mut self,
        session: &GuestSessionId,
    ) -> Result<SessionBinding, AuthorizedTableError> {
        let Some(binding) = self.sessions.get(session).copied() else {
            return self.authorization_error(
                AuthorizedTableErrorCode::UnknownSession,
                "guest session is not bound to this table",
            );
        };
        if !binding.connected {
            return self.authorization_error(
                AuthorizedTableErrorCode::SessionDisconnected,
                "guest session is disconnected",
            );
        }
        Ok(binding)
    }

    fn authorization_error<T>(
        &mut self,
        code: AuthorizedTableErrorCode,
        message: impl Into<String>,
    ) -> Result<T, AuthorizedTableError> {
        self.authorization_rejections += 1;
        Err(AuthorizedTableError::new(code, message))
    }

    fn metrics(&self) -> AuthorizedTableMetrics {
        AuthorizedTableMetrics {
            actor: self.table.metrics().unwrap_or_default(),
            active_bindings: self.sessions.len() as u64,
            connected_bindings: self
                .sessions
                .values()
                .filter(|binding| binding.connected)
                .count() as u64,
            authorization_rejections: self.authorization_rejections,
            disconnects: self.disconnects,
            deadline_warnings: self.deadline_warnings,
            timeout_actions: self.timeout_actions,
            subscription_requests: self.subscription_requests,
            subscription_deliveries: self.subscription_deliveries,
            slow_subscribers_dropped: self.slow_subscribers_dropped,
            stream_sequence: self.stream_sequence,
            now_tick: self.now_tick,
        }
    }

    fn tick_result(
        &self,
        warning_emitted: bool,
        timeout_event: Option<EventEnvelope>,
        timeout_action: Option<Action>,
    ) -> TickResult {
        TickResult {
            now_tick: self.now_tick,
            warning_emitted,
            timeout_event,
            timeout_action,
            deadline: self.deadline,
            stream_sequence: self.stream_sequence,
        }
    }
}

fn accepted_event(receipt: &SubmissionReceipt) -> Option<EventEnvelope> {
    match &receipt.outcome {
        crate::protocol::CommandOutcome::Accepted { event } => Some(event.clone()),
        crate::protocol::CommandOutcome::Rejected { .. } => None,
    }
}

impl From<TableActorError> for AuthorizedTableError {
    fn from(_: TableActorError) -> Self {
        Self::authority_failure()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::multiway::MultiwayHand;
    use crate::game::seat::TableSize;
    use crate::protocol::{CommandOutcome, ProjectionKind, ProtocolErrorCode};

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    fn session(label: &str) -> GuestSessionId {
        GuestSessionId::new(label).unwrap()
    }

    fn runtime() -> AuthorizedTableRuntime {
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
        AuthorizedTableRuntime::spawn(ProtocolAuthority::new(TableId(44), HandId(1), hand)).unwrap()
    }

    fn bind_player(handle: &AuthorizedTableHandle, label: &str, player_seat: SeatId) {
        handle
            .bind(
                session(label),
                TableId(44),
                HandId(1),
                SessionRole::Player { seat: player_seat },
            )
            .unwrap();
    }

    fn command(id: &str, revision: u64, player_seat: SeatId, action: Action) -> CommandEnvelope {
        CommandEnvelope::act_for_hand(id, TableId(44), HandId(1), revision, player_seat, action)
    }

    #[test]
    fn session_binding_blocks_identity_confusion_before_authority_mutation() {
        let runtime = runtime();
        let handle = runtime.handle();
        bind_player(&handle, "guest-s0", seat(0));

        let duplicate_session = handle
            .bind(
                session("guest-s0"),
                TableId(44),
                HandId(1),
                SessionRole::Player { seat: seat(1) },
            )
            .unwrap_err();
        assert_eq!(
            duplicate_session.code,
            AuthorizedTableErrorCode::SessionAlreadyBound
        );
        let duplicate_seat = handle
            .bind(
                session("guest-other"),
                TableId(44),
                HandId(1),
                SessionRole::Player { seat: seat(0) },
            )
            .unwrap_err();
        assert_eq!(
            duplicate_seat.code,
            AuthorizedTableErrorCode::SeatAlreadyOwned
        );

        handle
            .bind(
                session("guest-watch"),
                TableId(44),
                HandId(1),
                SessionRole::Spectator,
            )
            .unwrap();
        let spectator = handle
            .submit(
                session("guest-watch"),
                command("watch-act", 0, seat(3), Action::AllIn(200)),
            )
            .unwrap_err();
        assert_eq!(spectator.code, AuthorizedTableErrorCode::SpectatorCannotAct);

        let cross_seat = handle
            .submit(
                session("guest-s0"),
                command("cross-seat", 0, seat(3), Action::AllIn(200)),
            )
            .unwrap_err();
        assert_eq!(cross_seat.code, AuthorizedTableErrorCode::UnauthorizedSeat);
        let wrong_table = handle
            .submit(
                session("guest-s0"),
                CommandEnvelope::act_for_hand(
                    "wrong-table",
                    TableId(45),
                    HandId(1),
                    0,
                    seat(0),
                    Action::AllIn(40),
                ),
            )
            .unwrap_err();
        assert_eq!(wrong_table.code, AuthorizedTableErrorCode::WrongTable);
        let wrong_hand = handle
            .submit(
                session("guest-s0"),
                CommandEnvelope::act_for_hand(
                    "wrong-hand",
                    TableId(44),
                    HandId(2),
                    0,
                    seat(0),
                    Action::AllIn(40),
                ),
            )
            .unwrap_err();
        assert_eq!(wrong_hand.code, AuthorizedTableErrorCode::WrongHand);
        let unknown = handle.snapshot(session("guest-unknown")).unwrap_err();
        assert_eq!(unknown.code, AuthorizedTableErrorCode::UnknownSession);

        let spectator_snapshot = handle.snapshot(session("guest-watch")).unwrap();
        assert_eq!(spectator_snapshot.revision, 0);
        assert_eq!(handle.metrics().unwrap().actor.processed_commands, 0);

        handle.disconnect(session("guest-s0")).unwrap();
        let disconnected = handle
            .submit(
                session("guest-s0"),
                command("after-disconnect", 0, seat(0), Action::AllIn(40)),
            )
            .unwrap_err();
        assert_eq!(
            disconnected.code,
            AuthorizedTableErrorCode::SessionDisconnected
        );
        assert_eq!(handle.metrics().unwrap().actor.processed_commands, 0);
        assert_eq!(
            format!("{:?}", session("private-guest")),
            "GuestSessionId(<redacted>)"
        );
        runtime.shutdown().unwrap();
    }

    #[test]
    fn authorized_player_submission_derives_its_private_audience() {
        let runtime = runtime();
        let handle = runtime.handle();
        bind_player(&handle, "guest-s3", seat(3));
        let response = handle
            .submit(
                session("guest-s3"),
                command("authorized-s3", 0, seat(3), Action::AllIn(200)),
            )
            .unwrap();
        assert_eq!(response.snapshot.revision, 1);
        assert_eq!(
            response.snapshot.snapshot.audience,
            ProjectionKind::Player { seat: seat(3) }
        );
        assert!(response.snapshot.snapshot.seats[3].hole_cards.is_some());
        assert!(response.snapshot.snapshot.seats[..3]
            .iter()
            .all(|projected| projected.hole_cards.is_none()));
        assert_eq!(response.deadline.unwrap().seat, seat(0));
        runtime.shutdown().unwrap();
    }

    #[test]
    fn warning_and_timeout_fold_are_deterministic_and_late_command_is_stale() {
        let runtime = runtime();
        let handle = runtime.handle();
        bind_player(&handle, "guest-s3", seat(3));
        bind_player(&handle, "guest-s0", seat(0));
        handle
            .submit(
                session("guest-s3"),
                command("open-all-in", 0, seat(3), Action::AllIn(200)),
            )
            .unwrap();

        let warning = handle.advance_time(50).unwrap();
        assert!(warning.warning_emitted);
        assert_eq!(warning.deadline.unwrap().seat, seat(0));
        assert_eq!(warning.deadline.unwrap().due_tick, 60);
        let timeout = handle.advance_time(60).unwrap();
        assert_eq!(timeout.timeout_action, Some(Action::Fold));
        assert_eq!(timeout.timeout_event.as_ref().unwrap().revision, 2);
        assert_eq!(timeout.deadline.unwrap().seat, seat(1));
        assert_eq!(timeout.deadline.unwrap().due_tick, 120);

        let late = handle
            .submit(
                session("guest-s0"),
                command("late-s0", 1, seat(0), Action::AllIn(40)),
            )
            .unwrap();
        assert!(
            matches!(late.receipt.outcome, CommandOutcome::Rejected { ref error }
            if error.error.code == ProtocolErrorCode::StaleRevision)
        );
        assert_eq!(late.snapshot.revision, 2);
        let metrics = handle.metrics().unwrap();
        assert_eq!(metrics.deadline_warnings, 1);
        assert_eq!(metrics.timeout_actions, 1);
        runtime.shutdown().unwrap();
    }

    #[test]
    fn timeout_checks_when_check_is_legal_and_clock_cannot_regress() {
        let runtime = runtime();
        let handle = runtime.handle();
        for index in 0..4 {
            bind_player(&handle, &format!("guest-s{index}"), seat(index));
        }
        let actions = [
            (3, Action::Call(2)),
            (0, Action::Call(2)),
            (1, Action::Call(1)),
            (2, Action::Check),
        ];
        for (revision, (index, action)) in actions.into_iter().enumerate() {
            handle
                .submit(
                    session(&format!("guest-s{index}")),
                    command(
                        &format!("preflop-{revision}"),
                        revision as u64,
                        seat(index),
                        action,
                    ),
                )
                .unwrap();
        }
        let before = handle.snapshot(session("guest-s1")).unwrap();
        assert_eq!(before.revision, 4);
        assert_eq!(before.snapshot.to_act, Some(seat(1)));
        assert!(before.snapshot.legal_actions.unwrap().can_check);

        let timeout = handle.advance_time(60).unwrap();
        assert_eq!(timeout.timeout_action, Some(Action::Check));
        assert_eq!(timeout.timeout_event.unwrap().revision, 5);
        let regression = handle.advance_time(59).unwrap_err();
        assert_eq!(regression.code, AuthorizedTableErrorCode::ClockRegression);
        assert_eq!(handle.metrics().unwrap().now_tick, 60);
        runtime.shutdown().unwrap();
    }

    #[test]
    fn player_and_spectator_streams_share_order_but_not_private_cards() {
        let runtime = runtime();
        let handle = runtime.handle();
        bind_player(&handle, "guest-s0", seat(0));
        bind_player(&handle, "guest-s3", seat(3));
        handle
            .bind(
                session("guest-watch"),
                TableId(44),
                HandId(1),
                SessionRole::Spectator,
            )
            .unwrap();
        let player = handle.subscribe(session("guest-s0")).unwrap();
        let spectator = handle.subscribe(session("guest-watch")).unwrap();
        let player_initial = player.recv().unwrap();
        let spectator_initial = spectator.recv().unwrap();
        assert_eq!(player_initial.snapshot.snapshot.visible_hand_count(), 1);
        assert_eq!(spectator_initial.snapshot.snapshot.visible_hand_count(), 0);

        handle
            .submit(
                session("guest-s3"),
                command("stream-action", 0, seat(3), Action::AllIn(200)),
            )
            .unwrap();
        let player_action = player.recv().unwrap();
        let spectator_action = spectator.recv().unwrap();
        assert_eq!(
            player_action.stream_sequence,
            spectator_action.stream_sequence
        );
        assert_eq!(player_action.snapshot.revision, 1);
        assert_eq!(spectator_action.snapshot.revision, 1);
        assert_eq!(player_action.snapshot.snapshot.visible_hand_count(), 1);
        assert_eq!(spectator_action.snapshot.snapshot.visible_hand_count(), 0);

        let serialized = serde_json::to_string(&player_action).unwrap();
        for forbidden in [
            "session",
            "credential",
            "reconnect",
            "deck",
            "seed",
            "random",
        ] {
            assert!(!serialized.to_ascii_lowercase().contains(forbidden));
        }
        let denied = handle
            .submit(
                session("guest-s0"),
                command("denied-no-broadcast", 1, seat(1), Action::AllIn(100)),
            )
            .unwrap_err();
        assert_eq!(denied.code, AuthorizedTableErrorCode::UnauthorizedSeat);
        assert!(matches!(player.try_recv(), Err(TryRecvError::Empty)));
        assert!(matches!(spectator.try_recv(), Err(TryRecvError::Empty)));
        runtime.shutdown().unwrap();
    }

    #[test]
    fn reconnect_reactivates_the_same_bound_audience_with_a_fresh_subscription() {
        let runtime = runtime();
        let handle = runtime.handle();
        let guest = session("reconnect-s0");
        bind_player(&handle, "reconnect-s0", seat(0));

        let first = handle.subscribe(guest.clone()).unwrap();
        let initial = first.recv().unwrap();
        assert_eq!(
            initial.snapshot.snapshot.audience,
            ProjectionKind::Player { seat: seat(0) }
        );

        handle.disconnect(guest.clone()).unwrap();
        assert_eq!(
            handle.snapshot(guest.clone()).unwrap_err().code,
            AuthorizedTableErrorCode::SessionDisconnected
        );

        handle.reconnect(guest.clone()).unwrap();
        let second = handle.subscribe(guest.clone()).unwrap();
        let restored = second.recv().unwrap();
        assert!(restored.stream_sequence > initial.stream_sequence);
        assert_eq!(
            restored.snapshot.snapshot.audience,
            initial.snapshot.snapshot.audience
        );
        assert_eq!(restored.snapshot.revision, initial.snapshot.revision);
        runtime.shutdown().unwrap();
    }

    #[test]
    fn a_full_subscription_buffer_is_dropped_without_blocking_the_table() {
        let runtime = runtime();
        let handle = runtime.handle();
        handle
            .bind(
                session("slow-watch"),
                TableId(44),
                HandId(1),
                SessionRole::Spectator,
            )
            .unwrap();
        let _slow = handle.subscribe(session("slow-watch")).unwrap();
        for index in 0..4 {
            bind_player(&handle, &format!("guest-s{index}"), seat(index));
        }
        let actions = [
            (3, Action::Call(2)),
            (0, Action::Call(2)),
            (1, Action::Call(1)),
            (2, Action::Check),
        ];
        for (revision, (index, action)) in actions.into_iter().enumerate() {
            handle
                .submit(
                    session(&format!("guest-s{index}")),
                    command(
                        &format!("buffer-{revision}"),
                        revision as u64,
                        seat(index),
                        action,
                    ),
                )
                .unwrap();
        }
        let metrics = handle.metrics().unwrap();
        assert_eq!(metrics.actor.accepted_commands, 4);
        assert_eq!(metrics.slow_subscribers_dropped, 1);
        assert_eq!(handle.snapshot(session("guest-s1")).unwrap().revision, 4);
        runtime.shutdown().unwrap();
    }

    trait VisibleHands {
        fn visible_hand_count(&self) -> usize;
    }

    impl VisibleHands for crate::protocol::TableProjection {
        fn visible_hand_count(&self) -> usize {
            self.seats
                .iter()
                .filter(|seat| seat.hole_cards.is_some())
                .count()
        }
    }
}
