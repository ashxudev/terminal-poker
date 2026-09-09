//! Bounded loopback TCP framing for the local network-alpha candidate.
//!
//! The transport carries existing protocol and authorized-runtime structures.
//! It never owns or mutates poker state.

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{self, Read, Write};

use std::thread;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::authorized_table::{AuthorizedTableResponse, SubscriptionUpdate};
use crate::credentials::{BearerToken, ReconnectGrant};
use crate::lobby::{LobbyEnvelope, LobbyError, LobbyResponse, PublicTableSummary};
use crate::protocol::CommandEnvelope;

pub const WIRE_VERSION: u16 = 5;
pub const MAX_WIRE_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_WIRE_BUFFER_BYTES: usize = (MAX_WIRE_FRAME_BYTES + 4) * 4;
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientWireMessage {
    Connect {
        version: u16,
        label: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reconnect: Option<BearerToken>,
    },
    Lobby {
        request: LobbyEnvelope,
    },
    Command {
        command: CommandEnvelope,
    },
    SnapshotRequest,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerWireMessage {
    LobbyWelcome {
        version: u16,
        lobby_revision: u64,
        capacity: u8,
        tables: Vec<PublicTableSummary>,
    },
    Lobby {
        response: LobbyResponse,
    },
    LobbyError {
        error: LobbyError,
    },
    Welcome {
        update: SubscriptionUpdate,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reconnect: Option<ReconnectGrant>,
    },
    Response {
        response: AuthorizedTableResponse,
    },
    Update {
        update: SubscriptionUpdate,
    },
    Error {
        error: PublicWireError,
    },
    Goodbye,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicWireError {
    pub code: String,
    pub message: String,
}

impl PublicWireError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub enum TransportError {
    Io(io::Error),
    EmptyFrame,
    FrameTooLarge { length: usize },
    BufferLimitExceeded,
    TruncatedFrame,
    MalformedJson(String),
    WriteTimedOut,
}

impl Display for TransportError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "transport I/O failed: {error}"),
            Self::EmptyFrame => write!(formatter, "zero-length wire frames are not allowed"),
            Self::FrameTooLarge { length } => write!(
                formatter,
                "wire frame length {length} exceeds {MAX_WIRE_FRAME_BYTES} bytes"
            ),
            Self::BufferLimitExceeded => write!(
                formatter,
                "undecoded wire buffer exceeds {MAX_WIRE_BUFFER_BYTES} bytes"
            ),
            Self::TruncatedFrame => write!(formatter, "peer closed with a partial wire frame"),
            Self::MalformedJson(message) => write!(formatter, "wire JSON is malformed: {message}"),
            Self::WriteTimedOut => write!(formatter, "wire write exceeded the bounded timeout"),
        }
    }
}

impl Error for TransportError {}

impl From<io::Error> for TransportError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Default)]
pub struct FrameDecoder {
    buffer: Vec<u8>,
}

impl FrameDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.buffer.len().saturating_add(bytes.len()) > MAX_WIRE_BUFFER_BYTES {
            return Err(TransportError::BufferLimitExceeded);
        }
        self.buffer.extend_from_slice(bytes);
        self.validate_announced_length()
    }

    pub fn decode_next<T: DeserializeOwned>(&mut self) -> Result<Option<T>, TransportError> {
        self.validate_announced_length()?;
        if self.buffer.len() < 4 {
            return Ok(None);
        }
        let length = u32::from_be_bytes(self.buffer[..4].try_into().expect("four-byte header"));
        let length = usize::try_from(length).expect("u32 fits usize on supported hosts");
        if self.buffer.len() < 4 + length {
            return Ok(None);
        }
        let payload = self.buffer[4..4 + length].to_vec();
        self.buffer.drain(..4 + length);
        serde_json::from_slice(&payload)
            .map(Some)
            .map_err(|error| TransportError::MalformedJson(error.to_string()))
    }

    pub fn finish(&self) -> Result<(), TransportError> {
        let mut cursor = 0usize;
        while cursor < self.buffer.len() {
            if self.buffer.len() - cursor < 4 {
                return Err(TransportError::TruncatedFrame);
            }
            let length = u32::from_be_bytes(
                self.buffer[cursor..cursor + 4]
                    .try_into()
                    .expect("four-byte header"),
            );
            let length = usize::try_from(length).expect("u32 fits usize on supported hosts");
            if length == 0 {
                return Err(TransportError::EmptyFrame);
            }
            if length > MAX_WIRE_FRAME_BYTES {
                return Err(TransportError::FrameTooLarge { length });
            }
            if self.buffer.len() - cursor - 4 < length {
                return Err(TransportError::TruncatedFrame);
            }
            cursor += 4 + length;
        }
        Ok(())
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    fn validate_announced_length(&self) -> Result<(), TransportError> {
        if self.buffer.len() < 4 {
            return Ok(());
        }
        let length = u32::from_be_bytes(self.buffer[..4].try_into().expect("four-byte header"));
        let length = usize::try_from(length).expect("u32 fits usize on supported hosts");
        if length == 0 {
            return Err(TransportError::EmptyFrame);
        }
        if length > MAX_WIRE_FRAME_BYTES {
            return Err(TransportError::FrameTooLarge { length });
        }
        Ok(())
    }
}

pub fn encode_frame<T: Serialize>(message: &T) -> Result<Vec<u8>, TransportError> {
    let payload = serde_json::to_vec(message)
        .map_err(|error| TransportError::MalformedJson(error.to_string()))?;
    if payload.is_empty() {
        return Err(TransportError::EmptyFrame);
    }
    if payload.len() > MAX_WIRE_FRAME_BYTES {
        return Err(TransportError::FrameTooLarge {
            length: payload.len(),
        });
    }
    let length = u32::try_from(payload.len()).expect("wire frame limit fits u32");
    let mut frame = Vec::with_capacity(4 + payload.len());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub fn write_message<T: Serialize>(
    stream: &mut impl Write,
    message: &T,
) -> Result<(), TransportError> {
    let frame = encode_frame(message)?;
    let started = Instant::now();
    let mut written = 0;
    while written < frame.len() {
        match stream.write(&frame[written..]) {
            Ok(0) => {
                return Err(TransportError::Io(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "peer accepted zero bytes",
                )))
            }
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if started.elapsed() >= WRITE_TIMEOUT {
                    return Err(TransportError::WriteTimedOut);
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => return Err(TransportError::Io(error)),
        }
    }
    loop {
        match stream.flush() {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if started.elapsed() >= WRITE_TIMEOUT {
                    return Err(TransportError::WriteTimedOut);
                }
                thread::sleep(Duration::from_millis(1));
            }
            Err(e) => return Err(TransportError::Io(e)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadStatus {
    Open,
    Closed,
}

pub fn read_available(
    stream: &mut impl Read,
    decoder: &mut FrameDecoder,
) -> Result<ReadStatus, TransportError> {
    let mut bytes = [0u8; 4096];
    loop {
        match stream.read(&mut bytes) {
            Ok(0) => {
                decoder.finish()?;
                return Ok(ReadStatus::Closed);
            }
            Ok(count) => decoder.push(&bytes[..count])?,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(ReadStatus::Open),
            Err(error) => return Err(TransportError::Io(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connect_message() -> ClientWireMessage {
        ClientWireMessage::Connect {
            version: WIRE_VERSION,
            label: "player-s0".to_string(),
            reconnect: None,
        }
    }

    #[test]
    fn split_header_and_payload_decode_incrementally() {
        let frame = encode_frame(&connect_message()).unwrap();
        let mut decoder = FrameDecoder::default();
        decoder.push(&frame[..2]).unwrap();
        assert_eq!(decoder.decode_next::<ClientWireMessage>().unwrap(), None);
        decoder.push(&frame[2..7]).unwrap();
        assert_eq!(decoder.decode_next::<ClientWireMessage>().unwrap(), None);
        decoder.push(&frame[7..]).unwrap();
        assert_eq!(
            decoder.decode_next::<ClientWireMessage>().unwrap(),
            Some(connect_message())
        );
        decoder.finish().unwrap();
    }

    #[test]
    fn coalesced_frames_decode_in_order() {
        let first = connect_message();
        let second = ClientWireMessage::SnapshotRequest;
        let mut bytes = encode_frame(&first).unwrap();
        bytes.extend(encode_frame(&second).unwrap());
        let mut decoder = FrameDecoder::default();
        decoder.push(&bytes).unwrap();
        assert_eq!(
            decoder.decode_next::<ClientWireMessage>().unwrap(),
            Some(first)
        );
        assert_eq!(
            decoder.decode_next::<ClientWireMessage>().unwrap(),
            Some(second)
        );
        assert_eq!(decoder.decode_next::<ClientWireMessage>().unwrap(), None);
    }

    #[test]
    fn zero_and_oversize_lengths_fail_before_payload_retention() {
        let mut zero = FrameDecoder::default();
        assert!(matches!(
            zero.push(&0u32.to_be_bytes()),
            Err(TransportError::EmptyFrame)
        ));

        let mut large = FrameDecoder::default();
        let length = u32::try_from(MAX_WIRE_FRAME_BYTES + 1).unwrap();
        assert!(matches!(
            large.push(&length.to_be_bytes()),
            Err(TransportError::FrameTooLarge { .. })
        ));
    }

    #[test]
    fn malformed_json_and_truncated_close_fail_closed() {
        let payload = b"{not-json}";
        let mut bytes = (payload.len() as u32).to_be_bytes().to_vec();
        bytes.extend(payload);
        let mut malformed = FrameDecoder::default();
        malformed.push(&bytes).unwrap();
        assert!(matches!(
            malformed.decode_next::<ClientWireMessage>(),
            Err(TransportError::MalformedJson(_))
        ));

        let frame = encode_frame(&connect_message()).unwrap();
        let mut truncated = FrameDecoder::default();
        truncated.push(&frame[..frame.len() - 1]).unwrap();
        assert!(matches!(
            truncated.finish(),
            Err(TransportError::TruncatedFrame)
        ));
    }

    #[test]
    fn undecoded_buffer_is_bounded() {
        let mut decoder = FrameDecoder::default();
        let bytes = vec![1u8; MAX_WIRE_BUFFER_BYTES + 1];
        assert!(matches!(
            decoder.push(&bytes),
            Err(TransportError::BufferLimitExceeded)
        ));
        assert_eq!(decoder.buffered_len(), 0);
    }
}
