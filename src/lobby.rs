//! Versioned, bounded public lobby contracts.
//!
//! Lobby projections are constructed independently from poker-hand projections.
//! They contain only public configuration and aggregate lifecycle information.

use serde::{Deserialize, Serialize};

use crate::game::seat::{SeatId, TableSize};
use crate::protocol::{HandId, TableId};
use crate::tournament::{TournamentConfig, TournamentPublicState};

pub const LOBBY_PROTOCOL_VERSION: u16 = 2;
pub const MAX_PUBLIC_TABLE_NAME_BYTES: usize = 32;
pub const MAX_LOBBY_REQUEST_ID_BYTES: usize = 64;
pub const MIN_PRIVATE_JOIN_CODE_BYTES: usize = 24;
pub const MAX_PRIVATE_JOIN_CODE_BYTES: usize = 96;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TableVisibility {
    #[default]
    Public,
    PasswordProtected,
    Unlisted,
    Private,
}

const fn default_min_players() -> u8 {
    2
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicTableConfig {
    pub name: String,
    pub seats: TableSize,
    pub starting_stack: u32,
    #[serde(default = "default_min_players")]
    pub min_players: u8,
    #[serde(default)]
    pub visibility: TableVisibility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicTableStatus {
    Waiting,
    Running,
    Paused,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicTableSummary {
    pub table_id: TableId,
    pub name: String,
    pub seats: TableSize,
    pub starting_stack: u32,
    pub min_players: u8,
    pub small_blind: u32,
    pub big_blind: u32,
    pub occupied: u8,
    pub reserved: u8,
    pub waiting: u8,
    pub status: PublicTableStatus,
    pub joinable: bool,
    pub visibility: TableVisibility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tournament: Option<TournamentPublicState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableHealthSummary {
    pub table_id: TableId,
    pub status: PublicTableStatus,
    pub hand_id: Option<HandId>,
    pub occupied: u8,
    pub waiting: u8,
    pub stream_sequence: u64,
    pub accepted_commands: u64,
    pub rejected_commands: u64,
    pub authorization_rejections: u64,
    pub disconnects: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryHealth {
    pub healthy: bool,
    pub lobby_revision: u64,
    pub table_capacity: usize,
    pub tables: usize,
    pub routed_sessions: usize,
    pub waiting_sessions: usize,
    pub active_capabilities: usize,
    pub capability_capacity: usize,
    pub retained_safe_histories: usize,
    pub last_checkpoint_millis: u64,
    pub max_checkpoint_millis: u64,
    pub last_checkpoint_bytes: usize,
    pub checkpoint_version: u16,
    pub recovery_boundary: String,
    pub table_health: Vec<TableHealthSummary>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublicTableFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seats: Option<TableSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub starting_stack: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<PublicTableStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub joinable: Option<bool>,
}

impl PublicTableFilter {
    pub fn matches(&self, table: &PublicTableSummary) -> bool {
        self.seats.is_none_or(|value| value == table.seats)
            && self
                .starting_stack
                .is_none_or(|value| value == table.starting_stack)
            && self.status.is_none_or(|value| value == table.status)
            && self.joinable.is_none_or(|value| value == table.joinable)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LobbyEnvelope {
    pub version: u16,
    pub request_id: String,
    pub payload: LobbyRequest,
}

impl LobbyEnvelope {
    pub fn new(request_id: impl Into<String>, payload: LobbyRequest) -> Self {
        Self {
            version: LOBBY_PROTOCOL_VERSION,
            request_id: request_id.into(),
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LobbyRequest {
    Create {
        config: PublicTableConfig,
    },
    CreateTournament {
        config: TournamentConfig,
    },
    List {
        filter: PublicTableFilter,
    },
    Inspect {
        table_id: TableId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_code: Option<String>,
    },
    Join {
        table_id: TableId,
        #[serde(skip_serializing_if = "Option::is_none")]
        seat: Option<SeatId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_code: Option<String>,
    },
    JoinStatus,
    CancelWait {
        table_id: TableId,
    },
    Health,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LobbyResponse {
    pub version: u16,
    pub request_id: String,
    pub lobby_revision: u64,
    pub result: LobbyResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum LobbyResult {
    Tables {
        tables: Vec<PublicTableSummary>,
    },
    Table {
        table: PublicTableSummary,
    },
    Joined {
        table: PublicTableSummary,
        seat: SeatId,
        #[serde(skip_serializing_if = "Option::is_none")]
        hand_id: Option<HandId>,
        ready: bool,
    },
    Waiting {
        table: PublicTableSummary,
        position: u8,
    },
    WaitCancelled {
        table_id: TableId,
    },
    Health {
        health: RegistryHealth,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LobbyError {
    pub version: u16,
    pub request_id: Option<String>,
    pub lobby_revision: u64,
    pub code: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_summary_serialization_is_an_explicit_allowlist() {
        let summary = PublicTableSummary {
            table_id: TableId(7),
            name: "River Room".to_string(),
            seats: TableSize::new(6).unwrap(),
            starting_stack: 200,
            min_players: 2,
            small_blind: 1,
            big_blind: 2,
            occupied: 3,
            reserved: 1,
            waiting: 0,
            status: PublicTableStatus::Waiting,
            joinable: true,
            visibility: TableVisibility::Public,
            tournament: None,
        };
        let json = serde_json::to_string(&summary).unwrap();
        for forbidden in [
            "hole_cards",
            "deck",
            "random",
            "session",
            "command",
            "reconnect",
            "subscription",
            "player_id",
        ] {
            assert!(!json.contains(forbidden), "leaked {forbidden}: {json}");
        }
        assert_eq!(
            serde_json::from_str::<PublicTableSummary>(&json).unwrap(),
            summary
        );
    }

    #[test]
    fn lobby_envelopes_reject_unknown_fields_and_preserve_version() {
        let message = LobbyEnvelope::new(
            "list-1",
            LobbyRequest::List {
                filter: PublicTableFilter::default(),
            },
        );
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"version\":2"));
        let hostile = json.replacen("{", "{\"unexpected\":true,", 1);
        assert!(serde_json::from_str::<LobbyEnvelope>(&hostile).is_err());
    }
}
