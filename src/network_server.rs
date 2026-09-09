//! One-table loopback TCP server for the local network-alpha candidate.

use crate::{admission::Admission, game_stream::GameStream};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::authorized_table::{
    AuthorizedTableHandle, AuthorizedTableRuntime, GuestSessionId, SessionRole,
};
use crate::credentials::{BearerToken, ReconnectGrant};
use crate::game::lifecycle::TableLifecycle;
use crate::game::multiway::MultiwayPhase;
use crate::game::seat::{PlayerId, SeatId, TableSize};
use crate::lobby::{
    LobbyEnvelope, LobbyError, LobbyRequest, LobbyResponse, LobbyResult, PublicTableFilter,
    LOBBY_PROTOCOL_VERSION, MAX_LOBBY_REQUEST_ID_BYTES,
};
use crate::network_transport::{
    read_available, write_message, ClientWireMessage, FrameDecoder, PublicWireError, ReadStatus,
    ServerWireMessage, TransportError, WIRE_VERSION,
};
use crate::protocol::{HandId, ProtocolAuthority, TableId};
use crate::table_registry::{TableRegistry, TableRegistryError, DEFAULT_TABLE_REGISTRY_CAPACITY};

pub const DEFAULT_NETWORK_TABLE_ID: TableId = TableId(1);
pub const DEFAULT_NETWORK_HAND_ID: HandId = HandId(1);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const LOOP_DELAY: Duration = Duration::from_millis(2);

#[derive(Debug, Clone)]
pub struct NetworkServerConfig {
    pub bind: SocketAddr,
    pub seats: TableSize,
    pub starting_stack: u32,
    pub deterministic_seed: Option<u64>,
    pub exit_after_hand: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkServerSummary {
    pub listen_addr: SocketAddr,
    pub revision: u64,
    pub stream_sequence: u64,
    pub connections_accepted: u64,
    pub disconnects: u64,
}

#[derive(Debug, Clone)]
pub struct MultiTableNetworkServerConfig {
    pub tls: Option<Arc<rustls::ServerConfig>>,
    pub bind: SocketAddr,
    pub max_tables: usize,
    pub deterministic_seed_base: Option<u64>,
    pub exit_after_hands: usize,
    pub checkpoint_path: Option<PathBuf>,
    pub history_path: Option<PathBuf>,
    pub table_idle_ttl: Duration,
    pub shutdown_requested: Arc<AtomicBool>,
    pub reconnect_credential_ttl: Duration,
}

impl Default for MultiTableNetworkServerConfig {
    fn default() -> Self {
        Self {
            tls: None,
            bind: "127.0.0.1:0".parse().expect("loopback default is valid"),
            max_tables: DEFAULT_TABLE_REGISTRY_CAPACITY,
            deterministic_seed_base: None,
            exit_after_hands: 0,
            checkpoint_path: None,
            history_path: None,
            table_idle_ttl: Duration::from_secs(15 * 60),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
            reconnect_credential_ttl: Duration::from_secs(5 * 60),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiTableNetworkServerSummary {
    pub listen_addr: SocketAddr,
    pub lobby_revision: u64,
    pub tables: usize,
    pub completed_hands: usize,
    pub connections_accepted: u64,
    pub expired_tables: usize,
    pub drain_millis: u64,
    pub drain_checkpoint_published: bool,
    pub stop_reason: MultiTableStopReason,
    pub history_recovery: HistoryRecoveryStatus,
    pub safe_histories: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiTableStopReason {
    HandTarget,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryRecoveryStatus {
    NotConfigured,
    Missing,
    Loaded,
    CorruptIgnored,
}

#[derive(Debug)]
pub enum NetworkServerError {
    NonLoopbackAddress(SocketAddr),
    InvalidStartingStack,
    Io(io::Error),
    Authority(String),
}

impl Display for NetworkServerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonLoopbackAddress(address) => {
                write!(
                    formatter,
                    "network server binds loopback only, received {address}"
                )
            }
            Self::InvalidStartingStack => write!(formatter, "starting stack must be positive"),
            Self::Io(error) => write!(formatter, "network server I/O failed: {error}"),
            Self::Authority(message) => write!(formatter, "network authority failed: {message}"),
        }
    }
}

impl Error for NetworkServerError {}

impl From<io::Error> for NetworkServerError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct NetworkServer {
    listener: TcpListener,
    listen_addr: SocketAddr,
    runtime: AuthorizedTableRuntime,
    handle: AuthorizedTableHandle,
    active_sessions: Arc<Mutex<BTreeSet<String>>>,
    hand_complete: Arc<AtomicBool>,
    final_revision: Arc<AtomicU64>,
    exit_after_hand: bool,
    max_connections: usize,
}

pub struct MultiTableNetworkServer {
    tls: Option<Arc<rustls::ServerConfig>>,
    listener: TcpListener,
    listen_addr: SocketAddr,
    registry: Arc<Mutex<TableRegistry>>,
    active_sessions: Arc<Mutex<BTreeSet<GuestSessionId>>>,
    completed_hands: Arc<AtomicUsize>,
    next_seed_offset: Arc<AtomicU64>,
    deterministic_seed_base: Option<u64>,
    exit_after_hands: usize,
    max_connections: usize,
    checkpoint_path: Option<PathBuf>,
    history_path: Option<PathBuf>,
    history_recovery: HistoryRecoveryStatus,
    table_idle_ttl: Duration,
    shutdown_requested: Arc<AtomicBool>,
}

impl MultiTableNetworkServer {
    pub fn start(config: MultiTableNetworkServerConfig) -> Result<Self, NetworkServerError> {
        if !config.bind.ip().is_loopback() && config.tls.is_none() {
            return Err(NetworkServerError::NonLoopbackAddress(config.bind));
        }
        let mut registry = match config.checkpoint_path.as_deref() {
            Some(path) if path.exists() => TableRegistry::load_checkpoint(path),
            _ => TableRegistry::new(config.max_tables),
        }
        .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        let history_recovery = match config.history_path.as_deref() {
            None => HistoryRecoveryStatus::NotConfigured,
            Some(path) if !path.exists() => HistoryRecoveryStatus::Missing,
            Some(path) => {
                match registry.load_safe_histories(path) {
                    Ok(()) => HistoryRecoveryStatus::Loaded,
                    Err(error) => {
                        eprintln!("safe-history recovery failed closed code=invalid_history error={error}");
                        HistoryRecoveryStatus::CorruptIgnored
                    }
                }
            }
        };
        registry
            .set_reconnect_ttl(config.reconnect_credential_ttl)
            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        let max_connections = registry.max_tables().saturating_mul(11).saturating_add(8);
        let listener = TcpListener::bind(config.bind)?;
        listener.set_nonblocking(true)?;
        let listen_addr = listener.local_addr()?;
        Ok(Self {
            tls: config.tls,
            listener,
            listen_addr,
            registry: Arc::new(Mutex::new(registry)),
            active_sessions: Arc::new(Mutex::new(BTreeSet::new())),
            completed_hands: Arc::new(AtomicUsize::new(0)),
            next_seed_offset: Arc::new(AtomicU64::new(0)),
            deterministic_seed_base: config.deterministic_seed_base,
            exit_after_hands: config.exit_after_hands,
            max_connections,
            checkpoint_path: config.checkpoint_path,
            history_path: config.history_path,
            history_recovery,
            table_idle_ttl: config.table_idle_ttl,
            shutdown_requested: config.shutdown_requested,
        })
    }

    pub const fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    pub fn run(self) -> Result<MultiTableNetworkServerSummary, NetworkServerError> {
        let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
        let mut connections_accepted = 0u64;
        let mut expired_tables = 0usize;
        let mut last_expiry_sweep = Instant::now();
        let mut last_showdown_sweep = Instant::now();
        let mut admission = Admission::new(30, Duration::from_secs(10));
        let request_budget = Arc::new(Mutex::new(Admission::new(60, Duration::from_secs(10))));
        let stop_reason;
        loop {
            if last_showdown_sweep.elapsed() >= Duration::from_millis(20)
                || self.shutdown_requested.load(Ordering::Acquire)
            {
                let mut registry = self
                    .registry
                    .lock()
                    .map_err(|_| NetworkServerError::Authority("table registry poisoned".into()))?;
                let count = registry
                    .finalize_ready_hands()
                    .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
                if count > 0 {
                    if let Some(path) = &self.history_path {
                        registry
                            .save_safe_histories(path)
                            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
                    }
                    if let Some(path) = &self.checkpoint_path {
                        registry
                            .save_checkpoint(path)
                            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
                    }
                    self.completed_hands.fetch_add(count, Ordering::AcqRel);
                }
                last_showdown_sweep = Instant::now();
            }
            if self.shutdown_requested.load(Ordering::Acquire) {
                stop_reason = MultiTableStopReason::Interrupt;
                break;
            }
            if self.exit_after_hands > 0
                && self.completed_hands.load(Ordering::Acquire) >= self.exit_after_hands
            {
                stop_reason = MultiTableStopReason::HandTarget;
                break;
            }
            if last_expiry_sweep.elapsed() >= Duration::from_secs(1) {
                if let Ok(mut registry) = self.registry.lock() {
                    let _ = registry.advance_tournament_breaks();
                    expired_tables = expired_tables
                        .saturating_add(registry.expire_inactive(self.table_idle_ttl).expired);
                }
                last_expiry_sweep = Instant::now();
            }
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    workers.retain(|worker| !worker.is_finished());
                    if workers.len() >= self.max_connections {
                        continue;
                    }
                    if self.tls.is_some() && !admission.allow(peer.ip(), Instant::now()) {
                        continue;
                    }
                    if !peer.ip().is_loopback() && self.tls.is_none() {
                        continue;
                    }
                    let active_count = self
                        .active_sessions
                        .lock()
                        .map(|sessions| sessions.len())
                        .unwrap_or(self.max_connections);
                    if active_count >= self.max_connections {
                        if self.tls.is_some() {
                            continue;
                        }
                        reject_connection(
                            stream,
                            "connection_limit",
                            "multi-table connection limit reached",
                        );
                        continue;
                    }
                    connections_accepted = connections_accepted.saturating_add(1);
                    let registry = Arc::clone(&self.registry);
                    let active_sessions = Arc::clone(&self.active_sessions);
                    let completed_hands = Arc::clone(&self.completed_hands);
                    let seed_offset = Arc::clone(&self.next_seed_offset);
                    let seed_base = self.deterministic_seed_base;
                    let checkpoint_path = self.checkpoint_path.clone();
                    let history_path = self.history_path.clone();
                    let tls = self.tls.clone();
                    let budget = Arc::clone(&request_budget);
                    workers.push(thread::spawn(move || {
                        if let Err(error) = handle_multi_table_connection(
                            stream,
                            tls,
                            peer.ip(),
                            budget,
                            registry,
                            active_sessions,
                            completed_hands,
                            seed_base,
                            seed_offset,
                            checkpoint_path,
                            history_path,
                        ) {
                            if !expected_peer_close(&error) {
                                eprintln!("multi-table connection peer={peer} closed with {error}");
                            }
                        }
                    }));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(LOOP_DELAY);
                }
                Err(error) => return Err(NetworkServerError::Io(error)),
            }
        }

        // Terminal updates are already buffered per subscriber when the hand
        // count advances. Give every accepted connection a bounded chance to
        // forward that final projection and consume the client's Close before
        // the process summary causes OS-level socket teardown.
        let drain_started = Instant::now();
        while workers.iter().any(|worker| !worker.is_finished())
            && drain_started.elapsed() < Duration::from_secs(2)
        {
            thread::sleep(Duration::from_millis(5));
        }
        for worker in workers {
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
        let mut drain_checkpoint_published = false;
        let (lobby_revision, tables, safe_histories) = self
            .registry
            .lock()
            .map_err(|_| NetworkServerError::Authority("table registry poisoned".to_string()))
            .and_then(|mut registry| {
                if stop_reason == MultiTableStopReason::Interrupt {
                    if let Some(path) = self.checkpoint_path.as_deref() {
                        registry
                            .save_checkpoint(path)
                            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
                        drain_checkpoint_published = true;
                    }
                    if let Some(path) = self.history_path.as_deref() {
                        registry
                            .save_safe_histories(path)
                            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
                    }
                }
                Ok((
                    registry.revision(),
                    registry.len(),
                    registry.safe_history_count(),
                ))
            })?;
        let completed_hands = self.completed_hands.load(Ordering::Acquire);
        Ok(MultiTableNetworkServerSummary {
            listen_addr: self.listen_addr,
            lobby_revision,
            tables,
            completed_hands,
            connections_accepted,
            expired_tables,
            drain_millis: u64::try_from(drain_started.elapsed().as_millis()).unwrap_or(u64::MAX),
            drain_checkpoint_published,
            stop_reason,
            history_recovery: self.history_recovery,
            safe_histories,
        })
    }
}

fn expected_peer_close(error: &TransportError) -> bool {
    matches!(
        error,
        TransportError::Io(io_error)
            if matches!(
                io_error.kind(),
                io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::UnexpectedEof
            )
    )
}

impl NetworkServer {
    pub fn start(config: NetworkServerConfig) -> Result<Self, NetworkServerError> {
        if !config.bind.ip().is_loopback() {
            return Err(NetworkServerError::NonLoopbackAddress(config.bind));
        }
        if config.starting_stack == 0 {
            return Err(NetworkServerError::InvalidStartingStack);
        }
        let mut lifecycle = TableLifecycle::new(config.seats);
        for seat in config.seats.seats() {
            lifecycle
                .join(
                    PlayerId::new(u64::from(seat.as_u8()) + 1),
                    seat,
                    config.starting_stack,
                )
                .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        }
        let start = lifecycle
            .begin_hand()
            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        let hand = start
            .into_hand(config.seats, config.deterministic_seed)
            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        let authority =
            ProtocolAuthority::new_paced(DEFAULT_NETWORK_TABLE_ID, DEFAULT_NETWORK_HAND_ID, hand);
        let runtime = AuthorizedTableRuntime::spawn(authority)
            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        let handle = runtime.handle();
        for seat in config.seats.seats() {
            handle
                .bind(
                    GuestSessionId::new(session_id_for_seat(seat))
                        .expect("generated session IDs are valid"),
                    DEFAULT_NETWORK_TABLE_ID,
                    DEFAULT_NETWORK_HAND_ID,
                    SessionRole::Player { seat },
                )
                .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        }
        let listener = TcpListener::bind(config.bind)?;
        listener.set_nonblocking(true)?;
        let listen_addr = listener.local_addr()?;
        Ok(Self {
            listener,
            listen_addr,
            runtime,
            handle,
            active_sessions: Arc::new(Mutex::new(BTreeSet::new())),
            hand_complete: Arc::new(AtomicBool::new(false)),
            final_revision: Arc::new(AtomicU64::new(0)),
            exit_after_hand: config.exit_after_hand,
            max_connections: usize::from(config.seats.get()) + 2,
        })
    }

    pub const fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    pub fn run(self) -> Result<NetworkServerSummary, NetworkServerError> {
        let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
        let mut connections_accepted = 0u64;
        loop {
            if self.exit_after_hand && !self.hand_complete.load(Ordering::SeqCst) {
                let (snapshot, _) = self
                    .handle
                    .safe_history_material()
                    .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
                if matches!(
                    snapshot.snapshot.phase,
                    MultiwayPhase::Showdown | MultiwayPhase::HandComplete
                ) {
                    self.final_revision
                        .store(snapshot.revision, Ordering::SeqCst);
                    self.hand_complete.store(true, Ordering::SeqCst);
                }
            }
            if self.exit_after_hand && self.hand_complete.load(Ordering::SeqCst) {
                break;
            }
            match self.listener.accept() {
                Ok((stream, peer)) => {
                    if !peer.ip().is_loopback() {
                        continue;
                    }
                    let active_count = self
                        .active_sessions
                        .lock()
                        .map(|sessions| sessions.len())
                        .unwrap_or(self.max_connections);
                    if active_count >= self.max_connections {
                        reject_connection(
                            stream,
                            "connection_limit",
                            "table connection limit reached",
                        );
                        continue;
                    }
                    connections_accepted += 1;
                    let handle = self.handle.clone();
                    let active_sessions = Arc::clone(&self.active_sessions);
                    let hand_complete = Arc::clone(&self.hand_complete);
                    let final_revision = Arc::clone(&self.final_revision);
                    workers.push(thread::spawn(move || {
                        if let Err(error) = handle_connection(
                            stream,
                            handle,
                            active_sessions,
                            hand_complete,
                            final_revision,
                        ) {
                            eprintln!("connection peer={peer} closed with {error}");
                        }
                    }));
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(LOOP_DELAY);
                }
                Err(error) => return Err(NetworkServerError::Io(error)),
            }
        }

        thread::sleep(Duration::from_millis(250));
        for worker in workers {
            if worker.is_finished() {
                let _ = worker.join();
            }
        }
        let metrics = self
            .handle
            .metrics()
            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        let revision = self.final_revision.load(Ordering::SeqCst);
        let stream_sequence = metrics.stream_sequence;
        let disconnects = metrics.disconnects;
        self.runtime
            .shutdown()
            .map_err(|error| NetworkServerError::Authority(error.to_string()))?;
        Ok(NetworkServerSummary {
            listen_addr: self.listen_addr,
            revision,
            stream_sequence,
            connections_accepted,
            disconnects,
        })
    }
}

pub fn session_id_for_seat(seat: SeatId) -> String {
    format!("player-s{}", seat.as_u8())
}

#[allow(clippy::too_many_arguments)]
fn handle_multi_table_connection(
    stream: TcpStream,
    tls: Option<Arc<rustls::ServerConfig>>,
    peer: std::net::IpAddr,
    request_budget: Arc<Mutex<Admission>>,
    registry: Arc<Mutex<TableRegistry>>,
    active_sessions: Arc<Mutex<BTreeSet<GuestSessionId>>>,
    completed_hands: Arc<AtomicUsize>,
    deterministic_seed_base: Option<u64>,
    next_seed_offset: Arc<AtomicU64>,
    checkpoint_path: Option<PathBuf>,
    history_path: Option<PathBuf>,
) -> Result<(), TransportError> {
    let secure = tls.is_some();
    let mut stream = GameStream::accept(stream, tls)?;
    stream.set_nonblocking(true)?;
    stream.set_nodelay(true)?;
    let mut decoder = FrameDecoder::default();
    let first = wait_for_message(&mut stream, &mut decoder)?;
    let ClientWireMessage::Connect {
        version,
        label: _,
        reconnect,
    } = first
    else {
        send_error(
            &mut stream,
            "connect_required",
            "first message must be connect",
        )?;
        return Ok(());
    };
    if version != WIRE_VERSION {
        send_error(
            &mut stream,
            "unsupported_wire_version",
            "server supports only wire version 3",
        )?;
        return Ok(());
    }
    let guest = match reconnect.as_ref() {
        Some(token) => match lock_registry(&registry)?.identify_reconnect(token) {
            Ok(principal) => principal,
            Err(error) => {
                send_error(&mut stream, "reconnect_rejected", error.message)?;
                return Ok(());
            }
        },
        None => GuestSessionId::random(),
    };
    {
        let mut sessions = active_sessions.lock().map_err(|_| {
            TransportError::Io(io::Error::other("active-session registry poisoned"))
        })?;
        if !sessions.insert(guest.clone()) {
            send_error(
                &mut stream,
                "duplicate_active_session",
                "guest session already has an active connection",
            )?;
            return Ok(());
        }
    }
    let context = MultiConnectionContext {
        request_budget: secure.then_some((&request_budget, peer)),
        registry: &registry,
        completed_hands: &completed_hands,
        deterministic_seed_base,
        next_seed_offset: &next_seed_offset,
        checkpoint_path: checkpoint_path.as_deref(),
        history_path: history_path.as_deref(),
    };
    let result =
        run_multi_table_connection(&mut stream, &mut decoder, guest.clone(), reconnect, context);
    let _ = stream.close_notify();
    if let Ok(mut sessions) = active_sessions.lock() {
        sessions.remove(&guest);
    }
    result
}

struct MultiConnectionContext<'a> {
    request_budget: Option<(&'a Mutex<Admission>, std::net::IpAddr)>,
    registry: &'a Arc<Mutex<TableRegistry>>,
    completed_hands: &'a Arc<AtomicUsize>,
    deterministic_seed_base: Option<u64>,
    next_seed_offset: &'a AtomicU64,
    checkpoint_path: Option<&'a std::path::Path>,
    history_path: Option<&'a std::path::Path>,
}

fn run_multi_table_connection(
    stream: &mut GameStream,
    decoder: &mut FrameDecoder,
    guest: GuestSessionId,
    reconnect_token: Option<BearerToken>,
    context: MultiConnectionContext<'_>,
) -> Result<(), TransportError> {
    let route_wait_started = Instant::now();
    loop {
        let existing_route = {
            let registry = lock_registry(context.registry)?;
            registry.route_for_session(&guest)
        };
        let Ok(route) = existing_route else {
            let break_pending = lock_registry(context.registry)?.tournament_break_pending(&guest);
            if break_pending {
                thread::sleep(Duration::from_millis(100));
                continue;
            }
            break;
        };
        let terminal_or_closing = route
            .handle
            .safe_history_material()
            .map(|(snapshot, _)| {
                matches!(
                    snapshot.snapshot.phase,
                    MultiwayPhase::Showdown | MultiwayPhase::HandComplete
                )
            })
            .unwrap_or(true);
        if !terminal_or_closing {
            let rotated_grant = match reconnect_token.as_ref() {
                Some(token) => {
                    match lock_registry(context.registry)?.authenticate_and_rotate_reconnect(token)
                    {
                        Ok((authenticated, grant)) if authenticated == guest => Some(grant),
                        Ok(_) => {
                            send_error(
                                stream,
                                "reconnect_rejected",
                                "reconnect credential principal changed during authentication",
                            )?;
                            return Ok(());
                        }
                        Err(error) => {
                            send_error(stream, "reconnect_rejected", error.message)?;
                            return Ok(());
                        }
                    }
                }
                None => None,
            };
            return run_multi_bound_connection(
                stream,
                decoder,
                context.registry,
                guest,
                route,
                rotated_grant,
                context.completed_hands,
                context.checkpoint_path,
                context.history_path,
            );
        }
        if route_wait_started.elapsed() >= CONNECT_TIMEOUT {
            send_error(
                stream,
                "route_transition_timeout",
                "successor hand route was not ready within five seconds",
            )?;
            return Ok(());
        }
        thread::sleep(LOOP_DELAY);
    }
    if reconnect_token.is_some() {
        send_error(
            stream,
            "reconnect_rejected",
            "reconnect credential no longer has a routed successor",
        )?;
        return Ok(());
    }
    if let Some(update) = lock_registry(context.registry)?.take_retired_update(&guest) {
        write_message(
            stream,
            &ServerWireMessage::Welcome {
                update,
                reconnect: None,
            },
        )?;
        return Ok(());
    }
    let _registration_guard = RegistrationDisconnectGuard {
        registry: context.registry,
        guest: guest.clone(),
    };
    let (lobby_revision, capacity, tables) = {
        let registry = lock_registry(context.registry)?;
        (
            registry.revision(),
            u8::try_from(registry.max_tables()).expect("registry capacity is at most 64"),
            registry.list(&PublicTableFilter::default()),
        )
    };
    write_message(
        stream,
        &ServerWireMessage::LobbyWelcome {
            version: LOBBY_PROTOCOL_VERSION,
            lobby_revision,
            capacity,
            tables,
        },
    )?;

    // Waiting is read-only and gets a per-connection budget. Players sharing an
    // IP must not spend each other's password/create/join admission allowance.
    let mut waiting_budget = Admission::new(30, Duration::from_secs(10));
    loop {
        if read_available(stream, decoder)? == ReadStatus::Closed {
            return Ok(());
        }
        while let Some(message) = decoder.decode_next::<ClientWireMessage>()? {
            match message {
                ClientWireMessage::Lobby { request } => {
                    if let Some((budget, peer)) = context.request_budget {
                        let allowed = if matches!(request.payload, LobbyRequest::JoinStatus) {
                            waiting_budget.allow(peer, Instant::now())
                        } else {
                            budget
                                .lock()
                                .map_err(|_| io::Error::other("admission lock poisoned"))?
                                .allow(peer, Instant::now())
                        };
                        if !allowed {
                            send_error(
                                stream,
                                "rate_limited",
                                "Too many requests; please wait and retry",
                            )?;
                            return Ok(());
                        }
                    }
                    let request_id = request.request_id.clone();
                    match apply_lobby_request(
                        context.registry,
                        &guest,
                        request,
                        context.deterministic_seed_base,
                        context.next_seed_offset,
                    ) {
                        Ok((response, route)) => {
                            write_message(stream, &ServerWireMessage::Lobby { response })?;
                            if let Some(route) = route {
                                return run_multi_bound_connection(
                                    stream,
                                    decoder,
                                    context.registry,
                                    guest,
                                    route,
                                    None,
                                    context.completed_hands,
                                    context.checkpoint_path,
                                    context.history_path,
                                );
                            }
                        }
                        Err(error) => {
                            let revision = lock_registry(context.registry)?.revision();
                            write_message(
                                stream,
                                &ServerWireMessage::LobbyError {
                                    error: LobbyError {
                                        version: LOBBY_PROTOCOL_VERSION,
                                        request_id: Some(request_id),
                                        lobby_revision: revision,
                                        code: error.code.name().to_string(),
                                        message: error.message,
                                    },
                                },
                            )?;
                        }
                    }
                }
                ClientWireMessage::Connect { .. } => {
                    send_error(stream, "already_connected", "connect is valid only once")?;
                }
                ClientWireMessage::Command { .. } | ClientWireMessage::SnapshotRequest => {
                    send_error(
                        stream,
                        "table_join_required",
                        "join a ready table before requesting table authority",
                    )?;
                }
                ClientWireMessage::Close => {
                    write_message(stream, &ServerWireMessage::Goodbye)?;
                    return Ok(());
                }
            }
        }
        thread::sleep(LOOP_DELAY);
    }
}

struct RegistrationDisconnectGuard<'a> {
    registry: &'a Arc<Mutex<TableRegistry>>,
    guest: GuestSessionId,
}
impl Drop for RegistrationDisconnectGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.registry.lock() {
            registry.cancel_pending_registration(&self.guest);
        }
    }
}

fn apply_lobby_request(
    registry: &Arc<Mutex<TableRegistry>>,
    guest: &GuestSessionId,
    request: LobbyEnvelope,
    deterministic_seed_base: Option<u64>,
    next_seed_offset: &AtomicU64,
) -> Result<(LobbyResponse, Option<crate::table_registry::TableRoute>), TableRegistryError> {
    if request.version != LOBBY_PROTOCOL_VERSION {
        return Err(TableRegistryError::public(
            crate::table_registry::TableRegistryErrorCode::UnsupportedVersion,
            "server supports only lobby protocol version 2",
        ));
    }
    if request.request_id.is_empty()
        || request.request_id.len() > MAX_LOBBY_REQUEST_ID_BYTES
        || !request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(TableRegistryError::public(
            crate::table_registry::TableRegistryErrorCode::InvalidRequestId,
            "lobby request ID is invalid",
        ));
    }
    let mut registry = registry.lock().map_err(|_| {
        TableRegistryError::public(
            crate::table_registry::TableRegistryErrorCode::AuthorityFailure,
            "table registry is unavailable",
        )
    })?;
    let result = match request.payload {
        LobbyRequest::Create { config } => {
            let seed = deterministic_seed_base
                .map(|base| base.saturating_add(next_seed_offset.fetch_add(1, Ordering::SeqCst)));
            LobbyResult::Table {
                table: registry.create(config, seed)?,
            }
        }
        LobbyRequest::CreateTournament { config } => {
            let seed = deterministic_seed_base
                .map(|base| base.saturating_add(next_seed_offset.fetch_add(1, Ordering::SeqCst)));
            LobbyResult::Table {
                table: registry.create_tournament(config, seed)?,
            }
        }
        LobbyRequest::List { filter } => LobbyResult::Tables {
            tables: registry.list(&filter),
        },
        LobbyRequest::Inspect {
            table_id,
            access_code,
        } => LobbyResult::Table {
            table: registry.inspect_with_access(table_id, access_code.as_deref())?,
        },
        LobbyRequest::Join {
            table_id,
            seat,
            access_code,
        } => admission_result(registry.join_or_wait_with_access(
            guest.clone(),
            table_id,
            seat,
            access_code.as_deref(),
        )?),
        LobbyRequest::JoinStatus => admission_result(registry.admission_status(guest)?),
        LobbyRequest::CancelWait { table_id } => {
            registry.cancel_wait(guest, table_id)?;
            LobbyResult::WaitCancelled { table_id }
        }
        LobbyRequest::Health => LobbyResult::Health {
            health: registry.health()?,
        },
    };
    let route = match &result {
        LobbyResult::Joined { ready: true, .. } => Some(registry.route_for_session(guest)?),
        _ => None,
    };
    Ok((
        LobbyResponse {
            version: LOBBY_PROTOCOL_VERSION,
            request_id: request.request_id,
            lobby_revision: registry.revision(),
            result,
        },
        route,
    ))
}

fn admission_result(outcome: crate::table_registry::AdmissionOutcome) -> LobbyResult {
    match outcome {
        crate::table_registry::AdmissionOutcome::Joined(joined) => LobbyResult::Joined {
            table: joined.table,
            seat: joined.seat,
            hand_id: joined.hand_id,
            ready: joined.ready,
        },
        crate::table_registry::AdmissionOutcome::Waiting(waiting) => LobbyResult::Waiting {
            table: waiting.table,
            position: waiting.position,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn run_multi_bound_connection(
    stream: &mut GameStream,
    decoder: &mut FrameDecoder,
    registry: &Arc<Mutex<TableRegistry>>,
    guest: GuestSessionId,
    route: crate::table_registry::TableRoute,
    reconnect_grant: Option<ReconnectGrant>,
    _completed_hands: &Arc<AtomicUsize>,
    _checkpoint_path: Option<&std::path::Path>,
    _history_path: Option<&std::path::Path>,
) -> Result<(), TransportError> {
    if let Err(error) = route.handle.reconnect(guest.clone()) {
        send_error(stream, error.code.name(), error.message)?;
        return Ok(());
    }
    let _disconnect_guard = BoundDisconnectGuard {
        handle: route.handle.clone(),
        guest: guest.clone(),
    };
    let subscription = match route.handle.subscribe(guest.clone()) {
        Ok(subscription) => subscription,
        Err(error) => {
            send_error(stream, error.code.name(), error.message)?;
            return Ok(());
        }
    };
    let initial = subscription.recv().map_err(|error| {
        TransportError::Io(io::Error::new(io::ErrorKind::BrokenPipe, error.to_string()))
    })?;
    let reconnect = match reconnect_grant {
        Some(grant) => grant,
        None => lock_registry(registry)?
            .issue_reconnect_credential(&guest)
            .map_err(|error| TransportError::Io(io::Error::other(error.to_string())))?,
    };
    write_message(
        stream,
        &ServerWireMessage::Welcome {
            update: initial,
            reconnect: Some(reconnect),
        },
    )?;

    loop {
        while let Ok(update) = subscription.try_recv() {
            write_message(stream, &ServerWireMessage::Update { update })?;
        }
        if read_available(stream, decoder)? == ReadStatus::Closed {
            return Ok(());
        }
        while let Some(message) = decoder.decode_next::<ClientWireMessage>()? {
            match message {
                ClientWireMessage::Command { command } => {
                    if command.table_id != route.table_id {
                        send_error(
                            stream,
                            "wrong_table",
                            "session is routed to a different table",
                        )?;
                        continue;
                    }
                    match route.handle.submit(guest.clone(), command) {
                        Ok(response) => {
                            write_message(stream, &ServerWireMessage::Response { response })?;
                        }
                        Err(error) => {
                            eprintln!(
                                "table={} command rejected code={}",
                                route.table_id.0,
                                error.code.name()
                            );
                            send_error(stream, error.code.name(), error.message)?;
                        }
                    }
                }
                ClientWireMessage::SnapshotRequest => match route.handle.snapshot(guest.clone()) {
                    Ok(snapshot) => {
                        let metrics = route.handle.metrics().map_err(|error| {
                            TransportError::Io(io::Error::other(error.to_string()))
                        })?;
                        let update = crate::authorized_table::SubscriptionUpdate {
                            stream_sequence: metrics.stream_sequence,
                            reason: crate::authorized_table::SubscriptionReason::Initial,
                            event: None,
                            snapshot,
                            deadline: None,
                        };
                        write_message(stream, &ServerWireMessage::Update { update })?;
                    }
                    Err(error) => send_error(stream, error.code.name(), error.message)?,
                },
                ClientWireMessage::Lobby { .. } => send_error(
                    stream,
                    "already_joined",
                    "lobby commands are unavailable after joining a table",
                )?,
                ClientWireMessage::Connect { .. } => {
                    send_error(stream, "already_connected", "connect is valid only once")?;
                }
                ClientWireMessage::Close => {
                    write_message(stream, &ServerWireMessage::Goodbye)?;
                    return Ok(());
                }
            }
        }
        thread::sleep(LOOP_DELAY);
    }
}

struct BoundDisconnectGuard {
    handle: AuthorizedTableHandle,
    guest: GuestSessionId,
}

impl Drop for BoundDisconnectGuard {
    fn drop(&mut self) {
        let _ = self.handle.disconnect(self.guest.clone());
    }
}

fn lock_registry(
    registry: &Arc<Mutex<TableRegistry>>,
) -> Result<std::sync::MutexGuard<'_, TableRegistry>, TransportError> {
    registry
        .lock()
        .map_err(|_| TransportError::Io(io::Error::other("table registry poisoned")))
}

fn handle_connection(
    stream: TcpStream,
    handle: AuthorizedTableHandle,
    active_sessions: Arc<Mutex<BTreeSet<String>>>,
    hand_complete: Arc<AtomicBool>,
    final_revision: Arc<AtomicU64>,
) -> Result<(), TransportError> {
    let mut stream = GameStream::Plain(stream);
    stream.set_nonblocking(true)?;
    stream.set_nodelay(true)?;
    let mut decoder = FrameDecoder::default();
    let first = wait_for_message(&mut stream, &mut decoder)?;
    let ClientWireMessage::Connect {
        version,
        label: session,
        reconnect,
    } = first
    else {
        send_error(
            &mut stream,
            "connect_required",
            "first message must be connect",
        )?;
        return Ok(());
    };
    if version != WIRE_VERSION {
        send_error(
            &mut stream,
            "unsupported_wire_version",
            "server supports only wire version 3",
        )?;
        return Ok(());
    }
    if reconnect.is_some() {
        send_error(
            &mut stream,
            "reconnect_not_supported",
            "legacy fixed-table mode does not accept reconnect credentials",
        )?;
        return Ok(());
    }
    let guest = match GuestSessionId::new(session.clone()) {
        Ok(guest) => guest,
        Err(error) => {
            send_error(&mut stream, error.code.name(), error.message)?;
            return Ok(());
        }
    };
    {
        let mut sessions = active_sessions.lock().map_err(|_| {
            TransportError::Io(io::Error::other("active-session registry poisoned"))
        })?;
        if !sessions.insert(session.clone()) {
            send_error(
                &mut stream,
                "duplicate_active_session",
                "guest session already has an active connection",
            )?;
            return Ok(());
        }
    }

    let result = run_bound_connection(
        &mut stream,
        &mut decoder,
        &handle,
        guest.clone(),
        &hand_complete,
        &final_revision,
    );
    let _ = handle.disconnect(guest);
    if let Ok(mut sessions) = active_sessions.lock() {
        sessions.remove(&session);
    }
    result
}

fn run_bound_connection(
    stream: &mut GameStream,
    decoder: &mut FrameDecoder,
    handle: &AuthorizedTableHandle,
    guest: GuestSessionId,
    hand_complete: &AtomicBool,
    final_revision: &AtomicU64,
) -> Result<(), TransportError> {
    if let Err(error) = handle.reconnect(guest.clone()) {
        send_error(stream, error.code.name(), error.message)?;
        return Ok(());
    }
    let subscription = match handle.subscribe(guest.clone()) {
        Ok(subscription) => subscription,
        Err(error) => {
            send_error(stream, error.code.name(), error.message)?;
            return Ok(());
        }
    };
    let initial = subscription.recv().map_err(|error| {
        TransportError::Io(io::Error::new(io::ErrorKind::BrokenPipe, error.to_string()))
    })?;
    write_message(
        stream,
        &ServerWireMessage::Welcome {
            update: initial,
            reconnect: None,
        },
    )?;

    loop {
        while let Ok(update) = subscription.try_recv() {
            write_message(stream, &ServerWireMessage::Update { update })?;
        }
        if read_available(stream, decoder)? == ReadStatus::Closed {
            return Ok(());
        }
        while let Some(message) = decoder.decode_next::<ClientWireMessage>()? {
            match message {
                ClientWireMessage::Connect { .. } => {
                    send_error(stream, "already_connected", "connect is valid only once")?;
                }
                ClientWireMessage::Lobby { .. } => {
                    send_error(
                        stream,
                        "lobby_unavailable",
                        "server is in single-table mode",
                    )?;
                }
                ClientWireMessage::Command { command } => {
                    match handle.submit(guest.clone(), command) {
                        Ok(response) => {
                            let terminal = matches!(
                                response.snapshot.snapshot.phase,
                                MultiwayPhase::Showdown | MultiwayPhase::HandComplete
                            );
                            let response_revision = response.snapshot.revision;
                            write_message(stream, &ServerWireMessage::Response { response })?;
                            if terminal {
                                final_revision.store(response_revision, Ordering::SeqCst);
                                hand_complete.store(true, Ordering::SeqCst);
                            }
                        }
                        Err(error) => {
                            eprintln!("command rejected code={}", error.code.name());
                            send_error(stream, error.code.name(), error.message)?;
                        }
                    }
                }
                ClientWireMessage::SnapshotRequest => match handle.snapshot(guest.clone()) {
                    Ok(snapshot) => {
                        let metrics = handle.metrics().map_err(|error| {
                            TransportError::Io(io::Error::other(error.to_string()))
                        })?;
                        let update = crate::authorized_table::SubscriptionUpdate {
                            stream_sequence: metrics.stream_sequence,
                            reason: crate::authorized_table::SubscriptionReason::Initial,
                            event: None,
                            snapshot,
                            deadline: None,
                        };
                        write_message(stream, &ServerWireMessage::Update { update })?;
                    }
                    Err(error) => send_error(stream, error.code.name(), error.message)?,
                },
                ClientWireMessage::Close => {
                    write_message(stream, &ServerWireMessage::Goodbye)?;
                    return Ok(());
                }
            }
        }
        thread::sleep(LOOP_DELAY);
    }
}

fn wait_for_message(
    stream: &mut GameStream,
    decoder: &mut FrameDecoder,
) -> Result<ClientWireMessage, TransportError> {
    let started = Instant::now();
    loop {
        if read_available(stream, decoder)? == ReadStatus::Closed {
            return Err(TransportError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "peer closed before connect",
            )));
        }
        if let Some(message) = decoder.decode_next()? {
            return Ok(message);
        }
        if started.elapsed() >= CONNECT_TIMEOUT {
            return Err(TransportError::Io(io::Error::new(
                io::ErrorKind::TimedOut,
                "peer did not send connect in time",
            )));
        }
        thread::sleep(LOOP_DELAY);
    }
}

fn send_error(
    stream: &mut GameStream,
    code: impl Into<String>,
    message: impl Into<String>,
) -> Result<(), TransportError> {
    write_message(
        stream,
        &ServerWireMessage::Error {
            error: PublicWireError::new(code, message),
        },
    )
}

fn reject_connection(mut stream: TcpStream, code: &str, message: &str) {
    let _ = stream.set_nonblocking(false);
    let _ = write_message(
        &mut stream,
        &ServerWireMessage::Error {
            error: PublicWireError::new(code, message),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lobby::{LobbyRequest, PublicTableConfig};

    #[test]
    fn server_rejects_non_loopback_and_zero_stack() {
        let seats = TableSize::new(2).unwrap();
        let remote = NetworkServer::start(NetworkServerConfig {
            bind: "0.0.0.0:0".parse().unwrap(),
            seats,
            starting_stack: 100,
            deterministic_seed: Some(1),
            exit_after_hand: true,
        });
        assert!(matches!(
            remote,
            Err(NetworkServerError::NonLoopbackAddress(_))
        ));

        let zero = NetworkServer::start(NetworkServerConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            seats,
            starting_stack: 0,
            deterministic_seed: Some(1),
            exit_after_hand: true,
        });
        assert!(matches!(
            zero,
            Err(NetworkServerError::InvalidStartingStack)
        ));
    }

    #[test]
    fn generated_sessions_are_stable_and_safe() {
        for index in 0..9 {
            let seat = SeatId::new(index).unwrap();
            let value = session_id_for_seat(seat);
            assert_eq!(value, format!("player-s{index}"));
            GuestSessionId::new(value).unwrap();
        }
    }

    #[test]
    fn multi_table_server_rejects_remote_bind_and_invalid_registry_capacity() {
        let remote = MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
            bind: "0.0.0.0:0".parse().unwrap(),
            ..MultiTableNetworkServerConfig::default()
        });
        assert!(matches!(
            remote,
            Err(NetworkServerError::NonLoopbackAddress(_))
        ));
        let invalid = MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
            max_tables: 0,
            ..MultiTableNetworkServerConfig::default()
        });
        assert!(matches!(invalid, Err(NetworkServerError::Authority(_))));
    }

    #[test]
    fn incompatible_and_invalid_lobby_envelopes_are_mutation_free() {
        let registry = Arc::new(Mutex::new(TableRegistry::new(2).unwrap()));
        let guest = GuestSessionId::new("guest-a").unwrap();
        let seed = AtomicU64::new(0);
        for request in [
            LobbyEnvelope {
                version: LOBBY_PROTOCOL_VERSION + 1,
                request_id: "future".to_string(),
                payload: LobbyRequest::List {
                    filter: PublicTableFilter::default(),
                },
            },
            LobbyEnvelope {
                version: LOBBY_PROTOCOL_VERSION,
                request_id: "bad id!".to_string(),
                payload: LobbyRequest::Create {
                    config: PublicTableConfig {
                        name: "Should Not Exist".to_string(),
                        seats: TableSize::new(2).unwrap(),
                        starting_stack: 100,
                        min_players: 2,
                        visibility: crate::lobby::TableVisibility::Public,
                        join_code: None,
                    },
                },
            },
        ] {
            let before = registry.lock().unwrap().revision();
            assert!(apply_lobby_request(&registry, &guest, request, Some(1), &seed).is_err());
            let registry = registry.lock().unwrap();
            assert_eq!(registry.revision(), before);
            assert!(registry.is_empty());
        }
    }
}
