//! Privacy-safe ring hand histories derived only from accepted public events
//! and a terminal spectator projection.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::game::actions::Action;
use crate::game::deck::Card;
use crate::game::multiway::{MultiwayPhase, PotAward};
use crate::game::seat::SeatId;
use crate::protocol::{
    EventEnvelope, HandId, ProjectionKind, SnapshotEnvelope, TableEvent, TableId,
};

pub const RING_HISTORY_VERSION: u16 = 1;
pub const MAX_HISTORY_ACTIONS: usize = 256;
pub const MAX_RETAINED_HISTORIES: usize = 512;
pub const HISTORY_STORE_VERSION: u16 = 1;
pub const MAX_HISTORY_STORE_BYTES: usize = 4 * 1024 * 1024;
const HISTORY_STORE_FORMAT: &str = "terminal-poker-safe-ring-history";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeRingAction {
    pub revision: u64,
    pub seat: SeatId,
    pub phase: MultiwayPhase,
    pub action: Action,
    pub pot_after: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicCards {
    pub seat: SeatId,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafeRingHandHistory {
    pub version: u16,
    pub table_id: TableId,
    pub hand_id: HandId,
    pub button: SeatId,
    pub board: Vec<Card>,
    pub actions: Vec<SafeRingAction>,
    pub publicly_revealed: Vec<PublicCards>,
    pub awards: Vec<PotAward>,
    pub final_stacks: Vec<(SeatId, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryError {
    PlayerProjection,
    HandNotTerminal,
    WrongTableOrHand,
    NonMonotonicRevision,
    TooManyActions,
}

impl Display for HistoryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::PlayerProjection => "history source must be a public spectator projection",
            Self::HandNotTerminal => "history source hand is not terminal",
            Self::WrongTableOrHand => "history events belong to another table or hand",
            Self::NonMonotonicRevision => "history event revisions are not monotonic",
            Self::TooManyActions => "history contains too many accepted actions",
        };
        formatter.write_str(message)
    }
}

impl Error for HistoryError {}

#[derive(Debug)]
pub enum HistoryStoreError {
    InvalidPath,
    TooLarge,
    Io(std::io::Error),
    InvalidDocument,
}

impl Display for HistoryStoreError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath => write!(formatter, "history path must name a file"),
            Self::TooLarge => write!(
                formatter,
                "safe history store exceeds {MAX_HISTORY_STORE_BYTES} bytes"
            ),
            Self::Io(error) => write!(formatter, "safe history I/O failed: {error}"),
            Self::InvalidDocument => write!(formatter, "safe history document is invalid"),
        }
    }
}

impl Error for HistoryStoreError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryStoreEnvelope {
    format: String,
    version: u16,
    checksum: String,
    payload: HistoryStorePayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryStorePayload {
    retention_limit: usize,
    histories: Vec<SafeRingHandHistory>,
    statistics: RingStatistics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryStoreReceipt {
    pub version: u16,
    pub checksum: String,
    pub bytes: usize,
    pub histories: usize,
}

impl SafeRingHandHistory {
    pub fn from_public_terminal(
        terminal: &SnapshotEnvelope,
        accepted_events: &[EventEnvelope],
    ) -> Result<Self, HistoryError> {
        if terminal.snapshot.audience != ProjectionKind::Spectator {
            return Err(HistoryError::PlayerProjection);
        }
        if !matches!(
            terminal.snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            return Err(HistoryError::HandNotTerminal);
        }
        if accepted_events.len() > MAX_HISTORY_ACTIONS {
            return Err(HistoryError::TooManyActions);
        }
        let mut previous_revision = 0;
        let mut actions = Vec::with_capacity(accepted_events.len());
        for event in accepted_events {
            if event.table_id != terminal.table_id || event.hand_id != terminal.hand_id {
                return Err(HistoryError::WrongTableOrHand);
            }
            if event.revision <= previous_revision || event.revision > terminal.revision {
                return Err(HistoryError::NonMonotonicRevision);
            }
            previous_revision = event.revision;
            match &event.event {
                TableEvent::ShowdownAdvanced | TableEvent::ShowdownPreferenceAccepted { .. } => {}
                TableEvent::ActionAccepted {
                    seat,
                    action,
                    phase,
                    pot_total,
                    ..
                } => actions.push(SafeRingAction {
                    revision: event.revision,
                    seat: *seat,
                    phase: *phase,
                    action: *action,
                    pot_after: *pot_total,
                }),
            }
        }
        let publicly_revealed = terminal
            .snapshot
            .seats
            .iter()
            .filter_map(|seat| {
                seat.hole_cards.as_ref().map(|cards| PublicCards {
                    seat: seat.seat,
                    cards: cards.clone(),
                })
            })
            .collect();
        Ok(Self {
            version: RING_HISTORY_VERSION,
            table_id: terminal.table_id,
            hand_id: terminal.hand_id,
            button: terminal.snapshot.button,
            board: terminal.snapshot.board.clone(),
            actions,
            publicly_revealed,
            awards: terminal.snapshot.awards.clone(),
            final_stacks: terminal
                .snapshot
                .seats
                .iter()
                .map(|seat| (seat.seat, seat.stack))
                .collect(),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RingStatistics {
    pub hands: u64,
    pub showdowns: u64,
    pub accepted_actions: u64,
    pub pots_awarded: u64,
    pub chips_awarded: u64,
}

#[derive(Debug, Default)]
pub struct RingHistoryStore {
    histories: Vec<SafeRingHandHistory>,
    pub statistics: RingStatistics,
}

impl RingHistoryStore {
    pub fn record(&mut self, history: SafeRingHandHistory) {
        if self.histories.len() == MAX_RETAINED_HISTORIES {
            self.histories.remove(0);
        }
        self.statistics.hands = self.statistics.hands.saturating_add(1);
        self.statistics.showdowns = self
            .statistics
            .showdowns
            .saturating_add(u64::from(!history.publicly_revealed.is_empty()));
        self.statistics.accepted_actions = self
            .statistics
            .accepted_actions
            .saturating_add(history.actions.len() as u64);
        self.statistics.pots_awarded = self
            .statistics
            .pots_awarded
            .saturating_add(history.awards.len() as u64);
        self.statistics.chips_awarded = self.statistics.chips_awarded.saturating_add(
            history
                .awards
                .iter()
                .map(|award| u64::from(award.amount))
                .sum::<u64>(),
        );
        self.histories.push(history);
    }

    pub fn histories(&self) -> &[SafeRingHandHistory] {
        &self.histories
    }

    pub fn save_to_path(&self, path: &Path) -> Result<HistoryStoreReceipt, HistoryStoreError> {
        if path.file_name().is_none() {
            return Err(HistoryStoreError::InvalidPath);
        }
        let payload = HistoryStorePayload {
            retention_limit: MAX_RETAINED_HISTORIES,
            histories: self.histories.clone(),
            statistics: self.statistics.clone(),
        };
        let canonical =
            serde_json::to_vec(&payload).map_err(|_| HistoryStoreError::InvalidDocument)?;
        let checksum = format!("sha256:{}", hex(&Sha256::digest(&canonical)));
        let envelope = HistoryStoreEnvelope {
            format: HISTORY_STORE_FORMAT.to_string(),
            version: HISTORY_STORE_VERSION,
            checksum: checksum.clone(),
            payload,
        };
        let bytes =
            serde_json::to_vec_pretty(&envelope).map_err(|_| HistoryStoreError::InvalidDocument)?;
        if bytes.len() > MAX_HISTORY_STORE_BYTES {
            return Err(HistoryStoreError::TooLarge);
        }
        let temporary = temporary_path(path);
        let write_result = (|| -> std::io::Result<()> {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.flush()?;
            file.sync_all()?;
            drop(file);
            crate::table_registry::atomic_replace(&temporary, path)
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(HistoryStoreError::Io(error));
        }
        Ok(HistoryStoreReceipt {
            version: HISTORY_STORE_VERSION,
            checksum,
            bytes: bytes.len(),
            histories: self.histories.len(),
        })
    }

    pub fn load_from_path(path: &Path) -> Result<Self, HistoryStoreError> {
        let metadata = fs::metadata(path).map_err(HistoryStoreError::Io)?;
        if usize::try_from(metadata.len()).unwrap_or(usize::MAX) > MAX_HISTORY_STORE_BYTES {
            return Err(HistoryStoreError::TooLarge);
        }
        let bytes = fs::read(path).map_err(HistoryStoreError::Io)?;
        let envelope: HistoryStoreEnvelope =
            serde_json::from_slice(&bytes).map_err(|_| HistoryStoreError::InvalidDocument)?;
        if envelope.format != HISTORY_STORE_FORMAT
            || envelope.version != HISTORY_STORE_VERSION
            || envelope.payload.retention_limit != MAX_RETAINED_HISTORIES
            || envelope.payload.histories.len() > MAX_RETAINED_HISTORIES
        {
            return Err(HistoryStoreError::InvalidDocument);
        }
        let canonical = serde_json::to_vec(&envelope.payload)
            .map_err(|_| HistoryStoreError::InvalidDocument)?;
        let expected = format!("sha256:{}", hex(&Sha256::digest(&canonical)));
        if !constant_time_eq(envelope.checksum.as_bytes(), expected.as_bytes()) {
            return Err(HistoryStoreError::InvalidDocument);
        }
        validate_payload(&envelope.payload)?;
        Ok(Self {
            histories: envelope.payload.histories,
            statistics: envelope.payload.statistics,
        })
    }
}

fn validate_payload(payload: &HistoryStorePayload) -> Result<(), HistoryStoreError> {
    let mut identities = BTreeSet::new();
    let mut computed = RingStatistics::default();
    for history in &payload.histories {
        if history.version != RING_HISTORY_VERSION
            || history.actions.len() > MAX_HISTORY_ACTIONS
            || history.board.len() > 5
            || history
                .publicly_revealed
                .iter()
                .any(|cards| cards.cards.len() != 2)
            || !identities.insert((history.table_id.0, history.hand_id.0))
        {
            return Err(HistoryStoreError::InvalidDocument);
        }
        computed.hands = computed.hands.saturating_add(1);
        computed.showdowns = computed
            .showdowns
            .saturating_add(u64::from(!history.publicly_revealed.is_empty()));
        computed.accepted_actions = computed
            .accepted_actions
            .saturating_add(history.actions.len() as u64);
        computed.pots_awarded = computed
            .pots_awarded
            .saturating_add(history.awards.len() as u64);
        computed.chips_awarded = computed.chips_awarded.saturating_add(
            history
                .awards
                .iter()
                .map(|award| u64::from(award.amount))
                .sum::<u64>(),
        );
    }
    if payload.statistics.hands < computed.hands
        || payload.statistics.showdowns < computed.showdowns
        || payload.statistics.accepted_actions < computed.accepted_actions
        || payload.statistics.pots_awarded < computed.pots_awarded
        || payload.statistics.chips_awarded < computed.chips_awarded
    {
        return Err(HistoryStoreError::InvalidDocument);
    }
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .expect("validated history path has a file name")
        .to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    result
}

fn constant_time_eq(expected: &[u8], candidate: &[u8]) -> bool {
    let mut difference = expected.len() ^ candidate.len();
    let maximum = expected.len().max(candidate.len());
    for index in 0..maximum {
        difference |= usize::from(
            expected.get(index).copied().unwrap_or(0) ^ candidate.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::multiway::MultiwayHand;
    use crate::game::seat::TableSize;
    use crate::protocol::{CommandEnvelope, ProjectionAudience, ProtocolAuthority};

    fn seat(value: u8) -> SeatId {
        SeatId::new(value).unwrap()
    }

    #[test]
    fn history_accepts_only_terminal_spectator_data_and_public_events() {
        let hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(2).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100)],
            77,
        )
        .unwrap();
        let mut authority = ProtocolAuthority::new(TableId(9), HandId(4), hand);
        let player_before = authority
            .snapshot(ProjectionAudience::Player(seat(0)))
            .unwrap();
        assert_eq!(
            SafeRingHandHistory::from_public_terminal(&player_before, &[]),
            Err(HistoryError::PlayerProjection)
        );
        let actor = authority.hand().to_act.unwrap();
        let event = authority
            .submit(CommandEnvelope::act_for_hand(
                "history-fold",
                TableId(9),
                HandId(4),
                0,
                actor,
                Action::Fold,
            ))
            .unwrap();
        let terminal = authority.snapshot(ProjectionAudience::Spectator).unwrap();
        let history = SafeRingHandHistory::from_public_terminal(&terminal, &[event]).unwrap();
        assert_eq!(history.actions.len(), 1);
        assert!(history.publicly_revealed.is_empty());
        let json = serde_json::to_string(&history).unwrap();
        for forbidden in [
            "command_id",
            "legal_actions",
            "deck",
            "credential",
            "session",
            "random",
        ] {
            assert!(!json.contains(forbidden));
        }
    }

    #[test]
    fn ring_statistics_and_retention_are_mode_specific_and_bounded() {
        let template = SafeRingHandHistory {
            version: 1,
            table_id: TableId(1),
            hand_id: HandId(1),
            button: seat(0),
            board: vec![],
            actions: vec![],
            publicly_revealed: vec![],
            awards: vec![],
            final_stacks: vec![(seat(0), 100)],
        };
        let mut store = RingHistoryStore::default();
        for index in 0..=MAX_RETAINED_HISTORIES {
            let mut history = template.clone();
            history.hand_id = HandId(index as u64 + 1);
            store.record(history);
        }
        assert_eq!(store.histories().len(), MAX_RETAINED_HISTORIES);
        assert_eq!(store.statistics.hands, MAX_RETAINED_HISTORIES as u64 + 1);
        assert_eq!(store.histories()[0].hand_id, HandId(2));
    }

    #[test]
    fn separate_versioned_store_round_trips_and_corruption_fails_closed() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "terminal-poker-safe-history-{}.json",
            std::process::id()
        ));
        let mut corrupt = path.clone();
        corrupt.set_extension("corrupt.json");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&corrupt);
        let mut store = RingHistoryStore::default();
        store.record(SafeRingHandHistory {
            version: RING_HISTORY_VERSION,
            table_id: TableId(11),
            hand_id: HandId(17),
            button: seat(0),
            board: vec![],
            actions: vec![],
            publicly_revealed: vec![],
            awards: vec![],
            final_stacks: vec![(seat(0), 100), (seat(1), 100)],
        });
        let receipt = store.save_to_path(&path).unwrap();
        assert_eq!(receipt.version, HISTORY_STORE_VERSION);
        assert_eq!(receipt.histories, 1);
        assert!(receipt.bytes <= MAX_HISTORY_STORE_BYTES);
        let serialized = fs::read_to_string(&path).unwrap();
        for forbidden in [
            "credential",
            "principal",
            "session",
            "join_code",
            "legal_actions",
            "deck",
            "random",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "history leaked {forbidden}"
            );
        }
        let restored = RingHistoryStore::load_from_path(&path).unwrap();
        assert_eq!(restored.histories(), store.histories());
        assert_eq!(restored.statistics, store.statistics);
        fs::write(
            &corrupt,
            serialized.replacen("\"hand_id\": 17", "\"hand_id\": 18", 1),
        )
        .unwrap();
        assert!(matches!(
            RingHistoryStore::load_from_path(&corrupt),
            Err(HistoryStoreError::InvalidDocument)
        ));
        fs::remove_file(path).unwrap();
        fs::remove_file(corrupt).unwrap();
    }
}
