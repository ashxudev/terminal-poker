//! Safe table-roster lifecycle around the authoritative [`MultiwayHand`].
//!
//! The lifecycle owns identity, reservation, seating, and between-hand policy.
//! A hand receives an immutable seat/stack snapshot; mid-hand roster changes are
//! queued and can only take effect after the completed hand is reconciled.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::multiway::{BlindValues, ForcedPost, MultiwayConfigError, MultiwayHand};
use super::seat::{PlayerId, SeatId, TableSize};
use super::state::{BIG_BLIND, SMALL_BLIND};
use super::table::{
    BlindObligation, ConnectionState, HandParticipation, SeatMapError, SeatState,
    TableParticipation, TableSeats,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableRunState {
    Waiting,
    Running,
    Paused,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PendingSeatTransition {
    SitOut,
    Return,
    Leave,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandStart {
    pub number: u64,
    pub button: SeatId,
    pub stacks: Vec<(SeatId, u32)>,
    pub forced_posts: Vec<ForcedPost>,
}

/// Explicit persistence allowlist for a table at a between-hand boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BetweenHandLifecycle {
    pub table_size: TableSize,
    pub seats: Vec<BetweenHandSeat>,
    pub state: TableRunState,
    pub last_button: Option<SeatId>,
    pub next_hand_number: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BetweenHandSeat {
    pub seat: SeatId,
    pub player: PlayerId,
    pub stack: u32,
    pub participation: TableParticipation,
    #[serde(default)]
    pub blind_obligation: BlindObligation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingEntryChoice {
    WaitForBigBlind,
    PostLiveBigBlind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedBlind {
    Small,
    Big,
}

impl HandStart {
    pub fn into_hand(
        self,
        table_size: TableSize,
        deterministic_seed: Option<u64>,
    ) -> Result<MultiwayHand, MultiwayConfigError> {
        match deterministic_seed {
            Some(seed) => MultiwayHand::new_seeded_with_forced_posts(
                table_size,
                self.button,
                &self.stacks,
                &self.forced_posts,
                seed,
            ),
            None => MultiwayHand::new_with_forced_posts(
                table_size,
                self.button,
                &self.stacks,
                &self.forced_posts,
            ),
        }
    }

    /// Builds an authoritative hand using the current tournament level.
    /// Ring-game callers retain [`Self::into_hand`] and its 1/2 defaults.
    pub fn into_hand_with_blinds(
        self,
        table_size: TableSize,
        deterministic_seed: Option<u64>,
        blind_values: BlindValues,
    ) -> Result<MultiwayHand, MultiwayConfigError> {
        match deterministic_seed {
            Some(seed) => MultiwayHand::new_seeded_with_blinds(
                table_size,
                self.button,
                &self.stacks,
                &self.forced_posts,
                blind_values,
                seed,
            ),
            None => MultiwayHand::new_with_blinds(
                table_size,
                self.button,
                &self.stacks,
                &self.forced_posts,
                blind_values,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    Closed,
    HandActive,
    NoActiveHand,
    TooFewEligiblePlayers(usize),
    InvalidStack,
    SeatOutsideTable(SeatId),
    SeatOccupied(SeatId),
    SeatVacant(SeatId),
    SeatNotReserved(SeatId),
    ReservationOwnedByAnotherPlayer(SeatId),
    IdentityAlreadyAtTable(PlayerId),
    PlayerNotSeated(PlayerId),
    MissingHandSeat(SeatId),
    UnexpectedHandSeat(SeatId),
    DuplicateHandSeat(SeatId),
    ChipTotalMismatch { expected: u32, actual: u32 },
    InvalidCheckpoint,
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => write!(formatter, "table is closed"),
            Self::HandActive => write!(formatter, "operation requires a between-hand boundary"),
            Self::NoActiveHand => write!(formatter, "no active hand to complete"),
            Self::TooFewEligiblePlayers(count) => write!(
                formatter,
                "at least two eligible players are required, found {count}"
            ),
            Self::InvalidStack => write!(formatter, "seat stack must be positive"),
            Self::SeatOutsideTable(seat) => {
                write!(formatter, "seat {} is outside this table", seat.as_u8())
            }
            Self::SeatOccupied(seat) => write!(formatter, "seat {} is occupied", seat.as_u8()),
            Self::SeatVacant(seat) => write!(formatter, "seat {} is vacant", seat.as_u8()),
            Self::SeatNotReserved(seat) => {
                write!(formatter, "seat {} has no reservation", seat.as_u8())
            }
            Self::ReservationOwnedByAnotherPlayer(seat) => write!(
                formatter,
                "seat {} is reserved by another player",
                seat.as_u8()
            ),
            Self::IdentityAlreadyAtTable(player) => write!(
                formatter,
                "player {} already occupies or reserves a seat",
                player.value()
            ),
            Self::PlayerNotSeated(player) => {
                write!(formatter, "player {} is not seated", player.value())
            }
            Self::MissingHandSeat(seat) => write!(
                formatter,
                "completed stack report omitted hand seat {}",
                seat.as_u8()
            ),
            Self::UnexpectedHandSeat(seat) => write!(
                formatter,
                "completed stack report included non-hand seat {}",
                seat.as_u8()
            ),
            Self::DuplicateHandSeat(seat) => write!(
                formatter,
                "completed stack report duplicated seat {}",
                seat.as_u8()
            ),
            Self::ChipTotalMismatch { expected, actual } => write!(
                formatter,
                "completed hand chip total {actual} does not match starting total {expected}"
            ),
            Self::InvalidCheckpoint => {
                formatter.write_str("between-hand lifecycle checkpoint is inconsistent")
            }
        }
    }
}

impl std::error::Error for LifecycleError {}

#[derive(Debug, Clone)]
pub struct TableLifecycle {
    table_size: TableSize,
    seats: TableSeats,
    reservations: BTreeMap<SeatId, PlayerId>,
    pending: BTreeMap<PlayerId, PendingSeatTransition>,
    state: TableRunState,
    active_hand: Option<HandStart>,
    last_button: Option<SeatId>,
    next_hand_number: u64,
}

impl TableLifecycle {
    pub fn new(table_size: TableSize) -> Self {
        Self {
            table_size,
            seats: TableSeats::new(table_size),
            reservations: BTreeMap::new(),
            pending: BTreeMap::new(),
            state: TableRunState::Waiting,
            active_hand: None,
            last_button: None,
            next_hand_number: 1,
        }
    }

    pub const fn table_size(&self) -> TableSize {
        self.table_size
    }

    pub const fn state(&self) -> TableRunState {
        self.state
    }

    pub const fn hand_active(&self) -> bool {
        self.active_hand.is_some()
    }

    pub const fn seats(&self) -> &TableSeats {
        &self.seats
    }

    pub fn reservations(&self) -> impl Iterator<Item = (SeatId, PlayerId)> + '_ {
        self.reservations
            .iter()
            .map(|(&seat, &player)| (seat, player))
    }

    pub fn pending(&self) -> impl Iterator<Item = (PlayerId, PendingSeatTransition)> + '_ {
        self.pending
            .iter()
            .map(|(&player, &transition)| (player, transition))
    }

    pub fn between_hand_checkpoint(&self) -> Result<BetweenHandLifecycle, LifecycleError> {
        if self.hand_active() {
            return Err(LifecycleError::HandActive);
        }
        if !self.pending.is_empty() {
            return Err(LifecycleError::HandActive);
        }
        Ok(BetweenHandLifecycle {
            table_size: self.table_size,
            seats: self
                .seats
                .occupied()
                .map(|(seat, state)| BetweenHandSeat {
                    seat,
                    player: state.player_id(),
                    stack: state.stack,
                    participation: state.table_participation,
                    blind_obligation: state.blind_obligation,
                })
                .collect(),
            state: self.state,
            last_button: self.last_button,
            next_hand_number: self.next_hand_number,
        })
    }

    pub fn restore_between_hand(checkpoint: &BetweenHandLifecycle) -> Result<Self, LifecycleError> {
        if checkpoint.next_hand_number == 0
            || checkpoint
                .last_button
                .is_some_and(|seat| !checkpoint.table_size.contains(seat))
            || checkpoint.state == TableRunState::Closed
        {
            return Err(LifecycleError::InvalidCheckpoint);
        }
        let mut seats = TableSeats::new(checkpoint.table_size);
        for restored in &checkpoint.seats {
            if !checkpoint.table_size.contains(restored.seat)
                || restored.participation == TableParticipation::Leaving
            {
                return Err(LifecycleError::InvalidCheckpoint);
            }
            let mut state = SeatState::active(restored.player, restored.stack);
            state.connection = ConnectionState::Disconnected;
            state.table_participation = restored.participation;
            state.blind_obligation = restored.blind_obligation;
            seats
                .occupy(restored.seat, state)
                .map_err(Self::map_seat_error)?;
        }
        let lifecycle = Self {
            table_size: checkpoint.table_size,
            seats,
            reservations: BTreeMap::new(),
            pending: BTreeMap::new(),
            state: checkpoint.state,
            active_hand: None,
            last_button: checkpoint.last_button,
            next_hand_number: checkpoint.next_hand_number,
        };
        let eligible = lifecycle.eligible_count();
        let occupied = lifecycle.seats.occupied_count();
        let state_valid = match checkpoint.state {
            TableRunState::Waiting => occupied == 0 || checkpoint.next_hand_number == 1,
            TableRunState::Running => eligible >= 2,
            TableRunState::Paused => occupied > 0 && eligible < 2,
            TableRunState::Closed => false,
        };
        if !state_valid {
            return Err(LifecycleError::InvalidCheckpoint);
        }
        Ok(lifecycle)
    }

    pub fn eligible_count(&self) -> usize {
        self.seats
            .occupied()
            .filter(|(_, state)| state.eligible_for_next_hand())
            .count()
    }

    pub fn reserve(&mut self, player: PlayerId, seat: SeatId) -> Result<(), LifecycleError> {
        self.ensure_open()?;
        self.ensure_seat(seat)?;
        if self.identity_present(player) {
            return Err(LifecycleError::IdentityAlreadyAtTable(player));
        }
        if self.seats.seat(seat).is_some() {
            return Err(LifecycleError::SeatOccupied(seat));
        }
        if self.reservations.contains_key(&seat) {
            return Err(LifecycleError::ReservationOwnedByAnotherPlayer(seat));
        }
        self.reservations.insert(seat, player);
        Ok(())
    }

    pub fn cancel_reservation(
        &mut self,
        player: PlayerId,
        seat: SeatId,
    ) -> Result<(), LifecycleError> {
        self.ensure_open()?;
        match self.reservations.get(&seat).copied() {
            None => Err(LifecycleError::SeatNotReserved(seat)),
            Some(owner) if owner != player => {
                Err(LifecycleError::ReservationOwnedByAnotherPlayer(seat))
            }
            Some(_) => {
                self.reservations.remove(&seat);
                Ok(())
            }
        }
    }

    pub fn occupy(
        &mut self,
        player: PlayerId,
        seat: SeatId,
        stack: u32,
    ) -> Result<(), LifecycleError> {
        self.ensure_open()?;
        if self.hand_active() {
            return Err(LifecycleError::HandActive);
        }
        if stack == 0 {
            return Err(LifecycleError::InvalidStack);
        }
        self.ensure_seat(seat)?;
        if self.seats.seat_for_player(player).is_some() {
            return Err(LifecycleError::IdentityAlreadyAtTable(player));
        }
        match self.reservations.get(&seat).copied() {
            None => return Err(LifecycleError::SeatNotReserved(seat)),
            Some(owner) if owner != player => {
                return Err(LifecycleError::ReservationOwnedByAnotherPlayer(seat));
            }
            Some(_) => {}
        }
        self.seats
            .occupy(seat, SeatState::active(player, stack))
            .map_err(Self::map_seat_error)?;
        self.reservations.remove(&seat);
        self.refresh_between_hand_state();
        Ok(())
    }

    pub fn join(
        &mut self,
        player: PlayerId,
        seat: SeatId,
        stack: u32,
    ) -> Result<(), LifecycleError> {
        if stack == 0 {
            return Err(LifecycleError::InvalidStack);
        }
        self.reserve(player, seat)?;
        if let Err(error) = self.occupy(player, seat, stack) {
            self.reservations.remove(&seat);
            return Err(error);
        }
        Ok(())
    }

    pub fn join_with_entry(
        &mut self,
        player: PlayerId,
        seat: SeatId,
        stack: u32,
        entry: RingEntryChoice,
    ) -> Result<(), LifecycleError> {
        self.join(player, seat, stack)?;
        if self.next_hand_number > 1 {
            let state = self
                .seats
                .seat_mut(seat)
                .expect("newly joined player remains seated");
            state.blind_obligation = BlindObligation::OwesBigBlind;
            state.table_participation = match entry {
                RingEntryChoice::WaitForBigBlind => TableParticipation::WaitingForBlind,
                RingEntryChoice::PostLiveBigBlind => TableParticipation::Active,
            };
            self.refresh_between_hand_state();
        }
        Ok(())
    }

    pub fn blind_obligation(&self, player: PlayerId) -> Result<BlindObligation, LifecycleError> {
        let seat = self
            .seats
            .seat_for_player(player)
            .ok_or(LifecycleError::PlayerNotSeated(player))?;
        Ok(self
            .seats
            .seat(seat)
            .expect("player lookup returned an occupied seat")
            .blind_obligation)
    }

    pub fn record_missed_blind(
        &mut self,
        player: PlayerId,
        missed: MissedBlind,
    ) -> Result<(), LifecycleError> {
        self.ensure_open()?;
        if self.hand_active() {
            return Err(LifecycleError::HandActive);
        }
        let seat = self
            .seats
            .seat_for_player(player)
            .ok_or(LifecycleError::PlayerNotSeated(player))?;
        let state = self
            .seats
            .seat_mut(seat)
            .expect("player lookup returned an occupied seat");
        state.blind_obligation = match (state.blind_obligation, missed) {
            (BlindObligation::Clear, MissedBlind::Small) => BlindObligation::OwesSmallBlind,
            (BlindObligation::Clear, MissedBlind::Big) => BlindObligation::OwesBigBlind,
            (BlindObligation::OwesSmallBlind, MissedBlind::Big)
            | (BlindObligation::OwesBigBlind, MissedBlind::Small) => BlindObligation::OwesBoth,
            (existing, _) => existing,
        };
        Ok(())
    }

    pub fn request_return_with_entry(
        &mut self,
        player: PlayerId,
        entry: RingEntryChoice,
    ) -> Result<(), LifecycleError> {
        self.ensure_open()?;
        if self.hand_active() {
            return Err(LifecycleError::HandActive);
        }
        let seat = self
            .seats
            .seat_for_player(player)
            .ok_or(LifecycleError::PlayerNotSeated(player))?;
        let state = self
            .seats
            .seat_mut(seat)
            .expect("player lookup returned an occupied seat");
        state.table_participation = if state.blind_obligation == BlindObligation::Clear {
            TableParticipation::Active
        } else {
            match entry {
                RingEntryChoice::WaitForBigBlind => TableParticipation::WaitingForBlind,
                RingEntryChoice::PostLiveBigBlind => TableParticipation::Active,
            }
        };
        self.refresh_between_hand_state();
        Ok(())
    }

    pub fn set_connection(
        &mut self,
        player: PlayerId,
        connection: ConnectionState,
    ) -> Result<(), LifecycleError> {
        let seat = self
            .seats
            .seat_for_player(player)
            .ok_or(LifecycleError::PlayerNotSeated(player))?;
        self.seats
            .seat_mut(seat)
            .expect("player lookup returned an occupied seat")
            .connection = connection;
        Ok(())
    }

    pub fn request_sit_out(&mut self, player: PlayerId) -> Result<(), LifecycleError> {
        self.request_transition(player, PendingSeatTransition::SitOut)
    }

    pub fn request_return(&mut self, player: PlayerId) -> Result<(), LifecycleError> {
        self.request_transition(player, PendingSeatTransition::Return)
    }

    pub fn request_leave(&mut self, player: PlayerId) -> Result<(), LifecycleError> {
        self.request_transition(player, PendingSeatTransition::Leave)
    }

    pub fn begin_hand(&mut self) -> Result<HandStart, LifecycleError> {
        self.ensure_open()?;
        if self.hand_active() {
            return Err(LifecycleError::HandActive);
        }
        self.promote_natural_big_blind_waiter();
        let eligible = self.eligible_count();
        if eligible < 2 {
            self.state = if self.seats.occupied_count() == 0 {
                TableRunState::Waiting
            } else {
                TableRunState::Paused
            };
            return Err(LifecycleError::TooFewEligiblePlayers(eligible));
        }
        let button = self
            .next_button()
            .expect("two eligible seats provide a button");
        let stacks = self
            .seats
            .occupied()
            .filter(|(_, state)| state.eligible_for_next_hand())
            .map(|(seat, state)| (seat, state.stack))
            .collect::<Vec<_>>();
        let positions = self
            .seats
            .positions(button)
            .expect("eligible lifecycle seats provide valid positions");
        let mut forced_posts = Vec::new();
        for &(seat, _) in &stacks {
            let state = self
                .seats
                .seat_mut(seat)
                .expect("eligible seat remains occupied");
            match state.blind_obligation {
                BlindObligation::Clear => {}
                BlindObligation::OwesSmallBlind => {
                    if seat != positions.small_blind {
                        forced_posts.push(ForcedPost {
                            seat,
                            amount: SMALL_BLIND,
                            live: false,
                        });
                    }
                }
                BlindObligation::OwesBigBlind => {
                    if seat != positions.big_blind {
                        forced_posts.push(ForcedPost {
                            seat,
                            amount: BIG_BLIND,
                            live: true,
                        });
                    }
                }
                BlindObligation::OwesBoth => {
                    if seat != positions.small_blind {
                        forced_posts.push(ForcedPost {
                            seat,
                            amount: SMALL_BLIND,
                            live: false,
                        });
                    }
                    if seat != positions.big_blind {
                        forced_posts.push(ForcedPost {
                            seat,
                            amount: BIG_BLIND,
                            live: true,
                        });
                    }
                }
            }
            state.blind_obligation = BlindObligation::Clear;
            state.hand_participation = HandParticipation::Live;
        }
        let start = HandStart {
            number: self.next_hand_number,
            button,
            stacks,
            forced_posts,
        };
        self.next_hand_number += 1;
        self.last_button = Some(button);
        self.active_hand = Some(start.clone());
        self.state = TableRunState::Running;
        Ok(start)
    }

    pub fn complete_hand(&mut self, final_stacks: &[(SeatId, u32)]) -> Result<(), LifecycleError> {
        let start = self
            .active_hand
            .as_ref()
            .ok_or(LifecycleError::NoActiveHand)?;
        let expected = start.stacks.iter().copied().collect::<BTreeMap<_, _>>();
        let mut actual = BTreeMap::new();
        for &(seat, stack) in final_stacks {
            if actual.insert(seat, stack).is_some() {
                return Err(LifecycleError::DuplicateHandSeat(seat));
            }
            if !expected.contains_key(&seat) {
                return Err(LifecycleError::UnexpectedHandSeat(seat));
            }
        }
        if let Some(&seat) = expected.keys().find(|seat| !actual.contains_key(seat)) {
            return Err(LifecycleError::MissingHandSeat(seat));
        }
        let expected_total = expected.values().try_fold(0u32, |total, stack| {
            total
                .checked_add(*stack)
                .ok_or(LifecycleError::ChipTotalMismatch {
                    expected: u32::MAX,
                    actual: u32::MAX,
                })
        })?;
        let actual_total = actual.values().try_fold(0u32, |total, stack| {
            total
                .checked_add(*stack)
                .ok_or(LifecycleError::ChipTotalMismatch {
                    expected: expected_total,
                    actual: u32::MAX,
                })
        })?;
        if actual_total != expected_total {
            return Err(LifecycleError::ChipTotalMismatch {
                expected: expected_total,
                actual: actual_total,
            });
        }

        for (&seat, &stack) in &actual {
            let state = self
                .seats
                .seat_mut(seat)
                .expect("active hand seats remain occupied until boundary");
            state.stack = stack;
            state.street_bet = 0;
            state.hole_cards.clear();
            state.hand_participation = HandParticipation::NotDealt;
        }
        self.active_hand = None;
        self.apply_pending();
        self.refresh_between_hand_state();
        Ok(())
    }

    pub fn close(&mut self) -> Result<(), LifecycleError> {
        self.ensure_open()?;
        if self.hand_active() {
            return Err(LifecycleError::HandActive);
        }
        self.state = TableRunState::Closed;
        self.reservations.clear();
        self.pending.clear();
        Ok(())
    }

    fn request_transition(
        &mut self,
        player: PlayerId,
        transition: PendingSeatTransition,
    ) -> Result<(), LifecycleError> {
        self.ensure_open()?;
        if self.seats.seat_for_player(player).is_none() {
            return Err(LifecycleError::PlayerNotSeated(player));
        }
        if self.hand_active() {
            let prior = self.pending.get(&player).copied();
            if prior != Some(PendingSeatTransition::Leave) {
                self.pending.insert(player, transition);
            }
        } else {
            self.apply_transition(player, transition);
            self.refresh_between_hand_state();
        }
        Ok(())
    }

    fn apply_pending(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for (player, transition) in pending {
            self.apply_transition(player, transition);
        }
    }

    fn apply_transition(&mut self, player: PlayerId, transition: PendingSeatTransition) {
        let Some(seat) = self.seats.seat_for_player(player) else {
            return;
        };
        match transition {
            PendingSeatTransition::SitOut => {
                self.seats
                    .seat_mut(seat)
                    .expect("player lookup returned an occupied seat")
                    .table_participation = TableParticipation::SittingOut;
            }
            PendingSeatTransition::Return => {
                let state = self
                    .seats
                    .seat_mut(seat)
                    .expect("player lookup returned an occupied seat");
                state.table_participation = if state.blind_obligation == BlindObligation::Clear {
                    TableParticipation::Active
                } else {
                    TableParticipation::WaitingForBlind
                };
            }
            PendingSeatTransition::Leave => {
                self.seats
                    .vacate(seat)
                    .expect("player lookup returned an occupied seat");
            }
        }
    }

    fn next_button(&self) -> Option<SeatId> {
        match self.last_button {
            Some(last) => self.seats.next_for_hand(last),
            None => self.table_size.seats().find(|&seat| {
                self.seats
                    .seat(seat)
                    .is_some_and(SeatState::eligible_for_next_hand)
            }),
        }
    }

    fn promote_natural_big_blind_waiter(&mut self) {
        let waiting = self
            .seats
            .occupied()
            .filter_map(|(seat, state)| {
                (state.table_participation == TableParticipation::WaitingForBlind).then_some(seat)
            })
            .collect::<Vec<_>>();
        for candidate in waiting {
            let prospective = self
                .seats
                .occupied()
                .filter_map(|(seat, state)| {
                    (state.eligible_for_next_hand() || seat == candidate).then_some(seat)
                })
                .collect::<BTreeSet<_>>();
            if prospective.len() < 2 {
                continue;
            }
            let button = match self.last_button {
                Some(last) => self
                    .table_size
                    .next_eligible(last, |seat| prospective.contains(&seat)),
                None => self
                    .table_size
                    .seats()
                    .find(|seat| prospective.contains(seat)),
            };
            let Some(button) = button else {
                continue;
            };
            let first = self
                .table_size
                .next_eligible(button, |seat| prospective.contains(&seat))
                .expect("prospective hand has a blind seat");
            let big_blind = if prospective.len() == 2 {
                first
            } else {
                self.table_size
                    .next_eligible(first, |seat| prospective.contains(&seat))
                    .expect("three prospective seats have a big blind")
            };
            if big_blind == candidate {
                let state = self
                    .seats
                    .seat_mut(candidate)
                    .expect("waiting candidate remains occupied");
                state.table_participation = TableParticipation::Active;
                state.blind_obligation = BlindObligation::Clear;
                break;
            }
        }
    }

    fn refresh_between_hand_state(&mut self) {
        if self.state == TableRunState::Closed || self.hand_active() {
            return;
        }
        let eligible = self.eligible_count();
        self.state = match (self.state, eligible, self.seats.occupied_count()) {
            (_, _, 0) => TableRunState::Waiting,
            (TableRunState::Running | TableRunState::Paused, 2.., _) => TableRunState::Running,
            (TableRunState::Running | TableRunState::Paused, _, _) => TableRunState::Paused,
            _ => TableRunState::Waiting,
        };
    }

    fn identity_present(&self, player: PlayerId) -> bool {
        self.seats.seat_for_player(player).is_some()
            || self.reservations.values().any(|&owner| owner == player)
    }

    fn ensure_open(&self) -> Result<(), LifecycleError> {
        if self.state == TableRunState::Closed {
            Err(LifecycleError::Closed)
        } else {
            Ok(())
        }
    }

    fn ensure_seat(&self, seat: SeatId) -> Result<(), LifecycleError> {
        if self.table_size.contains(seat) {
            Ok(())
        } else {
            Err(LifecycleError::SeatOutsideTable(seat))
        }
    }

    fn map_seat_error(error: SeatMapError) -> LifecycleError {
        match error {
            SeatMapError::SeatOutsideTable { seat, .. } => LifecycleError::SeatOutsideTable(seat),
            SeatMapError::SeatOccupied(seat) => LifecycleError::SeatOccupied(seat),
            SeatMapError::SeatVacant(seat) => LifecycleError::SeatVacant(seat),
            SeatMapError::DuplicatePlayer(player) => LifecycleError::IdentityAlreadyAtTable(player),
        }
    }
}

pub fn final_stacks(hand: &MultiwayHand) -> Vec<(SeatId, u32)> {
    hand.occupied_seats()
        .map(|seat| (seat, hand.seat(seat).stack))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::actions::Action;
    use crate::game::command::SeatCommand;
    use std::collections::BTreeSet;

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    fn player(index: u64) -> PlayerId {
        PlayerId::new(index)
    }

    fn joined_table(count: u8) -> TableLifecycle {
        let mut table = TableLifecycle::new(TableSize::new(9).unwrap());
        for index in 0..count {
            table
                .join(player(u64::from(index) + 1), seat(index), 100)
                .unwrap();
        }
        table
    }

    #[test]
    fn reservation_and_occupancy_enforce_identity_and_safe_boundary() {
        let mut table = joined_table(2);
        let start = table.begin_hand().unwrap();
        let before = start.clone();
        table.reserve(player(3), seat(2)).unwrap();
        assert_eq!(
            table.occupy(player(3), seat(2), 100),
            Err(LifecycleError::HandActive)
        );
        assert_eq!(table.active_hand.as_ref(), Some(&before));
        assert!(table.seats().seat(seat(2)).is_none());
        assert_eq!(
            table.reserve(player(1), seat(3)),
            Err(LifecycleError::IdentityAlreadyAtTable(player(1)))
        );
    }

    #[test]
    fn mid_hand_sit_out_and_leave_apply_once_after_reconciliation() {
        let mut table = joined_table(3);
        let start = table.begin_hand().unwrap();
        table.request_sit_out(player(2)).unwrap();
        table.request_leave(player(3)).unwrap();
        table.request_return(player(3)).unwrap();
        assert_eq!(table.pending().count(), 2);
        assert_eq!(table.eligible_count(), 3);
        assert_eq!(table.seats().occupied_count(), 3);

        table.complete_hand(&start.stacks).unwrap();
        assert_eq!(table.pending().count(), 0);
        assert_eq!(table.seats().occupied_count(), 2);
        assert_eq!(
            table.seats().seat(seat(1)).unwrap().table_participation,
            TableParticipation::SittingOut
        );
        assert_eq!(table.eligible_count(), 1);
        assert_eq!(table.state(), TableRunState::Paused);
        assert_eq!(
            table.complete_hand(&start.stacks),
            Err(LifecycleError::NoActiveHand)
        );
    }

    #[test]
    fn start_pause_resume_and_close_follow_eligibility_without_stranding_hand() {
        let mut table = joined_table(2);
        assert_eq!(table.state(), TableRunState::Waiting);
        let start = table.begin_hand().unwrap();
        assert_eq!(table.state(), TableRunState::Running);
        assert_eq!(table.close(), Err(LifecycleError::HandActive));
        table.request_sit_out(player(2)).unwrap();
        table.complete_hand(&start.stacks).unwrap();
        assert_eq!(table.state(), TableRunState::Paused);
        assert!(matches!(
            table.begin_hand(),
            Err(LifecycleError::TooFewEligiblePlayers(1))
        ));
        table.request_return(player(2)).unwrap();
        assert_eq!(table.state(), TableRunState::Running);
        let second = table.begin_hand().unwrap();
        assert_eq!(second.number, 2);
        assert_eq!(second.button, seat(1));
        table.complete_hand(&second.stacks).unwrap();
        table.close().unwrap();
        assert_eq!(table.state(), TableRunState::Closed);
        assert_eq!(table.request_leave(player(1)), Err(LifecycleError::Closed));
    }

    #[test]
    fn reconciliation_rejects_missing_extra_duplicate_and_non_conserving_reports() {
        let mut table = joined_table(3);
        let start = table.begin_hand().unwrap();
        assert!(matches!(
            table.complete_hand(&start.stacks[..2]),
            Err(LifecycleError::MissingHandSeat(_))
        ));
        let mut extra = start.stacks.clone();
        extra.push((seat(8), 1));
        assert_eq!(
            table.complete_hand(&extra),
            Err(LifecycleError::UnexpectedHandSeat(seat(8)))
        );
        let mut duplicate = start.stacks.clone();
        duplicate.push(start.stacks[0]);
        assert_eq!(
            table.complete_hand(&duplicate),
            Err(LifecycleError::DuplicateHandSeat(start.stacks[0].0))
        );
        let mut inflated = start.stacks.clone();
        inflated[0].1 += 1;
        assert_eq!(
            table.complete_hand(&inflated),
            Err(LifecycleError::ChipTotalMismatch {
                expected: 300,
                actual: 301
            })
        );
        table.complete_hand(&start.stacks).unwrap();
    }

    #[test]
    fn lifecycle_snapshot_drives_the_multiway_authority_and_reconciles_result() {
        let mut table = joined_table(4);
        let start = table.begin_hand().unwrap();
        let mut hand = start
            .clone()
            .into_hand(table.table_size(), Some(91))
            .unwrap();
        while !matches!(
            hand.phase,
            super::super::multiway::MultiwayPhase::Showdown
                | super::super::multiway::MultiwayPhase::HandComplete
        ) {
            let actor = hand.to_act.unwrap();
            let legal = hand.legal_actions_for(actor).unwrap();
            let action = if legal.can_check {
                Action::Check
            } else if let Some(amount) = legal.call_amount {
                Action::Call(amount)
            } else {
                Action::AllIn(legal.all_in_to)
            };
            hand.apply_command(SeatCommand::new(actor, action)).unwrap();
        }
        assert_eq!(hand.total_chips(), 400);
        table.complete_hand(&final_stacks(&hand)).unwrap();
        assert!(!table.hand_active());
        assert_eq!(
            table
                .seats()
                .occupied()
                .map(|(_, seat)| seat.stack)
                .sum::<u32>(),
            400
        );
    }

    #[test]
    fn connection_changes_never_change_table_or_hand_participation() {
        let mut table = joined_table(2);
        let before = table.seats().seat(seat(0)).unwrap().clone();
        table
            .set_connection(player(1), ConnectionState::Disconnected)
            .unwrap();
        let after = table.seats().seat(seat(0)).unwrap();
        assert_eq!(after.connection, ConnectionState::Disconnected);
        assert_eq!(after.table_participation, before.table_participation);
        assert_eq!(after.hand_participation, before.hand_participation);
        assert_eq!(table.eligible_count(), 2);
    }

    #[test]
    fn reservations_are_bounded_claims_not_participants() {
        let mut table = TableLifecycle::new(TableSize::new(9).unwrap());
        table.reserve(player(1), seat(8)).unwrap();
        assert_eq!(table.reservations().count(), 1);
        assert_eq!(table.seats().occupied_count(), 0);
        assert_eq!(table.eligible_count(), 0);
        assert!(matches!(
            table.begin_hand(),
            Err(LifecycleError::TooFewEligiblePlayers(0))
        ));
        table.cancel_reservation(player(1), seat(8)).unwrap();
        assert_eq!(table.reservations().count(), 0);
    }

    #[test]
    fn every_supported_occupancy_can_start_from_lifecycle_snapshot() {
        for occupancy in 2..=9 {
            let mut table = joined_table(occupancy);
            let start = table.begin_hand().unwrap();
            let hand = start
                .into_hand(table.table_size(), Some(occupancy.into()))
                .unwrap();
            assert_eq!(hand.occupied_seats().count(), usize::from(occupancy));
            assert_eq!(hand.total_chips(), u32::from(occupancy) * 100);
        }
    }

    #[test]
    fn active_hand_snapshot_has_unique_seats() {
        let mut table = joined_table(9);
        let start = table.begin_hand().unwrap();
        let unique = start
            .stacks
            .iter()
            .map(|(seat, _)| *seat)
            .collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), start.stacks.len());
    }

    #[test]
    fn moving_button_skips_gaps_and_switches_to_heads_up_at_the_boundary() {
        let mut table = TableLifecycle::new(TableSize::new(9).unwrap());
        for (player_id, seat_id) in [(1, 0), (2, 3), (3, 7)] {
            table.join(player(player_id), seat(seat_id), 100).unwrap();
        }
        let first = table.begin_hand().unwrap();
        assert_eq!(first.button, seat(0));
        table.complete_hand(&first.stacks).unwrap();

        let second = table.begin_hand().unwrap();
        assert_eq!(second.button, seat(3));
        table.request_leave(player(2)).unwrap();
        table.complete_hand(&second.stacks).unwrap();

        let third = table.begin_hand().unwrap();
        assert_eq!(third.button, seat(7));
        let hand = third.into_hand(table.table_size(), Some(712)).unwrap();
        assert_eq!(hand.small_blind, seat(7));
        assert_eq!(hand.big_blind, seat(0));
        assert_eq!(hand.to_act, Some(seat(7)));
    }

    #[test]
    fn posting_both_blind_debts_creates_live_and_dead_contributions_once() {
        let mut table = TableLifecycle::new(TableSize::new(9).unwrap());
        for (player_id, seat_id) in [(1, 0), (2, 3), (3, 6)] {
            table.join(player(player_id), seat(seat_id), 100).unwrap();
        }
        let first = table.begin_hand().unwrap();
        table.complete_hand(&first.stacks).unwrap();
        table
            .join_with_entry(player(4), seat(1), 100, RingEntryChoice::PostLiveBigBlind)
            .unwrap();
        table
            .record_missed_blind(player(4), MissedBlind::Small)
            .unwrap();
        assert_eq!(
            table.blind_obligation(player(4)).unwrap(),
            BlindObligation::OwesBoth
        );

        let start = table.begin_hand().unwrap();
        assert_eq!(start.forced_posts.len(), 2);
        let hand = start.into_hand(table.table_size(), Some(713)).unwrap();
        let entry = hand.seat(seat(1));
        assert_eq!(entry.street_contribution, BIG_BLIND);
        assert_eq!(entry.hand_contribution, BIG_BLIND + SMALL_BLIND);
        assert_eq!(entry.stack, 100 - BIG_BLIND - SMALL_BLIND);
        assert_eq!(hand.total_chips(), 400);
        assert_eq!(
            table.blind_obligation(player(4)).unwrap(),
            BlindObligation::Clear
        );
    }

    #[test]
    fn wait_for_big_blind_is_bounded_and_never_deals_player_early() {
        let mut table = TableLifecycle::new(TableSize::new(9).unwrap());
        for (player_id, seat_id) in [(1, 0), (2, 1), (3, 3)] {
            table.join(player(player_id), seat(seat_id), 100).unwrap();
        }
        let first = table.begin_hand().unwrap();
        table.complete_hand(&first.stacks).unwrap();
        table
            .join_with_entry(player(4), seat(2), 100, RingEntryChoice::WaitForBigBlind)
            .unwrap();

        let mut admitted = false;
        for _ in 0..3 {
            let start = table.begin_hand().unwrap();
            let includes_waiter = start.stacks.iter().any(|(seat_id, _)| *seat_id == seat(2));
            if includes_waiter {
                admitted = true;
                assert!(start.forced_posts.is_empty());
                assert_eq!(
                    table.blind_obligation(player(4)).unwrap(),
                    BlindObligation::Clear
                );
            } else {
                assert_eq!(
                    table.seats().seat(seat(2)).unwrap().table_participation,
                    TableParticipation::WaitingForBlind
                );
            }
            table.complete_hand(&start.stacks).unwrap();
            if admitted {
                break;
            }
        }
        assert!(admitted, "waiting player must reach a natural big blind");
    }

    #[test]
    fn blind_debt_survives_checkpoint_and_blocks_plain_return() {
        let mut table = joined_table(2);
        let first = table.begin_hand().unwrap();
        table.complete_hand(&first.stacks).unwrap();
        table.request_sit_out(player(1)).unwrap();
        table
            .record_missed_blind(player(1), MissedBlind::Small)
            .unwrap();
        table
            .record_missed_blind(player(1), MissedBlind::Big)
            .unwrap();
        table
            .record_missed_blind(player(1), MissedBlind::Big)
            .unwrap();

        let checkpoint = table.between_hand_checkpoint().unwrap();
        let mut restored = TableLifecycle::restore_between_hand(&checkpoint).unwrap();
        assert_eq!(
            restored.blind_obligation(player(1)).unwrap(),
            BlindObligation::OwesBoth
        );
        restored.request_return(player(1)).unwrap();
        assert_eq!(
            restored.seats().seat(seat(0)).unwrap().table_participation,
            TableParticipation::WaitingForBlind
        );
        assert_eq!(restored.eligible_count(), 1);

        restored
            .request_return_with_entry(player(1), RingEntryChoice::PostLiveBigBlind)
            .unwrap();
        let next = restored.begin_hand().unwrap();
        assert_eq!(next.forced_posts.len(), 1);
        assert_eq!(next.forced_posts[0].seat, seat(0));
        assert_eq!(next.forced_posts[0].amount, SMALL_BLIND);
        assert!(!next.forced_posts[0].live);
        let hand = next.into_hand(restored.table_size(), Some(714)).unwrap();
        assert_eq!(hand.seat(seat(0)).street_contribution, BIG_BLIND);
        assert_eq!(
            hand.seat(seat(0)).hand_contribution,
            BIG_BLIND + SMALL_BLIND
        );
        assert_eq!(hand.total_chips(), 200);
    }
}
