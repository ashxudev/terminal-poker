//! Bounded multi-table registry and server-side routing boundary.
//!
//! Each registered running table owns exactly one [`AuthorizedTableRuntime`].
//! The registry assigns identities, constructs public lobby summaries, and
//! derives routes from private guest bindings before commands reach a table.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::authorized_table::{
    AuthorizedTableHandle, AuthorizedTableRuntime, GuestSessionId, SessionRole, SubscriptionReason,
    SubscriptionUpdate,
};
use crate::credentials::{
    AccessVerifier, BearerToken, CredentialRole, CredentialScope, CredentialVault,
    DurableCredentialRecord, ReconnectGrant, DEFAULT_CREDENTIAL_CAPACITY,
};
use crate::game::lifecycle::{
    BetweenHandLifecycle, RingEntryChoice, TableLifecycle, TableRunState,
};
use crate::game::multiway::MultiwayPhase;
use crate::game::seat::{PlayerId, SeatId};
use crate::game::state::{BIG_BLIND, SMALL_BLIND};
use crate::lobby::{
    PublicTableConfig, PublicTableFilter, PublicTableStatus, PublicTableSummary, RegistryHealth,
    TableHealthSummary, TableVisibility, MAX_PRIVATE_JOIN_CODE_BYTES, MAX_PUBLIC_TABLE_NAME_BYTES,
    MIN_PRIVATE_JOIN_CODE_BYTES,
};
use crate::protocol::{HandId, ProtocolAuthority, TableId};
use crate::ring_history::{
    HistoryStoreError, HistoryStoreReceipt, RingHistoryStore, SafeRingHandHistory,
};
use crate::tournament::{
    TournamentConfig, TournamentController, TournamentEntrant, TournamentStatus,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_TABLE_REGISTRY_CAPACITY: usize = 16;
pub const MAX_TABLE_REGISTRY_CAPACITY: usize = 64;
pub const MAX_TABLE_WAITING_ENTRIES: usize = 64;
pub const REGISTRY_CHECKPOINT_VERSION: u16 = 4;
pub const RECONNECT_CREDENTIAL_TTL: Duration = Duration::from_secs(5 * 60);
pub const MAX_REGISTRY_CHECKPOINT_BYTES: usize = 1_048_576;
const REGISTRY_CHECKPOINT_FORMAT: &str = "terminal-poker-registry";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRegistryErrorCode {
    InvalidCapacity,
    UnsupportedVersion,
    InvalidRequestId,
    CapacityReached,
    InvalidTableName,
    InvalidStartingStack,
    InvalidTournament,
    UnknownTable,
    TableNotJoinable,
    SeatUnavailable,
    SessionAlreadyRouted,
    WaitlistFull,
    SessionNotWaiting,
    SessionNotRouted,
    WrongTable,
    TableNotReady,
    HandNotComplete,
    UnsafeCheckpointBoundary,
    InvalidCheckpoint,
    CheckpointTooLarge,
    PersistenceFailure,
    TableNotRemovable,
    AuthorityFailure,
}

impl TableRegistryErrorCode {
    pub const fn name(self) -> &'static str {
        match self {
            Self::InvalidCapacity => "invalid_registry_capacity",
            Self::UnsupportedVersion => "unsupported_lobby_version",
            Self::InvalidRequestId => "invalid_lobby_request_id",
            Self::CapacityReached => "table_capacity_reached",
            Self::InvalidTableName => "invalid_table_name",
            Self::InvalidStartingStack => "invalid_starting_stack",
            Self::InvalidTournament => "invalid_tournament",
            Self::UnknownTable => "unknown_table",
            Self::TableNotJoinable => "table_not_joinable",
            Self::SeatUnavailable => "seat_unavailable",
            Self::SessionAlreadyRouted => "session_already_routed",
            Self::WaitlistFull => "table_waitlist_full",
            Self::SessionNotWaiting => "session_not_waiting",
            Self::SessionNotRouted => "session_not_routed",
            Self::WrongTable => "wrong_table",
            Self::TableNotReady => "table_not_ready",
            Self::HandNotComplete => "hand_not_complete",
            Self::UnsafeCheckpointBoundary => "unsafe_checkpoint_boundary",
            Self::InvalidCheckpoint => "invalid_checkpoint",
            Self::CheckpointTooLarge => "checkpoint_too_large",
            Self::PersistenceFailure => "checkpoint_io_failure",
            Self::TableNotRemovable => "table_not_removable",
            Self::AuthorityFailure => "table_authority_failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRegistryError {
    pub code: TableRegistryErrorCode,
    pub message: String,
}

impl TableRegistryError {
    fn new(code: TableRegistryErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn public(code: TableRegistryErrorCode, message: impl Into<String>) -> Self {
        Self::new(code, message)
    }
}

impl Display for TableRegistryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.name(), self.message)
    }
}

impl Error for TableRegistryError {}

#[derive(Debug, Clone)]
pub struct TableRoute {
    pub table_id: TableId,
    pub hand_id: HandId,
    pub seat: SeatId,
    pub handle: AuthorizedTableHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinOutcome {
    pub table: PublicTableSummary,
    pub seat: SeatId,
    pub hand_id: Option<HandId>,
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitOutcome {
    pub table: PublicTableSummary,
    pub position: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionOutcome {
    Joined(JoinOutcome),
    Waiting(WaitOutcome),
}

#[derive(Debug, Clone)]
struct WaitingEntry {
    session: GuestSessionId,
    requested_seat: Option<SeatId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistryCheckpointReceipt {
    pub version: u16,
    pub checksum: String,
    pub bytes: usize,
    pub tables: usize,
    pub sessions: usize,
    pub registry_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpirySweep {
    pub scanned: usize,
    pub expired: usize,
    pub retained: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCheckpointEnvelope {
    format: String,
    version: u16,
    checksum: String,
    payload: RegistryCheckpointPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryCheckpointPayload {
    max_tables: usize,
    next_table_id: u64,
    next_player_id: u64,
    next_hand_id: u64,
    registry_revision: u64,
    credential_capacity: usize,
    tables: Vec<CheckpointTable>,
    sessions: Vec<CheckpointSession>,
    credentials: Vec<DurableCredentialRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointTable {
    table_id: TableId,
    config: PublicTableConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_verifier: Option<AccessVerifier>,
    deterministic_seed: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tournament: Option<TournamentController>,
    lifecycle: BetweenHandLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointSession {
    principal_id: String,
    table_id: TableId,
    player_id: PlayerId,
    seat: SeatId,
}

#[derive(Debug, Clone)]
struct SessionRoute {
    table_id: TableId,
    player: PlayerId,
    seat: SeatId,
}

#[derive(Debug)]
struct RegisteredTable {
    config: PublicTableConfig,
    access_verifier: Option<AccessVerifier>,
    lifecycle: TableLifecycle,
    durable_lifecycle: TableLifecycle,
    deterministic_seed: Option<u64>,
    tournament: Option<TournamentController>,
    hand_id: Option<HandId>,
    seat_sessions: BTreeMap<SeatId, GuestSessionId>,
    waiting: VecDeque<WaitingEntry>,
    pending_departures: BTreeSet<GuestSessionId>,
    last_activity: Instant,
    runtime: Option<AuthorizedTableRuntime>,
    handle: Option<AuthorizedTableHandle>,
}

impl RegisteredTable {
    fn summary(&self, table_id: TableId) -> PublicTableSummary {
        let occupied =
            u8::try_from(self.lifecycle.seats().occupied_count()).expect("table occupancy fits u8");
        let reserved =
            u8::try_from(self.lifecycle.reservations().count()).expect("table reservations fit u8");
        let status = match self.lifecycle.state() {
            TableRunState::Waiting => PublicTableStatus::Waiting,
            TableRunState::Running => PublicTableStatus::Running,
            TableRunState::Paused => PublicTableStatus::Paused,
            TableRunState::Closed => PublicTableStatus::Closed,
        };
        PublicTableSummary {
            table_id,
            name: self.config.name.clone(),
            seats: self.config.seats,
            starting_stack: self.config.starting_stack,
            min_players: self.config.min_players,
            small_blind: self.tournament.as_ref().map_or(SMALL_BLIND, |tournament| {
                tournament.current_blinds().small_blind
            }),
            big_blind: self.tournament.as_ref().map_or(BIG_BLIND, |tournament| {
                tournament.current_blinds().big_blind
            }),
            occupied,
            reserved,
            waiting: u8::try_from(self.waiting.len()).expect("bounded waiting count fits u8"),
            status,
            joinable: self.runtime.is_none()
                && status != PublicTableStatus::Closed
                && self
                    .tournament
                    .as_ref()
                    .is_none_or(|tournament| tournament.status() == TournamentStatus::Registering)
                && occupied.saturating_add(reserved) < self.config.seats.get(),
            visibility: self.config.visibility,
            tournament: self
                .tournament
                .as_ref()
                .map(TournamentController::public_state),
        }
    }
}

#[derive(Debug)]
pub struct TableRegistry {
    max_tables: usize,
    next_table_id: u64,
    next_player_id: u64,
    next_hand_id: u64,
    revision: u64,
    tables: BTreeMap<TableId, RegisteredTable>,
    sessions: BTreeMap<GuestSessionId, SessionRoute>,
    waiting_sessions: BTreeMap<GuestSessionId, TableId>,
    retired_updates: BTreeMap<GuestSessionId, SubscriptionUpdate>,
    credentials: CredentialVault,
    history: RingHistoryStore,
    reconnect_ttl: Duration,
    last_checkpoint_millis: u64,
    max_checkpoint_millis: u64,
    last_checkpoint_bytes: usize,
}

impl TableRegistry {
    pub fn new(max_tables: usize) -> Result<Self, TableRegistryError> {
        if !(1..=MAX_TABLE_REGISTRY_CAPACITY).contains(&max_tables) {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::InvalidCapacity,
                format!("registry capacity must be between 1 and {MAX_TABLE_REGISTRY_CAPACITY}"),
            ));
        }
        Ok(Self {
            max_tables,
            next_table_id: 1,
            next_player_id: 1,
            next_hand_id: 1,
            revision: 0,
            tables: BTreeMap::new(),
            sessions: BTreeMap::new(),
            waiting_sessions: BTreeMap::new(),
            retired_updates: BTreeMap::new(),
            credentials: CredentialVault::new(DEFAULT_CREDENTIAL_CAPACITY).map_err(|_| {
                TableRegistryError::new(
                    TableRegistryErrorCode::InvalidCapacity,
                    "credential capacity is invalid",
                )
            })?,
            history: RingHistoryStore::default(),
            reconnect_ttl: RECONNECT_CREDENTIAL_TTL,
            last_checkpoint_millis: 0,
            max_checkpoint_millis: 0,
            last_checkpoint_bytes: 0,
        })
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn max_tables(&self) -> usize {
        self.max_tables
    }

    pub fn len(&self) -> usize {
        self.tables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    pub fn create(
        &mut self,
        mut config: PublicTableConfig,
        deterministic_seed: Option<u64>,
    ) -> Result<PublicTableSummary, TableRegistryError> {
        validate_config(&config)?;
        if self.tables.len() >= self.max_tables {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::CapacityReached,
                "public table registry is at its configured capacity",
            ));
        }
        let table_id = TableId(self.next_table_id);
        self.next_table_id = self.next_table_id.checked_add(1).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::CapacityReached,
                "table identity space is exhausted",
            )
        })?;
        let access_verifier = config.join_code.as_deref().map(|code| {
            if config.visibility == TableVisibility::PasswordProtected {
                AccessVerifier::derive_password(code)
            } else {
                AccessVerifier::derive(code)
            }
        });
        config.join_code = None;
        let lifecycle = TableLifecycle::new(config.seats);
        let table = RegisteredTable {
            durable_lifecycle: lifecycle.clone(),
            lifecycle,
            config,
            access_verifier,
            deterministic_seed,
            tournament: None,
            hand_id: None,
            seat_sessions: BTreeMap::new(),
            waiting: VecDeque::new(),
            pending_departures: BTreeSet::new(),
            last_activity: Instant::now(),
            runtime: None,
            handle: None,
        };
        let summary = table.summary(table_id);
        self.tables.insert(table_id, table);
        self.bump_revision();
        Ok(summary)
    }

    pub fn create_tournament(
        &mut self,
        config: TournamentConfig,
        deterministic_seed: Option<u64>,
    ) -> Result<PublicTableSummary, TableRegistryError> {
        let controller = TournamentController::new(config.clone()).map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::InvalidTournament, error.to_string())
        })?;
        let seats = crate::game::seat::TableSize::new(config.entrants).map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::InvalidTournament, error.to_string())
        })?;
        let table_config = PublicTableConfig {
            name: config.name.clone(),
            seats,
            starting_stack: config.starting_stack,
            min_players: config.entrants,
            visibility: if config.join_code.is_empty() {
                TableVisibility::Public
            } else {
                TableVisibility::PasswordProtected
            },
            join_code: if config.join_code.is_empty() {
                None
            } else {
                Some(config.join_code.clone())
            },
        };
        let summary = self.create(table_config, deterministic_seed)?;
        let table = self
            .tables
            .get_mut(&summary.table_id)
            .expect("new tournament table remains registered");
        table.tournament = Some(controller);
        Ok(table.summary(summary.table_id))
    }

    pub fn list(&self, filter: &PublicTableFilter) -> Vec<PublicTableSummary> {
        self.tables
            .iter()
            .filter(|(_, table)| {
                matches!(
                    table.config.visibility,
                    TableVisibility::Public | TableVisibility::PasswordProtected
                )
            })
            .map(|(&table_id, table)| table.summary(table_id))
            .filter(|summary| filter.matches(summary))
            .collect()
    }

    pub fn inspect(&self, table_id: TableId) -> Result<PublicTableSummary, TableRegistryError> {
        self.inspect_with_access(table_id, None)
    }

    pub fn health(&self) -> Result<RegistryHealth, TableRegistryError> {
        let mut table_health = Vec::with_capacity(self.tables.len());
        for (&table_id, table) in &self.tables {
            let summary = table.summary(table_id);
            let metrics = match &table.handle {
                Some(handle) => handle.metrics().map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::AuthorityFailure,
                        error.to_string(),
                    )
                })?,
                None => Default::default(),
            };
            table_health.push(TableHealthSummary {
                table_id,
                status: summary.status,
                hand_id: table.hand_id,
                occupied: summary.occupied,
                waiting: summary.waiting,
                stream_sequence: metrics.stream_sequence,
                accepted_commands: metrics.actor.accepted_commands,
                rejected_commands: metrics.actor.rejected_commands,
                authorization_rejections: metrics.authorization_rejections,
                disconnects: metrics.disconnects,
            });
        }
        Ok(RegistryHealth {
            healthy: true,
            lobby_revision: self.revision,
            table_capacity: self.max_tables,
            tables: self.tables.len(),
            routed_sessions: self.sessions.len(),
            waiting_sessions: self.waiting_sessions.len(),
            active_capabilities: self.credentials.active(),
            capability_capacity: DEFAULT_CREDENTIAL_CAPACITY,
            retained_safe_histories: self.history.histories().len(),
            last_checkpoint_millis: self.last_checkpoint_millis,
            max_checkpoint_millis: self.max_checkpoint_millis,
            last_checkpoint_bytes: self.last_checkpoint_bytes,
            checkpoint_version: REGISTRY_CHECKPOINT_VERSION,
            recovery_boundary: "latest_validated_between_hand_checkpoint".to_string(),
            table_health,
        })
    }

    pub fn inspect_with_access(
        &self,
        table_id: TableId,
        access_code: Option<&str>,
    ) -> Result<PublicTableSummary, TableRegistryError> {
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        if !table_allows(table, access_code) {
            return Err(unknown_table(table_id));
        }
        Ok(table.summary(table_id))
    }

    pub fn set_reconnect_ttl(&mut self, ttl: Duration) -> Result<(), TableRegistryError> {
        if ttl.is_zero() || ttl > Duration::from_secs(60 * 60) {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::InvalidCapacity,
                "reconnect credential TTL must be between 1 and 3600 seconds",
            ));
        }
        self.reconnect_ttl = ttl;
        Ok(())
    }

    pub fn issue_reconnect_credential(
        &mut self,
        principal: &GuestSessionId,
    ) -> Result<ReconnectGrant, TableRegistryError> {
        let route = self.sessions.get(principal).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::SessionNotRouted,
                "server-issued principal is not routed",
            )
        })?;
        let scope = CredentialScope {
            table_id: route.table_id,
            role: CredentialRole::Reconnect,
        };
        self.credentials.revoke_principal(principal);
        self.credentials
            .issue(principal.clone(), scope, self.reconnect_ttl)
            .map(|issued| issued.grant)
            .map_err(|_| {
                TableRegistryError::new(
                    TableRegistryErrorCode::CapacityReached,
                    "reconnect credential capacity reached",
                )
            })
    }

    pub fn authenticate_and_rotate_reconnect(
        &mut self,
        token: &BearerToken,
    ) -> Result<(GuestSessionId, ReconnectGrant), TableRegistryError> {
        let issued = self
            .credentials
            .authenticate_and_rotate(token, CredentialRole::Reconnect, self.reconnect_ttl)
            .map_err(|_| {
                TableRegistryError::new(
                    TableRegistryErrorCode::SessionNotRouted,
                    "reconnect credential is unknown, expired, revoked, or out of scope",
                )
            })?;
        let route = self.sessions.get(&issued.principal).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::SessionNotRouted,
                "reconnect credential has no live route",
            )
        })?;
        if route.table_id != issued.scope.table_id {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::WrongTable,
                "reconnect credential scope does not match its route",
            ));
        }
        Ok((issued.principal, issued.grant))
    }

    pub fn identify_reconnect(
        &mut self,
        token: &BearerToken,
    ) -> Result<GuestSessionId, TableRegistryError> {
        let (principal, scope) = self
            .credentials
            .authenticate(token, CredentialRole::Reconnect)
            .map_err(|_| {
                TableRegistryError::new(
                    TableRegistryErrorCode::SessionNotRouted,
                    "reconnect credential is unknown, expired, revoked, or out of scope",
                )
            })?;
        let route = self.sessions.get(&principal).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::SessionNotRouted,
                "reconnect credential has no live route",
            )
        })?;
        if route.table_id != scope.table_id {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::WrongTable,
                "reconnect credential scope does not match its route",
            ));
        }
        Ok(principal)
    }

    pub fn finalize_safe_history(
        &mut self,
        table_id: TableId,
        hand_id: HandId,
    ) -> Result<(), TableRegistryError> {
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        if table.hand_id != Some(hand_id) {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::WrongTable,
                "safe history finalization targets a different hand",
            ));
        }
        let handle = table.handle.clone().ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::TableNotReady,
                "safe history requires a running authority",
            )
        })?;
        let (terminal, events) = handle.safe_history_material().map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
        })?;
        let history =
            SafeRingHandHistory::from_public_terminal(&terminal, &events).map_err(|_| {
                TableRegistryError::new(
                    TableRegistryErrorCode::AuthorityFailure,
                    "terminal public history construction failed closed",
                )
            })?;
        self.history.record(history);
        Ok(())
    }

    /// Reconcile terminal authorities even when completion came from a timer
    /// and every client is disconnected. Registry serialization makes this
    /// exact-once across history, tournament standings, and successor hands.
    pub fn finalize_ready_hands(&mut self) -> Result<usize, TableRegistryError> {
        let mut ready = Vec::new();
        for (&table_id, table) in &self.tables {
            if let (Some(hand_id), Some(handle)) = (table.hand_id, table.handle.as_ref()) {
                let (snapshot, _) = handle.safe_history_material().map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::AuthorityFailure,
                        error.to_string(),
                    )
                })?;
                if matches!(
                    snapshot.snapshot.phase,
                    MultiwayPhase::Showdown | MultiwayPhase::HandComplete
                ) {
                    ready.push((table_id, hand_id));
                }
            }
        }
        for &(table_id, hand_id) in &ready {
            self.finalize_safe_history(table_id, hand_id)?;
            self.rollover_completed_hand(table_id, hand_id)?;
        }
        Ok(ready.len())
    }

    pub fn save_safe_histories(
        &self,
        path: &Path,
    ) -> Result<HistoryStoreReceipt, HistoryStoreError> {
        self.history.save_to_path(path)
    }

    pub fn load_safe_histories(&mut self, path: &Path) -> Result<(), HistoryStoreError> {
        self.history = RingHistoryStore::load_from_path(path)?;
        Ok(())
    }

    pub fn safe_history_count(&self) -> usize {
        self.history.histories().len()
    }

    /// Read-only public terminal evidence retained by the registry.
    pub fn safe_histories(&self) -> &[SafeRingHandHistory] {
        self.history.histories()
    }

    pub fn join(
        &mut self,
        session: GuestSessionId,
        table_id: TableId,
        requested_seat: Option<SeatId>,
    ) -> Result<JoinOutcome, TableRegistryError> {
        if self.sessions.contains_key(&session) {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::SessionAlreadyRouted,
                "guest session is already assigned to a table",
            ));
        }
        let player = PlayerId::new(self.next_player_id);
        let next_player_id = self.next_player_id.checked_add(1).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::AuthorityFailure,
                "player identity space is exhausted",
            )
        })?;

        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        if table.runtime.is_some() || table.lifecycle.state() == TableRunState::Closed {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::TableNotJoinable,
                "table is not accepting between-hand joins in this slice",
            ));
        }
        let mut lifecycle = table.lifecycle.clone();
        let mut seat_sessions = table.seat_sessions.clone();
        let config = table.config.clone();
        let deterministic_seed = table.deterministic_seed;
        let mut tournament = table.tournament.clone();
        let seat = match requested_seat {
            Some(seat)
                if config.seats.contains(seat)
                    && lifecycle.seats().seat(seat).is_none()
                    && lifecycle
                        .reservations()
                        .all(|(reserved, _)| reserved != seat) =>
            {
                seat
            }
            Some(_) => {
                return Err(TableRegistryError::new(
                    TableRegistryErrorCode::SeatUnavailable,
                    "requested seat is outside the table or unavailable",
                ))
            }
            None => config
                .seats
                .seats()
                .find(|&candidate| {
                    lifecycle.seats().seat(candidate).is_none()
                        && lifecycle
                            .reservations()
                            .all(|(reserved, _)| reserved != candidate)
                })
                .ok_or_else(|| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::SeatUnavailable,
                        "table has no available seat",
                    )
                })?,
        };
        lifecycle
            .join(player, seat, config.starting_stack)
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
        if let Some(controller) = tournament.as_mut() {
            controller
                .register(TournamentEntrant { player, seat })
                .map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::InvalidTournament,
                        error.to_string(),
                    )
                })?;
            if controller.public_state().registered == config.min_players {
                controller.start().map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::InvalidTournament,
                        error.to_string(),
                    )
                })?;
            }
        }
        seat_sessions.insert(seat, session.clone());
        let durable_lifecycle = lifecycle.clone();

        let mut next_hand_id = self.next_hand_id;
        let ready_to_start = tournament.as_ref().map_or_else(
            || lifecycle.eligible_count() >= usize::from(config.min_players),
            |controller| controller.status() == TournamentStatus::Running,
        );
        let prepared_runtime = if ready_to_start {
            let start = lifecycle.begin_hand().map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
            let hand_id = HandId(self.next_hand_id);
            next_hand_id = self.next_hand_id.checked_add(1).ok_or_else(|| {
                TableRegistryError::new(
                    TableRegistryErrorCode::AuthorityFailure,
                    "hand identity space is exhausted",
                )
            })?;
            let hand = match tournament.as_ref() {
                Some(controller) => start.into_hand_with_blinds(
                    config.seats,
                    deterministic_seed,
                    controller.current_blinds(),
                ),
                None => start.into_hand(config.seats, deterministic_seed),
            }
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
            let runtime = AuthorizedTableRuntime::spawn(ProtocolAuthority::new_paced(
                table_id,
                table_id_to_hand(table_id, hand_id),
                hand,
            ))
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
            let handle = runtime.handle();
            for (&bound_seat, bound_session) in &seat_sessions {
                handle
                    .bind(
                        bound_session.clone(),
                        table_id,
                        table_id_to_hand(table_id, hand_id),
                        SessionRole::Player { seat: bound_seat },
                    )
                    .map_err(|error| {
                        TableRegistryError::new(
                            TableRegistryErrorCode::AuthorityFailure,
                            error.to_string(),
                        )
                    })?;
            }
            Some((table_id_to_hand(table_id, hand_id), runtime, handle))
        } else {
            None
        };

        let table = self
            .tables
            .get_mut(&table_id)
            .expect("validated table remains registered");
        table.lifecycle = lifecycle;
        table.durable_lifecycle = durable_lifecycle;
        table.seat_sessions = seat_sessions;
        table.tournament = tournament;
        table.last_activity = Instant::now();
        if let Some((hand_id, runtime, handle)) = prepared_runtime {
            table.hand_id = Some(hand_id);
            table.handle = Some(handle);
            table.runtime = Some(runtime);
        }
        self.next_player_id = next_player_id;
        self.next_hand_id = next_hand_id;
        self.retired_updates.remove(&session);
        self.sessions.insert(
            session,
            SessionRoute {
                table_id,
                player,
                seat,
            },
        );
        self.bump_revision();
        let table = self.tables.get(&table_id).expect("joined table remains");
        Ok(JoinOutcome {
            table: table.summary(table_id),
            seat,
            hand_id: table.hand_id,
            ready: table.handle.is_some(),
        })
    }

    /// Admits immediately at a safe boundary, otherwise appends exactly once
    /// to the table's bounded FIFO. Player identities and stacks are allocated
    /// only when admission succeeds.
    pub fn join_or_wait(
        &mut self,
        session: GuestSessionId,
        table_id: TableId,
        requested_seat: Option<SeatId>,
    ) -> Result<AdmissionOutcome, TableRegistryError> {
        self.join_or_wait_with_access(session, table_id, requested_seat, None)
    }

    pub fn join_or_wait_with_access(
        &mut self,
        session: GuestSessionId,
        table_id: TableId,
        requested_seat: Option<SeatId>,
        access_code: Option<&str>,
    ) -> Result<AdmissionOutcome, TableRegistryError> {
        if self.sessions.contains_key(&session) || self.waiting_sessions.contains_key(&session) {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::SessionAlreadyRouted,
                "guest session already has a seat or waiting entry",
            ));
        }
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        if !table_allows(table, access_code) {
            return Err(unknown_table(table_id));
        }
        if table
            .tournament
            .as_ref()
            .is_some_and(|tournament| tournament.status() != TournamentStatus::Registering)
        {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::TableNotJoinable,
                "tournament registration is locked",
            ));
        }
        if table.lifecycle.state() == TableRunState::Closed {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::TableNotJoinable,
                "table is closed",
            ));
        }
        let has_requested_seat = requested_seat.is_none_or(|seat| {
            table.config.seats.contains(seat)
                && table.lifecycle.seats().seat(seat).is_none()
                && table
                    .lifecycle
                    .reservations()
                    .all(|(reserved, _)| reserved != seat)
        });
        let has_any_seat = table.lifecycle.seats().occupied_count()
            + table.lifecycle.reservations().count()
            < usize::from(table.config.seats.get());
        if table.runtime.is_none() && has_requested_seat && has_any_seat {
            return self
                .join(session, table_id, requested_seat)
                .map(AdmissionOutcome::Joined);
        }
        if table.waiting.len() >= MAX_TABLE_WAITING_ENTRIES {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::WaitlistFull,
                "table waiting list is at its bounded capacity",
            ));
        }
        let table = self
            .tables
            .get_mut(&table_id)
            .expect("validated table remains");
        table.waiting.push_back(WaitingEntry {
            session: session.clone(),
            requested_seat,
        });
        table.last_activity = Instant::now();
        self.waiting_sessions.insert(session, table_id);
        self.bump_revision();
        let table = self.tables.get(&table_id).expect("queued table remains");
        Ok(AdmissionOutcome::Waiting(WaitOutcome {
            table: table.summary(table_id),
            position: u8::try_from(table.waiting.len()).expect("bounded waiting position fits u8"),
        }))
    }

    pub fn admission_status(
        &self,
        session: &GuestSessionId,
    ) -> Result<AdmissionOutcome, TableRegistryError> {
        if self.sessions.contains_key(session) {
            return self.join_status(session).map(AdmissionOutcome::Joined);
        }
        let table_id = *self.waiting_sessions.get(session).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::SessionNotRouted,
                "guest session has not joined or queued",
            )
        })?;
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        let position = table
            .waiting
            .iter()
            .position(|entry| &entry.session == session)
            .expect("waiting index and table queue remain consistent")
            + 1;
        Ok(AdmissionOutcome::Waiting(WaitOutcome {
            table: table.summary(table_id),
            position: u8::try_from(position).expect("bounded waiting position fits u8"),
        }))
    }

    /// Socket departure releases only pre-start registration/waitlist entries.
    /// Active games retain their ordinary disconnect/reconnect policy.
    pub fn cancel_pending_registration(&mut self, session: &GuestSessionId) {
        let table_id = self
            .waiting_sessions
            .get(session)
            .copied()
            .or_else(|| self.sessions.get(session).map(|route| route.table_id));
        if let Some(table_id) = table_id {
            let _ = self.cancel_wait(session, table_id);
        }
    }

    pub fn cancel_wait(
        &mut self,
        session: &GuestSessionId,
        table_id: TableId,
    ) -> Result<(), TableRegistryError> {
        if let Some(route) = self.sessions.get(session).cloned() {
            let table = self
                .tables
                .get_mut(&table_id)
                .ok_or_else(|| unknown_table(table_id))?;
            if route.table_id != table_id
                || table.runtime.is_some()
                || table
                    .tournament
                    .as_ref()
                    .is_none_or(|t| t.status() != TournamentStatus::Registering)
            {
                return Err(TableRegistryError::new(
                    TableRegistryErrorCode::SessionNotWaiting,
                    "registration is already locked",
                ));
            }
            let mut lifecycle = table.lifecycle.clone();
            lifecycle.request_leave(route.player).map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
            table
                .tournament
                .as_mut()
                .expect("registering tournament")
                .unregister(route.player)
                .map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::InvalidTournament,
                        error.to_string(),
                    )
                })?;
            table.lifecycle = lifecycle;
            table.durable_lifecycle = table.lifecycle.clone();
            table.seat_sessions.remove(&route.seat);
            table.last_activity = Instant::now();
            self.sessions.remove(session);
            self.bump_revision();
            return Ok(());
        }
        if self.waiting_sessions.get(session) != Some(&table_id) {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::SessionNotWaiting,
                "guest session has no matching waiting entry",
            ));
        }
        let table = self
            .tables
            .get_mut(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        table.waiting.retain(|entry| &entry.session != session);
        table.last_activity = Instant::now();
        self.waiting_sessions.remove(session);
        self.bump_revision();
        Ok(())
    }

    pub fn request_leave(&mut self, session: &GuestSessionId) -> Result<(), TableRegistryError> {
        let route = self.sessions.get(session).cloned().ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::SessionNotRouted,
                "guest session has not joined a table",
            )
        })?;
        let table = self
            .tables
            .get_mut(&route.table_id)
            .ok_or_else(|| unknown_table(route.table_id))?;
        table
            .lifecycle
            .request_leave(route.player)
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
        table.pending_departures.insert(session.clone());
        table.last_activity = Instant::now();
        self.bump_revision();
        Ok(())
    }

    pub fn join_status(&self, session: &GuestSessionId) -> Result<JoinOutcome, TableRegistryError> {
        let route = self.sessions.get(session).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::SessionNotRouted,
                "guest session has not joined a table",
            )
        })?;
        let table = self
            .tables
            .get(&route.table_id)
            .ok_or_else(|| unknown_table(route.table_id))?;
        Ok(JoinOutcome {
            table: table.summary(route.table_id),
            seat: route.seat,
            hand_id: table.hand_id,
            ready: table.handle.is_some(),
        })
    }

    pub fn route(
        &self,
        session: &GuestSessionId,
        requested_table: TableId,
    ) -> Result<TableRoute, TableRegistryError> {
        let binding = self.sessions.get(session).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::SessionNotRouted,
                "guest session has not joined a table",
            )
        })?;
        if binding.table_id != requested_table {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::WrongTable,
                "guest session is bound to a different table",
            ));
        }
        let table = self
            .tables
            .get(&binding.table_id)
            .ok_or_else(|| unknown_table(binding.table_id))?;
        let handle = table.handle.clone().ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::TableNotReady,
                "table is waiting for enough eligible players",
            )
        })?;
        Ok(TableRoute {
            table_id: binding.table_id,
            hand_id: table.hand_id.expect("ready table has hand identity"),
            seat: binding.seat,
            handle,
        })
    }

    pub fn route_for_session(
        &self,
        session: &GuestSessionId,
    ) -> Result<TableRoute, TableRegistryError> {
        let table_id = self
            .sessions
            .get(session)
            .map(|route| route.table_id)
            .ok_or_else(|| {
                TableRegistryError::new(
                    TableRegistryErrorCode::SessionNotRouted,
                    "guest session has not joined a table",
                )
            })?;
        self.route(session, table_id)
    }

    pub fn player_for_session(&self, session: &GuestSessionId) -> Option<PlayerId> {
        self.sessions.get(session).map(|route| route.player)
    }

    pub fn tournament_break_pending(&self, session: &GuestSessionId) -> bool {
        self.sessions
            .get(session)
            .and_then(|route| self.tables.get(&route.table_id))
            .and_then(|table| table.tournament.as_ref())
            .is_some_and(|tournament| tournament.status() == TournamentStatus::Break)
    }

    pub fn take_retired_update(&mut self, session: &GuestSessionId) -> Option<SubscriptionUpdate> {
        self.retired_updates.remove(session)
    }

    /// Reconciles one terminal hand and installs exactly one fresh authority.
    ///
    /// The expected hand identity makes concurrent terminal observers
    /// idempotent: once another observer has advanced the table, this call is a
    /// no-op. Only public terminal stacks cross the authority boundary; command
    /// ledgers, hole-card projections, and pot awards stay inside the retired
    /// per-hand runtime.
    pub fn rollover_completed_hand(
        &mut self,
        table_id: TableId,
        expected_hand_id: HandId,
    ) -> Result<Option<HandId>, TableRegistryError> {
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        if table.hand_id != Some(expected_hand_id) {
            return Ok(None);
        }
        let (&sample_seat, sample_session) =
            table.seat_sessions.iter().next().ok_or_else(|| {
                TableRegistryError::new(
                    TableRegistryErrorCode::AuthorityFailure,
                    "running table has no bound player session",
                )
            })?;
        let handle = table.handle.as_ref().ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::TableNotReady,
                "table has no runtime",
            )
        })?;
        let terminal = handle
            .bound_snapshot(sample_session.clone())
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
        if terminal.hand_id != expected_hand_id
            || !matches!(
                terminal.snapshot.phase,
                MultiwayPhase::Showdown | MultiwayPhase::HandComplete
            )
        {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::HandNotComplete,
                "only a terminal authoritative hand can roll over",
            ));
        }
        debug_assert!(terminal
            .snapshot
            .seats
            .iter()
            .any(|projected| projected.seat == sample_seat));
        let terminal_stream_sequence = handle
            .metrics()
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?
            .stream_sequence;
        let terminal_updates = table
            .seat_sessions
            .values()
            .map(|session| {
                handle.bound_snapshot(session.clone()).map(|snapshot| {
                    (
                        session.clone(),
                        SubscriptionUpdate {
                            stream_sequence: terminal_stream_sequence,
                            reason: SubscriptionReason::Initial,
                            event: None,
                            snapshot,
                            deadline: None,
                        },
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
        for (session, update) in terminal_updates {
            self.retired_updates.insert(session, update);
        }
        let final_stacks = terminal
            .snapshot
            .seats
            .iter()
            .map(|seat| (seat.seat, seat.stack))
            .collect::<Vec<_>>();

        let table = self
            .tables
            .get(&table_id)
            .expect("validated table remains registered");
        let mut lifecycle = table.lifecycle.clone();
        let mut seat_sessions = table.seat_sessions.clone();
        let pending_departures = table.pending_departures.clone();
        let mut waiting = table.waiting.clone();
        let config = table.config.clone();
        let deterministic_seed = table.deterministic_seed;
        let mut tournament = table.tournament.clone();
        let hand_elapsed = table.last_activity.elapsed();
        let starting_stacks = lifecycle_stacks(&table.lifecycle);
        lifecycle.complete_hand(&final_stacks).map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
        })?;
        if let Some(controller) = tournament.as_mut() {
            controller
                .tick_between_hands(u64::try_from(hand_elapsed.as_millis()).unwrap_or(u64::MAX));
            controller
                .complete_hand(&starting_stacks, &final_stacks)
                .map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::InvalidTournament,
                        error.to_string(),
                    )
                })?;
        }
        for departing in &pending_departures {
            if let Some(route) = self.sessions.remove(departing) {
                seat_sessions.remove(&route.seat);
            }
        }
        seat_sessions.retain(|seat, _| lifecycle.seats().seat(*seat).is_some());

        while tournament.is_none() {
            let Some(entry) = waiting.front().cloned() else {
                break;
            };
            let seat = match entry.requested_seat {
                Some(seat)
                    if config.seats.contains(seat)
                        && lifecycle.seats().seat(seat).is_none()
                        && lifecycle
                            .reservations()
                            .all(|(reserved, _)| reserved != seat) =>
                {
                    seat
                }
                Some(_) => break,
                None => match config.seats.seats().find(|&candidate| {
                    lifecycle.seats().seat(candidate).is_none()
                        && lifecycle
                            .reservations()
                            .all(|(reserved, _)| reserved != candidate)
                }) {
                    Some(seat) => seat,
                    None => break,
                },
            };
            let player = PlayerId::new(self.next_player_id);
            self.next_player_id = self.next_player_id.checked_add(1).ok_or_else(|| {
                TableRegistryError::new(
                    TableRegistryErrorCode::AuthorityFailure,
                    "player identity space is exhausted",
                )
            })?;
            lifecycle
                .join_with_entry(
                    player,
                    seat,
                    config.starting_stack,
                    RingEntryChoice::PostLiveBigBlind,
                )
                .map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::AuthorityFailure,
                        error.to_string(),
                    )
                })?;
            waiting.pop_front();
            self.waiting_sessions.remove(&entry.session);
            seat_sessions.insert(seat, entry.session.clone());
            self.sessions.insert(
                entry.session,
                SessionRoute {
                    table_id,
                    player,
                    seat,
                },
            );
        }
        let durable_lifecycle = lifecycle.clone();
        let tournament_paused_or_complete = tournament.as_ref().is_some_and(|controller| {
            matches!(
                controller.status(),
                TournamentStatus::Break | TournamentStatus::Complete | TournamentStatus::Cancelled
            )
        });
        let minimum_remaining = if tournament.is_some() {
            2
        } else {
            usize::from(config.min_players)
        };
        if lifecycle.eligible_count() < minimum_remaining || tournament_paused_or_complete {
            let table = self
                .tables
                .get_mut(&table_id)
                .expect("validated table remains registered");
            table.lifecycle = lifecycle;
            table.durable_lifecycle = durable_lifecycle;
            table.hand_id = None;
            table.handle = None;
            table.runtime = None;
            table.seat_sessions = seat_sessions;
            table.waiting = waiting;
            table.pending_departures.clear();
            table.tournament = tournament;
            table.last_activity = Instant::now();
            self.bump_revision();
            return Ok(None);
        }

        let start = lifecycle.begin_hand().map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
        })?;
        let next_hand_id = HandId(self.next_hand_id);
        let following_hand_id = self.next_hand_id.checked_add(1).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::AuthorityFailure,
                "hand identity space is exhausted",
            )
        })?;
        let hand_seed = deterministic_seed.map(|seed| seed.wrapping_add(start.number - 1));
        let hand = match tournament.as_ref() {
            Some(controller) => {
                start.into_hand_with_blinds(config.seats, hand_seed, controller.current_blinds())
            }
            None => start.into_hand(config.seats, hand_seed),
        }
        .map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
        })?;
        let runtime = AuthorizedTableRuntime::spawn(ProtocolAuthority::new_paced(
            table_id,
            next_hand_id,
            hand,
        ))
        .map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
        })?;
        let handle = runtime.handle();
        for (&seat, session) in &seat_sessions {
            handle
                .bind(
                    session.clone(),
                    table_id,
                    next_hand_id,
                    SessionRole::Player { seat },
                )
                .map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::AuthorityFailure,
                        error.to_string(),
                    )
                })?;
        }

        let table = self
            .tables
            .get_mut(&table_id)
            .expect("validated table remains registered");
        table.lifecycle = lifecycle;
        table.durable_lifecycle = durable_lifecycle;
        table.hand_id = Some(next_hand_id);
        table.handle = Some(handle);
        table.runtime = Some(runtime);
        table.seat_sessions = seat_sessions;
        table.waiting = waiting;
        table.pending_departures.clear();
        table.tournament = tournament;
        table.last_activity = Instant::now();
        self.next_hand_id = following_hand_id;
        self.bump_revision();
        Ok(Some(next_hand_id))
    }

    /// Advances scheduled breaks on the server clock and installs successor
    /// authorities only after the break has ended.
    pub fn advance_tournament_breaks(&mut self) -> Result<usize, TableRegistryError> {
        let now = Instant::now();
        let table_ids =
            self.tables
                .iter()
                .filter_map(|(&table_id, table)| {
                    (table.runtime.is_none()
                        && table.tournament.as_ref().is_some_and(|tournament| {
                            tournament.status() == TournamentStatus::Break
                        }))
                    .then_some(table_id)
                })
                .collect::<Vec<_>>();
        let mut resumed = 0usize;
        for table_id in table_ids {
            let ready = {
                let table = self
                    .tables
                    .get_mut(&table_id)
                    .expect("collected tournament remains registered");
                let elapsed = now.saturating_duration_since(table.last_activity);
                let controller = table
                    .tournament
                    .as_mut()
                    .expect("break table has a tournament controller");
                controller
                    .tick_between_hands(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
                table.last_activity = now;
                controller.status() == TournamentStatus::Running
                    && table.lifecycle.eligible_count() >= 2
            };
            if ready {
                self.start_waiting_tournament_hand(table_id)?;
                resumed = resumed.saturating_add(1);
            }
        }
        if resumed > 0 {
            self.bump_revision();
        }
        Ok(resumed)
    }

    fn start_waiting_tournament_hand(
        &mut self,
        table_id: TableId,
    ) -> Result<HandId, TableRegistryError> {
        let table = self
            .tables
            .get(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        let mut lifecycle = table.lifecycle.clone();
        let config = table.config.clone();
        let deterministic_seed = table.deterministic_seed;
        let blind_values = table
            .tournament
            .as_ref()
            .ok_or_else(|| {
                TableRegistryError::new(
                    TableRegistryErrorCode::InvalidTournament,
                    "successor requires a tournament controller",
                )
            })?
            .current_blinds();
        let seat_sessions = table.seat_sessions.clone();
        let start = lifecycle.begin_hand().map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
        })?;
        let hand_id = HandId(self.next_hand_id);
        let following = self.next_hand_id.checked_add(1).ok_or_else(|| {
            TableRegistryError::new(
                TableRegistryErrorCode::AuthorityFailure,
                "hand identity space is exhausted",
            )
        })?;
        let hand_seed = deterministic_seed.map(|seed| seed.wrapping_add(start.number - 1));
        let hand = start
            .into_hand_with_blinds(config.seats, hand_seed, blind_values)
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
        let runtime =
            AuthorizedTableRuntime::spawn(ProtocolAuthority::new_paced(table_id, hand_id, hand))
                .map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::AuthorityFailure,
                        error.to_string(),
                    )
                })?;
        let handle = runtime.handle();
        for (&seat, session) in &seat_sessions {
            handle
                .bind(
                    session.clone(),
                    table_id,
                    hand_id,
                    SessionRole::Player { seat },
                )
                .map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::AuthorityFailure,
                        error.to_string(),
                    )
                })?;
        }
        let table = self
            .tables
            .get_mut(&table_id)
            .expect("validated tournament remains registered");
        table.lifecycle = lifecycle;
        table.hand_id = Some(hand_id);
        table.handle = Some(handle);
        table.runtime = Some(runtime);
        table.last_activity = Instant::now();
        self.next_hand_id = following;
        Ok(hand_id)
    }

    /// Publishes the latest per-table between-hand images as one bounded,
    /// checksummed registry checkpoint.
    pub fn save_checkpoint(
        &mut self,
        path: &Path,
    ) -> Result<RegistryCheckpointReceipt, TableRegistryError> {
        let started = Instant::now();
        if path.file_name().is_none() {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::PersistenceFailure,
                "checkpoint path must name a file",
            ));
        }
        let tables = self
            .tables
            .iter()
            .map(|(&table_id, table)| {
                table
                    .durable_lifecycle
                    .between_hand_checkpoint()
                    .map(|lifecycle| CheckpointTable {
                        table_id,
                        config: table.config.clone(),
                        access_verifier: table.access_verifier.clone(),
                        deterministic_seed: table.deterministic_seed,
                        tournament: table.tournament.clone(),
                        lifecycle,
                    })
                    .map_err(|error| {
                        TableRegistryError::new(
                            TableRegistryErrorCode::UnsafeCheckpointBoundary,
                            error.to_string(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sessions = self
            .sessions
            .iter()
            .map(|(session, route)| CheckpointSession {
                principal_id: session.stable_value().to_string(),
                table_id: route.table_id,
                player_id: route.player,
                seat: route.seat,
            })
            .collect::<Vec<_>>();
        let payload = RegistryCheckpointPayload {
            max_tables: self.max_tables,
            next_table_id: self.next_table_id,
            next_player_id: self.next_player_id,
            next_hand_id: self.next_hand_id,
            registry_revision: self.revision,
            credential_capacity: DEFAULT_CREDENTIAL_CAPACITY,
            tables,
            sessions,
            credentials: self.credentials.durable_records(),
        };
        let payload_bytes = serde_json::to_vec(&payload).map_err(checkpoint_serialization_error)?;
        let checksum = format!("fnv1a64:{:016x}", fnv1a64(&payload_bytes));
        let envelope = RegistryCheckpointEnvelope {
            format: REGISTRY_CHECKPOINT_FORMAT.to_string(),
            version: REGISTRY_CHECKPOINT_VERSION,
            checksum: checksum.clone(),
            payload,
        };
        let bytes = serde_json::to_vec_pretty(&envelope).map_err(checkpoint_serialization_error)?;
        if bytes.len() > MAX_REGISTRY_CHECKPOINT_BYTES {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::CheckpointTooLarge,
                format!(
                    "checkpoint is {} bytes; limit is {MAX_REGISTRY_CHECKPOINT_BYTES}",
                    bytes.len()
                ),
            ));
        }
        let temporary = checkpoint_temporary_path(path);
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.flush()?;
            file.sync_all()?;
            drop(file);
            atomic_replace(&temporary, path)
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::PersistenceFailure,
                format!("checkpoint publication failed: {error}"),
            ));
        }
        let elapsed = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_checkpoint_millis = elapsed;
        self.max_checkpoint_millis = self.max_checkpoint_millis.max(elapsed);
        self.last_checkpoint_bytes = bytes.len();
        Ok(RegistryCheckpointReceipt {
            version: REGISTRY_CHECKPOINT_VERSION,
            checksum,
            bytes: bytes.len(),
            tables: envelope.payload.tables.len(),
            sessions: envelope.payload.sessions.len(),
            registry_revision: self.revision,
        })
    }

    pub fn load_checkpoint(path: &Path) -> Result<Self, TableRegistryError> {
        let metadata = fs::metadata(path).map_err(checkpoint_io_error)?;
        let file_size = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
        if file_size > MAX_REGISTRY_CHECKPOINT_BYTES {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::CheckpointTooLarge,
                format!(
                    "checkpoint is {file_size} bytes; limit is {MAX_REGISTRY_CHECKPOINT_BYTES}"
                ),
            ));
        }
        let bytes = fs::read(path).map_err(checkpoint_io_error)?;
        let envelope: RegistryCheckpointEnvelope = serde_json::from_slice(&bytes)
            .map_err(|_| invalid_checkpoint("checkpoint JSON is malformed or incomplete"))?;
        if envelope.format != REGISTRY_CHECKPOINT_FORMAT
            || !matches!(envelope.version, 3 | REGISTRY_CHECKPOINT_VERSION)
        {
            return Err(invalid_checkpoint(
                "checkpoint format or schema version is unsupported",
            ));
        }
        let payload_bytes = serde_json::to_vec(&envelope.payload)
            .map_err(|_| invalid_checkpoint("checkpoint payload cannot be canonicalized"))?;
        let expected_checksum = format!("fnv1a64:{:016x}", fnv1a64(&payload_bytes));
        if envelope.checksum != expected_checksum {
            return Err(invalid_checkpoint(
                "checkpoint checksum does not match payload",
            ));
        }
        let payload = envelope.payload;
        if !(1..=MAX_TABLE_REGISTRY_CAPACITY).contains(&payload.max_tables)
            || payload.tables.len() > payload.max_tables
            || payload.next_table_id == 0
            || payload.next_player_id == 0
            || payload.next_hand_id == 0
        {
            return Err(invalid_checkpoint(
                "registry bounds or counters are invalid",
            ));
        }

        let mut tables = BTreeMap::new();
        for restored in payload.tables {
            if restored.table_id.0 == 0 || restored.table_id.0 >= payload.next_table_id {
                return Err(invalid_checkpoint("table identity counter regressed"));
            }
            validate_stored_config(&restored.config, restored.access_verifier.as_ref())
                .map_err(|_| invalid_checkpoint("restored table configuration is invalid"))?;
            if restored.lifecycle.table_size != restored.config.seats {
                return Err(invalid_checkpoint(
                    "lifecycle table size differs from public configuration",
                ));
            }
            let lifecycle = TableLifecycle::restore_between_hand(&restored.lifecycle)
                .map_err(|_| invalid_checkpoint("restored lifecycle is inconsistent"))?;
            if tables
                .insert(
                    restored.table_id,
                    RegisteredTable {
                        config: restored.config,
                        access_verifier: restored.access_verifier,
                        durable_lifecycle: lifecycle.clone(),
                        lifecycle,
                        deterministic_seed: restored.deterministic_seed,
                        tournament: restored.tournament,
                        hand_id: None,
                        seat_sessions: BTreeMap::new(),
                        waiting: VecDeque::new(),
                        pending_departures: BTreeSet::new(),
                        last_activity: Instant::now(),
                        runtime: None,
                        handle: None,
                    },
                )
                .is_some()
            {
                return Err(invalid_checkpoint("duplicate table identity"));
            }
        }

        let mut sessions = BTreeMap::new();
        for restored in payload.sessions {
            if restored.player_id.value() == 0
                || restored.player_id.value() >= payload.next_player_id
            {
                return Err(invalid_checkpoint("player identity counter regressed"));
            }
            let session = GuestSessionId::new(restored.principal_id)
                .map_err(|_| invalid_checkpoint("guest session identity is invalid"))?;
            let table = tables
                .get_mut(&restored.table_id)
                .ok_or_else(|| invalid_checkpoint("session references an unknown table"))?;
            let occupant = table
                .lifecycle
                .seats()
                .seat(restored.seat)
                .ok_or_else(|| invalid_checkpoint("session references a vacant seat"))?;
            if occupant.player_id() != restored.player_id
                || table
                    .seat_sessions
                    .insert(restored.seat, session.clone())
                    .is_some()
            {
                return Err(invalid_checkpoint(
                    "session ownership conflicts with restored lifecycle",
                ));
            }
            if sessions
                .insert(
                    session,
                    SessionRoute {
                        table_id: restored.table_id,
                        player: restored.player_id,
                        seat: restored.seat,
                    },
                )
                .is_some()
            {
                return Err(invalid_checkpoint("duplicate guest session identity"));
            }
        }
        for table in tables.values() {
            if table.seat_sessions.len() != table.lifecycle.seats().occupied_count() {
                return Err(invalid_checkpoint(
                    "every restored occupied seat requires one private session route",
                ));
            }
            table
                .lifecycle
                .seats()
                .occupied()
                .try_fold(0u32, |total, (_, seat)| total.checked_add(seat.stack))
                .ok_or_else(|| invalid_checkpoint("restored table chip total overflowed"))?;
        }

        let credentials =
            CredentialVault::restore(payload.credential_capacity, payload.credentials)
                .map_err(|_| invalid_checkpoint("credential verifier records are invalid"))?;
        let mut registry = Self {
            max_tables: payload.max_tables,
            next_table_id: payload.next_table_id,
            next_player_id: payload.next_player_id,
            next_hand_id: payload.next_hand_id,
            revision: payload.registry_revision,
            tables,
            sessions,
            waiting_sessions: BTreeMap::new(),
            retired_updates: BTreeMap::new(),
            credentials,
            history: RingHistoryStore::default(),
            reconnect_ttl: RECONNECT_CREDENTIAL_TTL,
            last_checkpoint_millis: 0,
            max_checkpoint_millis: 0,
            last_checkpoint_bytes: 0,
        };
        registry.start_restored_hands()?;
        Ok(registry)
    }

    fn start_restored_hands(&mut self) -> Result<(), TableRegistryError> {
        let table_ids = self.tables.keys().copied().collect::<Vec<_>>();
        for table_id in table_ids {
            let table = self
                .tables
                .get(&table_id)
                .expect("collected table identity remains registered");
            let ready = table.tournament.as_ref().map_or_else(
                || table.lifecycle.eligible_count() >= usize::from(table.config.min_players),
                |tournament| {
                    tournament.status() == TournamentStatus::Running
                        && table.lifecycle.eligible_count() >= 2
                },
            );
            if !ready {
                continue;
            }
            let mut lifecycle = table.lifecycle.clone();
            let start = lifecycle.begin_hand().map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
            let hand_id = HandId(self.next_hand_id);
            let following_hand_id = self.next_hand_id.checked_add(1).ok_or_else(|| {
                TableRegistryError::new(
                    TableRegistryErrorCode::AuthorityFailure,
                    "hand identity space is exhausted",
                )
            })?;
            let hand_seed = table
                .deterministic_seed
                .map(|seed| seed.wrapping_add(start.number - 1));
            let hand = match table.tournament.as_ref() {
                Some(tournament) => start.into_hand_with_blinds(
                    table.config.seats,
                    hand_seed,
                    tournament.current_blinds(),
                ),
                None => start.into_hand(table.config.seats, hand_seed),
            }
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
            let runtime = AuthorizedTableRuntime::spawn(ProtocolAuthority::new_paced(
                table_id, hand_id, hand,
            ))
            .map_err(|error| {
                TableRegistryError::new(TableRegistryErrorCode::AuthorityFailure, error.to_string())
            })?;
            let handle = runtime.handle();
            for (&seat, session) in &table.seat_sessions {
                handle
                    .bind(
                        session.clone(),
                        table_id,
                        hand_id,
                        SessionRole::Player { seat },
                    )
                    .map_err(|error| {
                        TableRegistryError::new(
                            TableRegistryErrorCode::AuthorityFailure,
                            error.to_string(),
                        )
                    })?;
                handle.disconnect(session.clone()).map_err(|error| {
                    TableRegistryError::new(
                        TableRegistryErrorCode::AuthorityFailure,
                        error.to_string(),
                    )
                })?;
            }
            let table = self
                .tables
                .get_mut(&table_id)
                .expect("restored table remains registered");
            table.lifecycle = lifecycle;
            table.hand_id = Some(hand_id);
            table.handle = Some(handle);
            table.runtime = Some(runtime);
            self.next_hand_id = following_hand_id;
        }
        Ok(())
    }

    pub fn close_and_remove(&mut self, table_id: TableId) -> Result<(), TableRegistryError> {
        let table = self
            .tables
            .get_mut(&table_id)
            .ok_or_else(|| unknown_table(table_id))?;
        if table.runtime.is_some()
            || table.lifecycle.hand_active()
            || table.lifecycle.seats().occupied_count() != 0
            || table.lifecycle.reservations().next().is_some()
        {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::TableNotRemovable,
                "only an empty inactive table can be removed",
            ));
        }
        table.lifecycle.close().map_err(|error| {
            TableRegistryError::new(TableRegistryErrorCode::TableNotRemovable, error.to_string())
        })?;
        self.tables.remove(&table_id);
        self.bump_revision();
        Ok(())
    }

    /// Bounded monotonic cleanup. Active hands, occupied seats, reservations,
    /// waiters, and routed sessions are all hard retention conditions.
    pub fn expire_inactive(&mut self, idle_for: Duration) -> ExpirySweep {
        self.expire_inactive_at(Instant::now(), idle_for)
    }

    fn expire_inactive_at(&mut self, now: Instant, idle_for: Duration) -> ExpirySweep {
        let scanned = self.tables.len();
        let expired_ids = self
            .tables
            .iter()
            .filter_map(|(&table_id, table)| {
                let safe = table.runtime.is_none()
                    && !table.lifecycle.hand_active()
                    && table.lifecycle.seats().occupied_count() == 0
                    && table.lifecycle.reservations().next().is_none()
                    && table.waiting.is_empty()
                    && table.seat_sessions.is_empty()
                    && table.pending_departures.is_empty();
                (safe && now.saturating_duration_since(table.last_activity) >= idle_for)
                    .then_some(table_id)
            })
            .collect::<Vec<_>>();
        for table_id in &expired_ids {
            self.tables.remove(table_id);
        }
        if !expired_ids.is_empty() {
            self.bump_revision();
        }
        ExpirySweep {
            scanned,
            expired: expired_ids.len(),
            retained: scanned - expired_ids.len(),
        }
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
}

fn validate_config(config: &PublicTableConfig) -> Result<(), TableRegistryError> {
    if config.name.is_empty()
        || config.name.len() > MAX_PUBLIC_TABLE_NAME_BYTES
        || !config
            .name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'-' | b'_'))
    {
        return Err(TableRegistryError::new(
            TableRegistryErrorCode::InvalidTableName,
            format!("table name must contain 1 to {MAX_PUBLIC_TABLE_NAME_BYTES} safe ASCII bytes"),
        ));
    }
    if config.starting_stack == 0 {
        return Err(TableRegistryError::new(
            TableRegistryErrorCode::InvalidStartingStack,
            "starting stack must be positive",
        ));
    }
    if config.min_players < 2 || config.min_players > config.seats.get() {
        return Err(TableRegistryError::new(
            TableRegistryErrorCode::InvalidCapacity,
            "minimum players must be between two and the table seat count",
        ));
    }
    match config.visibility {
        TableVisibility::PasswordProtected => {
            let password = config.join_code.as_deref().unwrap_or_default();
            if !(4..=96).contains(&password.len())
                || !password.bytes().all(|b| b.is_ascii_graphic() || b == b' ')
            {
                return Err(TableRegistryError::new(
                    TableRegistryErrorCode::TableNotJoinable,
                    "Password must contain 4-96 printable characters",
                ));
            }
        }
        TableVisibility::Public if config.join_code.is_some() => {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::TableNotJoinable,
                "public tables cannot carry a private join credential",
            ));
        }
        TableVisibility::Unlisted | TableVisibility::Private => {
            let code = config.join_code.as_deref().unwrap_or_default();
            if !(MIN_PRIVATE_JOIN_CODE_BYTES..=MAX_PRIVATE_JOIN_CODE_BYTES).contains(&code.len())
                || !code
                    .bytes()
                    .all(|byte| byte.is_ascii_graphic() && !byte.is_ascii_whitespace())
            {
                return Err(TableRegistryError::new(
                    TableRegistryErrorCode::TableNotJoinable,
                    format!("non-public table join credential must contain {MIN_PRIVATE_JOIN_CODE_BYTES} to {MAX_PRIVATE_JOIN_CODE_BYTES} printable non-space bytes"),
                ));
            }
        }
        TableVisibility::Public => {}
    }
    Ok(())
}

fn validate_stored_config(
    config: &PublicTableConfig,
    access_verifier: Option<&AccessVerifier>,
) -> Result<(), TableRegistryError> {
    let mut validation_config = config.clone();
    match config.visibility {
        TableVisibility::Public if access_verifier.is_some() || config.join_code.is_some() => {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::InvalidCheckpoint,
                "public table cannot contain access verifier material",
            ));
        }
        TableVisibility::PasswordProtected
        | TableVisibility::Unlisted
        | TableVisibility::Private
            if access_verifier.is_none_or(|verifier| !verifier.is_valid())
                || config.join_code.is_some() =>
        {
            return Err(TableRegistryError::new(
                TableRegistryErrorCode::InvalidCheckpoint,
                "non-public table requires one valid non-recoverable access verifier",
            ));
        }
        _ => {}
    }
    validation_config.join_code = match validation_config.visibility {
        TableVisibility::Public => None,
        TableVisibility::PasswordProtected
        | TableVisibility::Unlisted
        | TableVisibility::Private => Some("checkpoint-validation-code".to_string()),
    };
    validate_config(&validation_config)
}

fn table_allows(table: &RegisteredTable, supplied: Option<&str>) -> bool {
    match table.config.visibility {
        TableVisibility::Public => true,
        TableVisibility::PasswordProtected
        | TableVisibility::Unlisted
        | TableVisibility::Private => match (&table.access_verifier, supplied) {
            (Some(verifier), Some(candidate)) => verifier.verify(candidate),
            _ => false,
        },
    }
}

fn checkpoint_serialization_error(error: serde_json::Error) -> TableRegistryError {
    TableRegistryError::new(
        TableRegistryErrorCode::PersistenceFailure,
        format!("checkpoint serialization failed: {error}"),
    )
}

fn checkpoint_io_error(error: std::io::Error) -> TableRegistryError {
    TableRegistryError::new(
        TableRegistryErrorCode::PersistenceFailure,
        format!("checkpoint I/O failed: {error}"),
    )
}

fn invalid_checkpoint(message: impl Into<String>) -> TableRegistryError {
    TableRegistryError::new(TableRegistryErrorCode::InvalidCheckpoint, message)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn checkpoint_temporary_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .expect("validated checkpoint path has a file name")
        .to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[cfg(not(windows))]
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
pub(crate) fn atomic_replace(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::time::Duration;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: both path buffers are NUL-terminated and remain alive for the
    // duration of the synchronous OS call; optional pointers are null.
    for attempt in 0..=50 {
        let replaced = unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if replaced != 0 {
            return Ok(());
        }

        let error = std::io::Error::last_os_error();
        let transient_contention = matches!(error.raw_os_error(), Some(5 | 32 | 1175));
        if !transient_contention || attempt == 50 {
            return Err(error);
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    unreachable!("bounded replacement loop always returns")
}

fn unknown_table(table_id: TableId) -> TableRegistryError {
    TableRegistryError::new(
        TableRegistryErrorCode::UnknownTable,
        format!("table {} does not exist", table_id.0),
    )
}

fn lifecycle_stacks(lifecycle: &TableLifecycle) -> Vec<(SeatId, u32)> {
    lifecycle
        .seats()
        .occupied()
        .map(|(seat, state)| (seat, state.stack))
        .collect()
}

const fn table_id_to_hand(_table_id: TableId, hand_id: HandId) -> HandId {
    hand_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::actions::Action;
    use crate::lobby::PublicTableFilter;
    use crate::protocol::CommandEnvelope;

    fn config(name: &str, seats: u8) -> PublicTableConfig {
        PublicTableConfig {
            name: name.to_string(),
            seats: crate::game::seat::TableSize::new(seats).unwrap(),
            starting_stack: 100,
            min_players: 2,
            visibility: TableVisibility::Public,
            join_code: None,
        }
    }

    fn guest(value: &str) -> GuestSessionId {
        GuestSessionId::new(value).unwrap()
    }

    fn checkpoint_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "terminal-poker-{label}-{}.json",
            std::process::id()
        ))
    }

    #[test]
    fn lobby_open_locked_hidden_privacy_and_cancellation_recovery() {
        let mut registry = TableRegistry::new(4).unwrap();
        let open = registry
            .create_tournament(TournamentConfig::recommended(2, ""), Some(191))
            .unwrap();
        let locked = registry
            .create_tournament(TournamentConfig::recommended(2, " Pass word "), Some(192))
            .unwrap();
        let mut hidden = config("Legacy Hidden", 2);
        hidden.visibility = TableVisibility::Private;
        hidden.join_code = Some("legacy-secret-1234567890123456".into());
        let hidden = registry.create(hidden, None).unwrap();
        let rows = registry.list(&PublicTableFilter::default());
        assert_eq!(rows.len(), 2);
        assert_eq!(open.visibility, TableVisibility::Public);
        assert_eq!(locked.visibility, TableVisibility::PasswordProtected);
        let public = serde_json::to_string(&rows).unwrap();
        for secret in [" Pass word ", "join_code", "verifier", "salt_hex"] {
            assert!(!public.contains(secret));
        }
        let revision = registry.revision();
        for access in [None, Some("Pass word"), Some(" pass word ")] {
            assert!(registry
                .join_or_wait_with_access(guest("wrong"), locked.table_id, None, access)
                .is_err());
            assert_eq!(registry.revision(), revision);
        }
        let player = guest("cancelled-registration");
        registry
            .join_or_wait_with_access(player.clone(), locked.table_id, None, Some(" Pass word "))
            .unwrap();
        registry.cancel_wait(&player, locked.table_id).unwrap();
        let summary = registry
            .inspect_with_access(locked.table_id, Some(" Pass word "))
            .unwrap();
        assert_eq!(summary.occupied, 0);
        assert_eq!(summary.tournament.unwrap().registered, 0);
        assert!(registry.player_for_session(&player).is_none());
        let path = checkpoint_path("lobby-v4");
        registry.save_checkpoint(&path).unwrap();
        let bytes = fs::read_to_string(&path).unwrap();
        assert!(bytes.contains("argon2id-v1"));
        assert!(!bytes.contains(" Pass word "));
        let restored = TableRegistry::load_checkpoint(&path).unwrap();
        assert_eq!(restored.list(&PublicTableFilter::default()).len(), 2);
        assert!(restored
            .inspect_with_access(locked.table_id, Some(" Pass word "))
            .is_ok());
        assert!(restored
            .inspect_with_access(hidden.table_id, Some("legacy-secret-1234567890123456"))
            .is_ok());
        // The unchanged schema-v3 envelope still reads legacy SHA-256 hidden tables.
        let mut legacy = TableRegistry::new(1).unwrap();
        let mut cfg = config("Legacy", 2);
        cfg.visibility = TableVisibility::Private;
        cfg.join_code = Some("legacy-secret-1234567890123456".into());
        legacy.create(cfg, None).unwrap();
        legacy.save_checkpoint(&path).unwrap();
        let mut json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        json["version"] = 3.into();
        fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
        assert!(TableRegistry::load_checkpoint(&path).is_ok());
        fs::remove_file(path).unwrap();
        registry
            .join_or_wait(guest("open-a"), open.table_id, None)
            .unwrap();
        registry
            .join_or_wait(guest("open-b"), open.table_id, None)
            .unwrap();
        let revision = registry.revision();
        assert!(registry
            .cancel_wait(&guest("open-a"), open.table_id)
            .is_err());
        assert!(registry
            .join_or_wait(guest("late"), open.table_id, None)
            .is_err());
        assert_eq!(registry.revision(), revision);
    }

    #[test]
    fn capacity_is_bounded_without_evicting_existing_tables() {
        assert!(matches!(
            TableRegistry::new(0),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::InvalidCapacity,
                ..
            })
        ));
        let mut registry = TableRegistry::new(2).unwrap();
        let first = registry.create(config("Alpha", 2), Some(1)).unwrap();
        let second = registry.create(config("Bravo", 2), Some(2)).unwrap();
        let revision = registry.revision();
        let before = registry.list(&PublicTableFilter::default());
        let overflow = registry.create(config("Charlie", 2), Some(3));
        assert!(matches!(
            overflow,
            Err(TableRegistryError {
                code: TableRegistryErrorCode::CapacityReached,
                ..
            })
        ));
        assert_eq!(registry.revision(), revision);
        assert_eq!(registry.list(&PublicTableFilter::default()), before);
        assert_eq!(before, vec![first, second]);
    }

    #[test]
    fn private_tournament_registers_exact_field_then_starts_with_level_blinds() {
        let invite = "T15-opaque-invite-123456789";
        let mut registry = TableRegistry::new(2).unwrap();
        let config = TournamentConfig::recommended(3, invite);
        let table = registry.create_tournament(config, Some(1500)).unwrap();
        assert_eq!(table.visibility, TableVisibility::PasswordProtected);
        assert_eq!(table.occupied, 0);
        assert_eq!(table.small_blind, 25);
        assert_eq!(table.big_blind, 50);
        assert!(!serde_json::to_string(&table).unwrap().contains(invite));

        for index in 0u8..2 {
            let outcome = registry
                .join_or_wait_with_access(
                    guest(&format!("tourney-{index}")),
                    table.table_id,
                    None,
                    Some(invite),
                )
                .unwrap();
            let AdmissionOutcome::Joined(joined) = outcome else {
                panic!("fixed registration should admit immediately");
            };
            assert!(!joined.ready);
            assert_eq!(joined.table.tournament.unwrap().registered, index + 1);
        }
        let AdmissionOutcome::Joined(started) = registry
            .join_or_wait_with_access(guest("tourney-2"), table.table_id, None, Some(invite))
            .unwrap()
        else {
            panic!("last entrant should start the tournament");
        };
        assert!(started.ready);
        assert_eq!(started.table.status, PublicTableStatus::Running);
        assert_eq!(
            started.table.tournament.unwrap().status,
            TournamentStatus::Running
        );
        assert!(matches!(
            registry.join_or_wait_with_access(
                guest("late-player"),
                table.table_id,
                None,
                Some(invite)
            ),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::TableNotJoinable,
                ..
            })
        ));
    }

    #[test]
    fn tournament_checkpoint_restores_running_level_without_recoverable_invite() {
        let invite = "T15-checkpoint-invite-123456";
        let path = checkpoint_path("tournament-v3");
        let mut registry = TableRegistry::new(2).unwrap();
        let table = registry
            .create_tournament(TournamentConfig::recommended(2, invite), Some(1600))
            .unwrap();
        registry
            .join_or_wait_with_access(guest("checkpoint-a"), table.table_id, None, Some(invite))
            .unwrap();
        registry
            .join_or_wait_with_access(guest("checkpoint-b"), table.table_id, None, Some(invite))
            .unwrap();
        registry.save_checkpoint(&path).unwrap();
        let persisted = fs::read_to_string(&path).unwrap();
        assert!(!persisted.contains(invite));
        assert!(!persisted.contains("join_code"));

        let restored = TableRegistry::load_checkpoint(&path).unwrap();
        let summary = restored
            .inspect_with_access(table.table_id, Some(invite))
            .unwrap();
        assert_eq!(summary.status, PublicTableStatus::Running);
        assert_eq!(summary.small_blind, 25);
        assert_eq!(summary.big_blind, 50);
        assert_eq!(
            summary.tournament.unwrap().status,
            TournamentStatus::Running
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn expiry_sweep_is_bounded_observable_and_never_retires_routed_or_active_tables() {
        let mut registry = TableRegistry::new(4).unwrap();
        let empty = registry.create(config("Empty", 2), Some(1)).unwrap();
        let routed = registry.create(config("Routed", 2), Some(2)).unwrap();
        registry
            .join(guest("routed-a"), routed.table_id, None)
            .unwrap();
        let active = registry.create(config("Active", 2), Some(3)).unwrap();
        registry
            .join(guest("active-a"), active.table_id, None)
            .unwrap();
        registry
            .join(guest("active-b"), active.table_id, None)
            .unwrap();

        let sweep = registry.expire_inactive_at(Instant::now(), Duration::ZERO);
        assert_eq!(
            sweep,
            ExpirySweep {
                scanned: 3,
                expired: 1,
                retained: 2
            }
        );
        assert!(matches!(
            registry.inspect(empty.table_id),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::UnknownTable,
                ..
            })
        ));
        assert_eq!(registry.inspect(routed.table_id).unwrap().occupied, 1);
        assert_eq!(
            registry.inspect(active.table_id).unwrap().status,
            PublicTableStatus::Running
        );
        assert!(registry.route_for_session(&guest("active-a")).is_ok());
    }

    #[test]
    fn structured_health_is_bounded_and_contains_no_cards_sessions_or_credentials() {
        let mut registry = TableRegistry::new(4).unwrap();
        let table = registry.create(config("Health", 2), Some(41)).unwrap();
        registry
            .join(guest("health-a"), table.table_id, None)
            .unwrap();
        registry
            .join(guest("health-b"), table.table_id, None)
            .unwrap();
        registry
            .join_or_wait(guest("health-wait"), table.table_id, None)
            .unwrap();
        let health = registry.health().unwrap();
        assert!(health.healthy);
        assert_eq!(health.tables, 1);
        assert_eq!(health.routed_sessions, 2);
        assert_eq!(health.waiting_sessions, 1);
        assert_eq!(health.table_health.len(), 1);
        assert_eq!(
            health.table_health[0].hand_id,
            registry
                .inspect(table.table_id)
                .ok()
                .and_then(|_| registry.tables[&table.table_id].hand_id)
        );
        let json = serde_json::to_string(&health).unwrap();
        for forbidden in [
            "health-a",
            "health-b",
            "health-wait",
            "hole_cards",
            "deck",
            "join_code",
            "credential",
            "checkpoint_payload",
        ] {
            assert!(!json.contains(forbidden), "health leaked {forbidden}");
        }
        assert!(json.len() < 4096);
    }

    #[test]
    fn checkpoint_is_bounded_checksummed_atomic_and_excludes_live_authority() {
        let path = checkpoint_path("allowlist");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(checkpoint_temporary_path(&path));
        let mut registry = TableRegistry::new(4).unwrap();
        let alpha = registry.create(config("Alpha", 2), Some(71)).unwrap();
        registry
            .join(
                guest("alpha-a"),
                alpha.table_id,
                Some(SeatId::new(0).unwrap()),
            )
            .unwrap();
        registry
            .join(
                guest("alpha-b"),
                alpha.table_id,
                Some(SeatId::new(1).unwrap()),
            )
            .unwrap();

        let first = registry.save_checkpoint(&path).unwrap();
        assert_eq!(first.version, REGISTRY_CHECKPOINT_VERSION);
        assert_eq!(first.tables, 1);
        assert_eq!(first.sessions, 2);
        assert!(first.bytes <= MAX_REGISTRY_CHECKPOINT_BYTES);
        assert!(first.checksum.starts_with("fnv1a64:"));
        let serialized = fs::read_to_string(&path).unwrap();
        for forbidden in [
            "hole_cards",
            "deck",
            "awards",
            "command",
            "deadline",
            "subscription",
            "runtime",
            "handle",
            "socket",
        ] {
            assert!(!serialized.contains(forbidden), "leaked field {forbidden}");
        }

        registry.create(config("Bravo", 4), Some(72)).unwrap();
        let second = registry.save_checkpoint(&path).unwrap();
        assert_eq!(second.tables, 2);
        assert_ne!(first.checksum, second.checksum);
        assert!(!checkpoint_temporary_path(&path).exists());
        let published: RegistryCheckpointEnvelope =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(published.payload.tables.len(), 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn restore_validates_whole_checkpoint_and_starts_fresh_monotonic_hands() {
        let path = checkpoint_path("restore");
        let corrupt_path = checkpoint_path("restore-corrupt");
        let version_path = checkpoint_path("restore-version");
        for candidate in [&path, &corrupt_path, &version_path] {
            let _ = fs::remove_file(candidate);
            let _ = fs::remove_file(checkpoint_temporary_path(candidate));
        }
        let mut source = TableRegistry::new(4).unwrap();
        let alpha = source.create(config("Alpha", 2), Some(801)).unwrap();
        let bravo = source.create(config("Bravo", 2), Some(802)).unwrap();
        for (table, prefix) in [(alpha.table_id, "alpha"), (bravo.table_id, "bravo")] {
            source
                .join(
                    guest(&format!("{prefix}-a")),
                    table,
                    Some(SeatId::new(0).unwrap()),
                )
                .unwrap();
            source
                .join(
                    guest(&format!("{prefix}-b")),
                    table,
                    Some(SeatId::new(1).unwrap()),
                )
                .unwrap();
        }
        let alpha_actor = guest("alpha-a");
        let alpha_route = source.route_for_session(&alpha_actor).unwrap();
        let before = alpha_route.handle.snapshot(alpha_actor.clone()).unwrap();
        let to_act = before.snapshot.to_act.unwrap();
        let acting_session = if to_act == SeatId::new(0).unwrap() {
            guest("alpha-a")
        } else {
            guest("alpha-b")
        };
        let acting_route = source.route_for_session(&acting_session).unwrap();
        let terminal = acting_route
            .handle
            .submit(
                acting_session,
                CommandEnvelope::act_for_hand(
                    "settle-before-save",
                    alpha.table_id,
                    acting_route.hand_id,
                    before.revision,
                    to_act,
                    Action::Fold,
                ),
            )
            .unwrap();
        let successor = source
            .rollover_completed_hand(alpha.table_id, terminal.snapshot.hand_id)
            .unwrap()
            .unwrap();
        let bravo_before_restart = source.route_for_session(&guest("bravo-a")).unwrap().hand_id;
        let source_revision = source.revision();
        let source_tables = source.list(&PublicTableFilter::default());
        source.save_checkpoint(&path).unwrap();

        let mut restored = TableRegistry::load_checkpoint(&path).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.revision(), source_revision);
        for (session, prior_hand) in [
            (guest("alpha-a"), successor),
            (guest("bravo-a"), bravo_before_restart),
        ] {
            let route = restored.route_for_session(&session).unwrap();
            assert!(route.hand_id > prior_hand);
            route.handle.reconnect(session.clone()).unwrap();
            let snapshot = route.handle.snapshot(session).unwrap();
            assert_eq!(snapshot.revision, 0);
            assert!(snapshot.snapshot.awards.is_empty());
            assert!(!matches!(
                snapshot.snapshot.phase,
                MultiwayPhase::Showdown | MultiwayPhase::HandComplete
            ));
            assert_eq!(
                snapshot
                    .snapshot
                    .seats
                    .iter()
                    .map(|seat| seat.stack)
                    .sum::<u32>()
                    + snapshot.snapshot.pot_total,
                200
            );
        }
        let charlie = restored.create(config("Charlie", 2), Some(803)).unwrap();
        assert!(charlie.table_id > bravo.table_id);
        restored
            .join(guest("charlie-a"), charlie.table_id, None)
            .unwrap();
        assert!(
            restored
                .player_for_session(&guest("charlie-a"))
                .unwrap()
                .value()
                > 4
        );

        let valid = fs::read_to_string(&path).unwrap();
        fs::write(&corrupt_path, valid.replacen("Alpha", "Alphx", 1)).unwrap();
        assert!(matches!(
            TableRegistry::load_checkpoint(&corrupt_path),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::InvalidCheckpoint,
                ..
            })
        ));
        fs::write(
            &version_path,
            valid.replacen(
                &format!("\"version\": {REGISTRY_CHECKPOINT_VERSION}"),
                "\"version\": 99",
                1,
            ),
        )
        .unwrap();
        assert!(matches!(
            TableRegistry::load_checkpoint(&version_path),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::InvalidCheckpoint,
                ..
            })
        ));
        assert_eq!(source.revision(), source_revision);
        assert_eq!(source.list(&PublicTableFilter::default()), source_tables);

        for candidate in [path, corrupt_path, version_path] {
            fs::remove_file(candidate).unwrap();
        }
    }

    #[test]
    fn identities_are_stable_and_public_lists_are_sorted_and_filterable() {
        let mut registry = TableRegistry::new(4).unwrap();
        let alpha = registry.create(config("Alpha", 2), Some(1)).unwrap();
        let bravo = registry.create(config("Bravo", 4), Some(2)).unwrap();
        assert_eq!(alpha.table_id, TableId(1));
        assert_eq!(bravo.table_id, TableId(2));
        assert_eq!(registry.inspect(TableId(1)).unwrap(), alpha);
        assert_eq!(
            registry.list(&PublicTableFilter {
                seats: Some(crate::game::seat::TableSize::new(4).unwrap()),
                ..PublicTableFilter::default()
            }),
            vec![bravo]
        );
    }

    #[test]
    fn private_and_unlisted_tables_are_hidden_and_fail_closed_without_their_code() {
        let mut registry = TableRegistry::new(4).unwrap();
        let public = registry.create(config("Public", 2), Some(1)).unwrap();
        let code = "private-beta-code-0123456789";
        let mut private_config = config("Private", 6);
        private_config.visibility = TableVisibility::Private;
        private_config.join_code = Some(code.to_string());
        let private = registry.create(private_config, Some(2)).unwrap();
        let mut unlisted_config = config("Unlisted", 9);
        unlisted_config.visibility = TableVisibility::Unlisted;
        unlisted_config.join_code = Some("unlisted-beta-code-01234567".to_string());
        registry.create(unlisted_config, Some(3)).unwrap();

        assert_eq!(registry.list(&PublicTableFilter::default()), vec![public]);
        for supplied in [None, Some("wrong-private-code-000000")] {
            let error = registry
                .inspect_with_access(private.table_id, supplied)
                .unwrap_err();
            assert_eq!(error.code, TableRegistryErrorCode::UnknownTable);
            assert!(!error.message.contains("Private"));
            assert!(!error.message.contains(code));
        }
        let visible = registry
            .inspect_with_access(private.table_id, Some(code))
            .unwrap();
        assert_eq!(visible.visibility, TableVisibility::Private);
        let json = serde_json::to_string(&visible).unwrap();
        assert!(!json.contains(code));
        assert!(matches!(
            registry.join_or_wait_with_access(guest("denied"), private.table_id, None, None),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::UnknownTable,
                ..
            })
        ));
        assert!(matches!(
            registry.join_or_wait_with_access(guest("allowed"), private.table_id, None, Some(code)),
            Ok(AdmissionOutcome::Joined(_))
        ));
    }

    #[test]
    fn private_checkpoint_keeps_only_verifiers_and_server_issued_principals() {
        let path = checkpoint_path("private-verifiers");
        let _ = fs::remove_file(&path);
        let code = "private-restart-code-0123456789";
        let caller_label_a = "caller-chosen-alpha";
        let caller_label_b = "caller-chosen-bravo";
        let mut registry = TableRegistry::new(4).unwrap();
        let mut private = config("Private", 2);
        private.visibility = TableVisibility::Private;
        private.join_code = Some(code.to_string());
        let table = registry.create(private, Some(991)).unwrap();
        let principal_a = GuestSessionId::random();
        let principal_b = GuestSessionId::random();
        registry
            .join_or_wait_with_access(principal_a.clone(), table.table_id, None, Some(code))
            .unwrap();
        registry
            .join_or_wait_with_access(principal_b, table.table_id, None, Some(code))
            .unwrap();
        let grant = registry.issue_reconnect_credential(&principal_a).unwrap();
        registry.save_checkpoint(&path).unwrap();
        let serialized = fs::read_to_string(&path).unwrap();
        for forbidden in [
            code,
            grant.token.expose_to_wire(),
            caller_label_a,
            caller_label_b,
            "guest_session_id",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "checkpoint leaked {forbidden}"
            );
        }
        assert!(serialized.contains("access_verifier"));
        assert!(serialized.contains("principal_id"));

        let mut restored = TableRegistry::load_checkpoint(&path).unwrap();
        assert!(restored
            .inspect_with_access(table.table_id, Some(code))
            .is_ok());
        assert!(matches!(
            restored.inspect_with_access(table.table_id, Some("wrong-private-code-0123456789")),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::UnknownTable,
                ..
            })
        ));
        let (restored_principal, rotated) = restored
            .authenticate_and_rotate_reconnect(&grant.token)
            .unwrap();
        assert_eq!(restored_principal, principal_a);
        assert_ne!(rotated.token.expose_to_wire(), grant.token.expose_to_wire());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejected_unknown_and_unavailable_joins_do_not_consume_identity_or_revision() {
        let mut registry = TableRegistry::new(2).unwrap();
        let table = registry.create(config("Alpha", 2), Some(1)).unwrap();
        let revision = registry.revision();
        assert!(matches!(
            registry.join(guest("unknown"), TableId(99), None),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::UnknownTable,
                ..
            })
        ));
        assert!(matches!(
            registry.join(
                guest("bad-seat"),
                table.table_id,
                Some(SeatId::new(8).unwrap())
            ),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::SeatUnavailable,
                ..
            })
        ));
        assert_eq!(registry.revision(), revision);
        let valid = guest("valid");
        registry.join(valid.clone(), table.table_id, None).unwrap();
        assert_eq!(registry.player_for_session(&valid), Some(PlayerId::new(1)));
    }

    #[test]
    fn default_capacity_bounds_public_results_to_sixteen() {
        let mut registry = TableRegistry::new(DEFAULT_TABLE_REGISTRY_CAPACITY).unwrap();
        for index in 0..DEFAULT_TABLE_REGISTRY_CAPACITY {
            registry
                .create(config(&format!("Table {index}"), 2), Some(index as u64))
                .unwrap();
        }
        let tables = registry.list(&PublicTableFilter::default());
        assert_eq!(tables.len(), DEFAULT_TABLE_REGISTRY_CAPACITY);
        assert!(tables
            .windows(2)
            .all(|pair| pair[0].table_id < pair[1].table_id));
        assert!(matches!(
            registry.create(config("Overflow", 2), None),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::CapacityReached,
                ..
            })
        ));
    }

    #[test]
    fn two_players_start_exactly_one_authority_and_receive_private_routes() {
        let mut registry = TableRegistry::new(4).unwrap();
        let table = registry.create(config("Alpha", 2), Some(91)).unwrap();
        let first = guest("guest-a");
        let second = guest("guest-b");
        let waiting = registry
            .join(first.clone(), table.table_id, Some(SeatId::new(0).unwrap()))
            .unwrap();
        assert!(!waiting.ready);
        assert!(matches!(
            registry.route_for_session(&first),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::TableNotReady,
                ..
            })
        ));
        let started = registry
            .join(
                second.clone(),
                table.table_id,
                Some(SeatId::new(1).unwrap()),
            )
            .unwrap();
        assert!(started.ready);
        let first_route = registry.route_for_session(&first).unwrap();
        let second_route = registry.route_for_session(&second).unwrap();
        assert_eq!(first_route.table_id, table.table_id);
        assert_eq!(second_route.table_id, table.table_id);
        assert_eq!(first_route.hand_id, second_route.hand_id);
        assert_ne!(first_route.seat, second_route.seat);
        assert_eq!(first_route.handle.metrics().unwrap().active_bindings, 2);
    }

    #[test]
    fn bounded_waiting_promotes_once_at_the_hand_boundary_and_cancels_atomically() {
        let mut registry = TableRegistry::new(2).unwrap();
        let table = registry.create(config("Queue", 2), Some(901)).unwrap();
        let first = guest("queue-a");
        let second = guest("queue-b");
        registry.join(first.clone(), table.table_id, None).unwrap();
        registry.join(second.clone(), table.table_id, None).unwrap();

        let promoted = guest("queue-next");
        let cancelled = guest("queue-cancel");
        assert!(matches!(
            registry
                .join_or_wait(promoted.clone(), table.table_id, None)
                .unwrap(),
            AdmissionOutcome::Waiting(WaitOutcome { position: 1, .. })
        ));
        assert!(matches!(
            registry
                .join_or_wait(cancelled.clone(), table.table_id, None)
                .unwrap(),
            AdmissionOutcome::Waiting(WaitOutcome { position: 2, .. })
        ));
        registry.cancel_wait(&cancelled, table.table_id).unwrap();
        assert!(matches!(
            registry.admission_status(&cancelled),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::SessionNotRouted,
                ..
            })
        ));

        registry.request_leave(&first).unwrap();
        let route = registry.route_for_session(&first).unwrap();
        let snapshot = route.handle.snapshot(first.clone()).unwrap();
        let actor = snapshot.snapshot.to_act.unwrap();
        let actor_session = if actor == route.seat {
            first.clone()
        } else {
            second.clone()
        };
        let actor_route = registry.route_for_session(&actor_session).unwrap();
        let terminal = actor_route
            .handle
            .submit(
                actor_session,
                CommandEnvelope::act_for_hand(
                    "queue-boundary-fold",
                    table.table_id,
                    actor_route.hand_id,
                    snapshot.revision,
                    actor,
                    Action::Fold,
                ),
            )
            .unwrap();
        registry
            .rollover_completed_hand(table.table_id, terminal.snapshot.hand_id)
            .unwrap();

        assert!(registry.player_for_session(&first).is_none());
        assert!(matches!(
            registry.admission_status(&promoted),
            Ok(AdmissionOutcome::Joined(_))
        ));
        assert_eq!(registry.inspect(table.table_id).unwrap().waiting, 0);
        let seats = [
            registry.route_for_session(&second).unwrap().seat,
            registry.route_for_session(&promoted).unwrap().seat,
        ];
        assert_ne!(seats[0], seats[1]);
    }

    #[test]
    fn waiting_capacity_rejects_overflow_without_consuming_a_player_identity() {
        let mut registry = TableRegistry::new(1).unwrap();
        let table = registry.create(config("Bounded", 2), Some(902)).unwrap();
        registry
            .join(guest("bound-a"), table.table_id, None)
            .unwrap();
        registry
            .join(guest("bound-b"), table.table_id, None)
            .unwrap();
        for index in 0..MAX_TABLE_WAITING_ENTRIES {
            registry
                .join_or_wait(guest(&format!("wait-{index}")), table.table_id, None)
                .unwrap();
        }
        assert!(matches!(
            registry.join_or_wait(guest("overflow"), table.table_id, None),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::WaitlistFull,
                ..
            })
        ));
        assert_eq!(
            registry.inspect(table.table_id).unwrap().waiting as usize,
            MAX_TABLE_WAITING_ENTRIES
        );
        assert_eq!(registry.next_player_id, 3);
    }

    #[test]
    fn terminal_hands_roll_over_once_with_fresh_identity_and_clean_authority() {
        let mut registry = TableRegistry::new(2).unwrap();
        let table = registry.create(config("Rollover", 2), Some(91)).unwrap();
        let first = guest("roll-a");
        let second = guest("roll-b");
        registry
            .join(first.clone(), table.table_id, Some(SeatId::new(0).unwrap()))
            .unwrap();
        registry
            .join(
                second.clone(),
                table.table_id,
                Some(SeatId::new(1).unwrap()),
            )
            .unwrap();

        let complete_by_folding = |registry: &TableRegistry, suffix: &str| {
            let route = registry.route_for_session(&first).unwrap();
            let snapshot = route.handle.snapshot(first.clone()).unwrap();
            let actor = snapshot.snapshot.to_act.unwrap();
            let actor_session = if actor == SeatId::new(0).unwrap() {
                first.clone()
            } else {
                second.clone()
            };
            let actor_route = registry.route_for_session(&actor_session).unwrap();
            actor_route
                .handle
                .submit(
                    actor_session,
                    CommandEnvelope::act_for_hand(
                        format!("fold-{suffix}"),
                        table.table_id,
                        actor_route.hand_id,
                        snapshot.revision,
                        actor,
                        Action::Fold,
                    ),
                )
                .unwrap()
        };

        let first_terminal = complete_by_folding(&registry, "one");
        assert!(matches!(
            first_terminal.snapshot.snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ));
        let first_hand = first_terminal.snapshot.hand_id;
        let second_hand = registry
            .rollover_completed_hand(table.table_id, first_hand)
            .unwrap()
            .unwrap();
        assert!(second_hand > first_hand);
        assert_eq!(
            registry
                .rollover_completed_hand(table.table_id, first_hand)
                .unwrap(),
            None
        );
        let fresh = registry
            .route_for_session(&first)
            .unwrap()
            .handle
            .snapshot(first.clone())
            .unwrap();
        assert_eq!(fresh.hand_id, second_hand);
        assert_eq!(fresh.revision, 0);
        assert!(fresh.snapshot.awards.is_empty());
        assert!(!matches!(
            fresh.snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ));

        let second_terminal = complete_by_folding(&registry, "two");
        let third_hand = registry
            .rollover_completed_hand(table.table_id, second_terminal.snapshot.hand_id)
            .unwrap()
            .unwrap();
        assert!(third_hand > second_hand);
        let third = registry
            .route_for_session(&first)
            .unwrap()
            .handle
            .snapshot(first)
            .unwrap();
        assert_eq!(third.hand_id, third_hand);
        assert_eq!(third.revision, 0);
        assert!(third.snapshot.awards.is_empty());
    }

    #[test]
    fn one_session_cannot_join_two_tables_and_wrong_table_route_is_mutation_free() {
        let mut registry = TableRegistry::new(4).unwrap();
        let alpha = registry.create(config("Alpha", 2), Some(1)).unwrap();
        let bravo = registry.create(config("Bravo", 2), Some(2)).unwrap();
        let session = guest("guest-a");
        registry
            .join(
                session.clone(),
                alpha.table_id,
                Some(SeatId::new(0).unwrap()),
            )
            .unwrap();
        let revision = registry.revision();
        let before = registry.list(&PublicTableFilter::default());
        assert!(matches!(
            registry.join(session.clone(), bravo.table_id, None),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::SessionAlreadyRouted,
                ..
            })
        ));
        assert!(matches!(
            registry.route(&session, bravo.table_id),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::WrongTable,
                ..
            })
        ));
        assert_eq!(registry.revision(), revision);
        assert_eq!(registry.list(&PublicTableFilter::default()), before);
    }

    #[test]
    fn two_running_tables_reject_cross_table_commands_without_revision_or_chip_leak() {
        let mut registry = TableRegistry::new(4).unwrap();
        let alpha = registry.create(config("Alpha", 2), Some(11)).unwrap();
        let bravo = registry.create(config("Bravo", 2), Some(22)).unwrap();
        for (table, prefix) in [(alpha.table_id, "a"), (bravo.table_id, "b")] {
            registry
                .join(
                    guest(&format!("{prefix}-0")),
                    table,
                    Some(SeatId::new(0).unwrap()),
                )
                .unwrap();
            registry
                .join(
                    guest(&format!("{prefix}-1")),
                    table,
                    Some(SeatId::new(1).unwrap()),
                )
                .unwrap();
        }
        let alpha_route = registry.route_for_session(&guest("a-0")).unwrap();
        let bravo_route = registry.route_for_session(&guest("b-0")).unwrap();
        let alpha_before = alpha_route.handle.snapshot(guest("a-0")).unwrap();
        let bravo_before = bravo_route.handle.snapshot(guest("b-0")).unwrap();
        let hostile = CommandEnvelope::act_for_hand(
            "cross-table",
            bravo.table_id,
            bravo_route.hand_id,
            bravo_before.revision,
            alpha_route.seat,
            Action::Fold,
        );
        let error = alpha_route
            .handle
            .submit(guest("a-0"), hostile)
            .unwrap_err();
        assert_eq!(error.code.name(), "wrong_table");
        assert_eq!(
            alpha_route.handle.snapshot(guest("a-0")).unwrap(),
            alpha_before
        );
        assert_eq!(
            bravo_route.handle.snapshot(guest("b-0")).unwrap(),
            bravo_before
        );
        let alpha_chips: u32 = alpha_before
            .snapshot
            .seats
            .iter()
            .map(|seat| seat.stack + seat.hand_contribution)
            .sum();
        let bravo_chips: u32 = bravo_before
            .snapshot
            .seats
            .iter()
            .map(|seat| seat.stack + seat.hand_contribution)
            .sum();
        assert_eq!(alpha_chips, 200);
        assert_eq!(bravo_chips, 200);
    }

    #[test]
    fn only_empty_inactive_tables_can_be_removed() {
        let mut registry = TableRegistry::new(4).unwrap();
        let empty = registry.create(config("Empty", 2), Some(1)).unwrap();
        let occupied = registry.create(config("Occupied", 2), Some(2)).unwrap();
        registry
            .join(guest("guest-a"), occupied.table_id, None)
            .unwrap();
        assert!(matches!(
            registry.close_and_remove(occupied.table_id),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::TableNotRemovable,
                ..
            })
        ));
        registry.close_and_remove(empty.table_id).unwrap();
        assert!(matches!(
            registry.inspect(empty.table_id),
            Err(TableRegistryError {
                code: TableRegistryErrorCode::UnknownTable,
                ..
            })
        ));
        assert_eq!(registry.inspect(occupied.table_id).unwrap().occupied, 1);
    }
}
