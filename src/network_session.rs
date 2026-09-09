//! Stateful loopback TCP client used by the production network TUI.

use crate::game_stream::GameStream;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;
use std::net::SocketAddr;
use std::thread;
use std::time::{Duration, Instant};

use crate::authorized_table::SubscriptionUpdate;
use crate::credentials::{BearerToken, ReconnectGrant};
use crate::game::seat::SeatId;
use crate::lobby::{
    LobbyEnvelope, LobbyRequest, LobbyResponse, LobbyResult, PublicTableConfig, PublicTableFilter,
    PublicTableSummary, LOBBY_PROTOCOL_VERSION,
};
use crate::network_transport::{
    read_available, write_message, ClientWireMessage, FrameDecoder, ReadStatus, ServerWireMessage,
    TransportError, WIRE_VERSION,
};
use crate::protocol::CommandEnvelope;
use crate::protocol::TableId;
use crate::tournament::TournamentConfig;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_DELAY: Duration = Duration::from_millis(2);

#[derive(Debug)]
pub enum NetworkSessionError {
    NonLoopbackAddress(SocketAddr),
    Transport(TransportError),
    HandshakeTimedOut,
    ClosedDuringHandshake,
    Rejected { code: String, message: String },
    UnexpectedHandshakeMessage,
    UnexpectedLobbyMessage,
    JoinCancelled,
}

impl Display for NetworkSessionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonLoopbackAddress(address) => {
                write!(
                    formatter,
                    "Sprint 8 clients connect to loopback only, received {address}"
                )
            }
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::HandshakeTimedOut => write!(formatter, "server welcome timed out"),
            Self::ClosedDuringHandshake => write!(formatter, "server closed before welcome"),
            Self::Rejected { code, message } => {
                write!(formatter, "server rejected {code}: {message}")
            }
            Self::UnexpectedHandshakeMessage => {
                write!(
                    formatter,
                    "server sent an unexpected message before welcome"
                )
            }
            Self::JoinCancelled => write!(formatter, "registration cancelled"),
            Self::UnexpectedLobbyMessage => {
                write!(formatter, "server sent an unexpected lobby message")
            }
        }
    }
}

impl Error for NetworkSessionError {}

impl From<TransportError> for NetworkSessionError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

pub struct NetworkSession {
    stream: GameStream,
    decoder: FrameDecoder,
    closed: bool,
    reconnect: Option<ReconnectGrant>,
}

pub struct LobbySession {
    stream: GameStream,
    decoder: FrameDecoder,
    closed: bool,
    next_request: u64,
}

impl LobbySession {
    pub fn connect(
        address: SocketAddr,
        session: impl Into<String>,
    ) -> Result<(Self, u64, u8, Vec<PublicTableSummary>), NetworkSessionError> {
        let stream = GameStream::connect(address).map_err(TransportError::Io)?;
        stream.set_nonblocking(true).map_err(TransportError::Io)?;
        stream.set_nodelay(true).map_err(TransportError::Io)?;
        let mut lobby = Self {
            stream,
            decoder: FrameDecoder::default(),
            closed: false,
            next_request: 1,
        };
        lobby.send(&ClientWireMessage::Connect {
            version: WIRE_VERSION,
            label: session.into(),
            reconnect: None,
        })?;
        let started = Instant::now();
        loop {
            lobby.read_available()?;
            if let Some(message) = lobby.decoder.decode_next()? {
                match message {
                    ServerWireMessage::LobbyWelcome {
                        version,
                        lobby_revision,
                        capacity,
                        tables,
                    } if version == LOBBY_PROTOCOL_VERSION => {
                        return Ok((lobby, lobby_revision, capacity, tables));
                    }
                    ServerWireMessage::Error { error } => {
                        return Err(NetworkSessionError::Rejected {
                            code: error.code,
                            message: error.message,
                        });
                    }
                    _ => return Err(NetworkSessionError::UnexpectedHandshakeMessage),
                }
            }
            if lobby.closed {
                return Err(NetworkSessionError::ClosedDuringHandshake);
            }
            if started.elapsed() >= CONNECT_TIMEOUT {
                return Err(NetworkSessionError::HandshakeTimedOut);
            }
            thread::sleep(POLL_DELAY);
        }
    }

    pub fn create_table(
        &mut self,
        config: PublicTableConfig,
    ) -> Result<PublicTableSummary, NetworkSessionError> {
        match self.request(LobbyRequest::Create { config })?.result {
            LobbyResult::Table { table } => Ok(table),
            _ => Err(NetworkSessionError::UnexpectedLobbyMessage),
        }
    }

    pub fn create_tournament(
        &mut self,
        config: TournamentConfig,
    ) -> Result<PublicTableSummary, NetworkSessionError> {
        match self
            .request(LobbyRequest::CreateTournament { config })?
            .result
        {
            LobbyResult::Table { table } => Ok(table),
            _ => Err(NetworkSessionError::UnexpectedLobbyMessage),
        }
    }

    pub fn list_tables(
        &mut self,
        filter: PublicTableFilter,
    ) -> Result<Vec<PublicTableSummary>, NetworkSessionError> {
        match self.request(LobbyRequest::List { filter })?.result {
            LobbyResult::Tables { tables } => Ok(tables),
            _ => Err(NetworkSessionError::UnexpectedLobbyMessage),
        }
    }

    pub fn health(&mut self) -> Result<crate::lobby::RegistryHealth, NetworkSessionError> {
        match self.request(LobbyRequest::Health)?.result {
            LobbyResult::Health { health } => Ok(health),
            _ => Err(NetworkSessionError::UnexpectedLobbyMessage),
        }
    }

    pub fn inspect_table(
        &mut self,
        table_id: TableId,
    ) -> Result<PublicTableSummary, NetworkSessionError> {
        match self
            .request(LobbyRequest::Inspect {
                table_id,
                access_code: None,
            })?
            .result
        {
            LobbyResult::Table { table } => Ok(table),
            _ => Err(NetworkSessionError::UnexpectedLobbyMessage),
        }
    }

    pub fn inspect_table_with_access(
        &mut self,
        table_id: TableId,
        access_code: String,
    ) -> Result<PublicTableSummary, NetworkSessionError> {
        match self
            .request(LobbyRequest::Inspect {
                table_id,
                access_code: Some(access_code),
            })?
            .result
        {
            LobbyResult::Table { table } => Ok(table),
            _ => Err(NetworkSessionError::UnexpectedLobbyMessage),
        }
    }

    pub fn join_and_wait(
        self,
        table_id: TableId,
        seat: Option<SeatId>,
    ) -> Result<(NetworkSession, SubscriptionUpdate, SeatId), NetworkSessionError> {
        self.join_and_wait_with_access(table_id, seat, None)
    }

    pub fn join_and_wait_with_access(
        self,
        table_id: TableId,
        seat: Option<SeatId>,
        access_code: Option<String>,
    ) -> Result<(NetworkSession, SubscriptionUpdate, SeatId), NetworkSessionError> {
        self.join_and_wait_while(table_id, seat, access_code, |_| true)
    }

    pub fn join_and_wait_while(
        mut self,
        table_id: TableId,
        seat: Option<SeatId>,
        access_code: Option<String>,
        mut keep_waiting: impl FnMut(&PublicTableSummary) -> bool,
    ) -> Result<(NetworkSession, SubscriptionUpdate, SeatId), NetworkSessionError> {
        let mut response = self.request(LobbyRequest::Join {
            table_id,
            seat,
            access_code,
        })?;
        let joined_seat = loop {
            match response.result {
                LobbyResult::Joined {
                    table, seat, ready, ..
                } if table.table_id == table_id => {
                    if ready {
                        break seat;
                    }
                    if !keep_waiting(&table) {
                        // If registration locked concurrently, stay and receive Welcome.
                        match self.request(LobbyRequest::CancelWait { table_id }) {
                            Ok(_) => {
                                let _ = self.close();
                                return Err(NetworkSessionError::JoinCancelled);
                            }
                            Err(NetworkSessionError::Rejected { code, .. })
                                if code == "session_not_waiting" => {}
                            Err(error) => return Err(error),
                        }
                    }
                }
                LobbyResult::Waiting { table, .. } if table.table_id == table_id => {
                    if !keep_waiting(&table) {
                        self.request(LobbyRequest::CancelWait { table_id })?;
                        let _ = self.close();
                        return Err(NetworkSessionError::JoinCancelled);
                    }
                }
                _ => return Err(NetworkSessionError::UnexpectedLobbyMessage),
            }
            thread::sleep(Duration::from_millis(500));
            response = self.request(LobbyRequest::JoinStatus)?;
        };
        let started = Instant::now();
        loop {
            self.read_available()?;
            if let Some(message) = self.decoder.decode_next()? {
                match message {
                    ServerWireMessage::Welcome { update, reconnect } => {
                        return Ok((
                            NetworkSession {
                                stream: self.stream,
                                decoder: self.decoder,
                                closed: self.closed,
                                reconnect,
                            },
                            update,
                            joined_seat,
                        ));
                    }
                    ServerWireMessage::Error { error } => {
                        return Err(NetworkSessionError::Rejected {
                            code: error.code,
                            message: error.message,
                        });
                    }
                    _ => return Err(NetworkSessionError::UnexpectedLobbyMessage),
                }
            }
            if self.closed {
                return Err(NetworkSessionError::ClosedDuringHandshake);
            }
            if started.elapsed() >= CONNECT_TIMEOUT {
                return Err(NetworkSessionError::HandshakeTimedOut);
            }
            thread::sleep(POLL_DELAY);
        }
    }

    pub fn close(&mut self) -> Result<(), NetworkSessionError> {
        self.send(&ClientWireMessage::Close)
    }

    fn request(&mut self, payload: LobbyRequest) -> Result<LobbyResponse, NetworkSessionError> {
        let request_id = format!("lobby-{}", self.next_request);
        self.next_request = self.next_request.saturating_add(1);
        self.send(&ClientWireMessage::Lobby {
            request: LobbyEnvelope::new(request_id.clone(), payload),
        })?;
        let started = Instant::now();
        loop {
            self.read_available()?;
            if let Some(message) = self.decoder.decode_next()? {
                match message {
                    ServerWireMessage::Lobby { response } if response.request_id == request_id => {
                        return Ok(response);
                    }
                    ServerWireMessage::LobbyError { error }
                        if error.request_id.as_deref() == Some(request_id.as_str()) =>
                    {
                        return Err(NetworkSessionError::Rejected {
                            code: error.code,
                            message: error.message,
                        });
                    }
                    ServerWireMessage::Error { error } => {
                        return Err(NetworkSessionError::Rejected {
                            code: error.code,
                            message: error.message,
                        });
                    }
                    _ => return Err(NetworkSessionError::UnexpectedLobbyMessage),
                }
            }
            if self.closed {
                return Err(NetworkSessionError::ClosedDuringHandshake);
            }
            if started.elapsed() >= CONNECT_TIMEOUT {
                return Err(NetworkSessionError::HandshakeTimedOut);
            }
            thread::sleep(POLL_DELAY);
        }
    }

    fn read_available(&mut self) -> Result<(), NetworkSessionError> {
        self.closed = read_available(&mut self.stream, &mut self.decoder)? == ReadStatus::Closed;
        Ok(())
    }

    fn send(&mut self, message: &ClientWireMessage) -> Result<(), NetworkSessionError> {
        write_message(&mut self.stream, message).map_err(Into::into)
    }
}

impl NetworkSession {
    pub fn connect(
        address: SocketAddr,
        session: impl Into<String>,
    ) -> Result<(Self, SubscriptionUpdate), NetworkSessionError> {
        let stream = GameStream::connect(address).map_err(TransportError::Io)?;
        stream.set_nonblocking(true).map_err(TransportError::Io)?;
        stream.set_nodelay(true).map_err(TransportError::Io)?;
        let mut client = Self {
            stream,
            decoder: FrameDecoder::default(),
            closed: false,
            reconnect: None,
        };
        client.send(&ClientWireMessage::Connect {
            version: WIRE_VERSION,
            label: session.into(),
            reconnect: None,
        })?;
        let started = Instant::now();
        loop {
            client.closed =
                read_available(&mut client.stream, &mut client.decoder)? == ReadStatus::Closed;
            if let Some(message) = client.decoder.decode_next()? {
                match message {
                    ServerWireMessage::Welcome { update, reconnect } => {
                        client.reconnect = reconnect;
                        return Ok((client, update));
                    }
                    ServerWireMessage::Error { error } => {
                        return Err(NetworkSessionError::Rejected {
                            code: error.code,
                            message: error.message,
                        });
                    }
                    _ => return Err(NetworkSessionError::UnexpectedHandshakeMessage),
                }
            }
            if client.closed {
                return Err(NetworkSessionError::ClosedDuringHandshake);
            }
            if started.elapsed() >= CONNECT_TIMEOUT {
                return Err(NetworkSessionError::HandshakeTimedOut);
            }
            thread::sleep(POLL_DELAY);
        }
    }

    pub fn reconnect(
        address: SocketAddr,
        label: impl Into<String>,
        token: BearerToken,
    ) -> Result<(Self, SubscriptionUpdate), NetworkSessionError> {
        let stream = GameStream::connect(address).map_err(TransportError::Io)?;
        stream.set_nonblocking(true).map_err(TransportError::Io)?;
        stream.set_nodelay(true).map_err(TransportError::Io)?;
        let mut client = Self {
            stream,
            decoder: FrameDecoder::default(),
            closed: false,
            reconnect: None,
        };
        client.send(&ClientWireMessage::Connect {
            version: WIRE_VERSION,
            label: label.into(),
            reconnect: Some(token),
        })?;
        let started = Instant::now();
        loop {
            client.closed =
                read_available(&mut client.stream, &mut client.decoder)? == ReadStatus::Closed;
            if let Some(message) = client.decoder.decode_next()? {
                match message {
                    ServerWireMessage::Welcome { update, reconnect } => {
                        client.reconnect = reconnect;
                        return Ok((client, update));
                    }
                    ServerWireMessage::Error { error } => {
                        return Err(NetworkSessionError::Rejected {
                            code: error.code,
                            message: error.message,
                        });
                    }
                    _ => return Err(NetworkSessionError::UnexpectedHandshakeMessage),
                }
            }
            if client.closed {
                return Err(NetworkSessionError::ClosedDuringHandshake);
            }
            if started.elapsed() >= CONNECT_TIMEOUT {
                return Err(NetworkSessionError::HandshakeTimedOut);
            }
            thread::sleep(POLL_DELAY);
        }
    }

    pub fn reconnect_token(&self) -> Option<BearerToken> {
        self.reconnect.as_ref().map(|grant| grant.token.clone())
    }

    pub fn reconnect_expiry(&self) -> Option<u64> {
        self.reconnect
            .as_ref()
            .map(|grant| grant.expires_at_unix_seconds)
    }

    pub fn send_command(&mut self, command: CommandEnvelope) -> Result<(), NetworkSessionError> {
        self.send(&ClientWireMessage::Command { command })
    }

    pub fn request_snapshot(&mut self) -> Result<(), NetworkSessionError> {
        self.send(&ClientWireMessage::SnapshotRequest)
    }

    pub fn close(&mut self) -> Result<(), NetworkSessionError> {
        self.send(&ClientWireMessage::Close)
    }

    pub fn poll(&mut self) -> Result<Vec<ServerWireMessage>, NetworkSessionError> {
        if self.closed {
            return Err(NetworkSessionError::Transport(TransportError::Io(
                io::Error::new(io::ErrorKind::UnexpectedEof, "server closed the connection"),
            )));
        }
        self.closed = read_available(&mut self.stream, &mut self.decoder)? == ReadStatus::Closed;
        let mut messages = Vec::new();
        while let Some(message) = self.decoder.decode_next()? {
            messages.push(message);
        }
        if self.closed && messages.is_empty() {
            return Err(NetworkSessionError::Transport(TransportError::Io(
                io::Error::new(io::ErrorKind::UnexpectedEof, "server closed the connection"),
            )));
        }
        Ok(messages)
    }

    fn send(&mut self, message: &ClientWireMessage) -> Result<(), NetworkSessionError> {
        write_message(&mut self.stream, message).map_err(Into::into)
    }
}
