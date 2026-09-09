use std::fmt;

use serde::{Deserialize, Serialize};

use super::deck::Card;
use super::seat::{PlayerId, SeatId, TableSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TableParticipation {
    Active,
    WaitingForBlind,
    SittingOut,
    Leaving,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlindObligation {
    #[default]
    Clear,
    OwesSmallBlind,
    OwesBigBlind,
    OwesBoth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandParticipation {
    NotDealt,
    Live,
    Folded,
    AllIn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatState {
    player_id: PlayerId,
    pub hole_cards: Vec<Card>,
    pub stack: u32,
    pub street_bet: u32,
    pub connection: ConnectionState,
    pub table_participation: TableParticipation,
    pub blind_obligation: BlindObligation,
    pub hand_participation: HandParticipation,
}

impl SeatState {
    pub fn active(player_id: PlayerId, stack: u32) -> Self {
        Self {
            player_id,
            hole_cards: Vec::new(),
            stack,
            street_bet: 0,
            connection: ConnectionState::Connected,
            table_participation: TableParticipation::Active,
            blind_obligation: BlindObligation::Clear,
            hand_participation: HandParticipation::NotDealt,
        }
    }

    pub fn eligible_for_next_hand(&self) -> bool {
        self.stack > 0 && self.table_participation == TableParticipation::Active
    }

    pub fn player_id(&self) -> PlayerId {
        self.player_id
    }

    pub fn eligible_to_act(&self) -> bool {
        self.stack > 0 && self.hand_participation == HandParticipation::Live
    }

    pub fn eligible_for_pot(&self) -> bool {
        matches!(
            self.hand_participation,
            HandParticipation::Live | HandParticipation::AllIn
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatMapError {
    SeatOutsideTable { seat: SeatId, table_size: TableSize },
    SeatOccupied(SeatId),
    SeatVacant(SeatId),
    DuplicatePlayer(PlayerId),
}

impl fmt::Display for SeatMapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SeatOutsideTable { seat, table_size } => write!(
                formatter,
                "seat {} is outside table size {}",
                seat.as_u8(),
                table_size.get()
            ),
            Self::SeatOccupied(seat) => write!(formatter, "seat {} is occupied", seat.as_u8()),
            Self::SeatVacant(seat) => write!(formatter, "seat {} is vacant", seat.as_u8()),
            Self::DuplicatePlayer(player_id) => {
                write!(
                    formatter,
                    "player {} already occupies a seat",
                    player_id.value()
                )
            }
        }
    }
}

impl std::error::Error for SeatMapError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSeats {
    table_size: TableSize,
    seats: Vec<Option<SeatState>>,
}

impl TableSeats {
    pub fn new(table_size: TableSize) -> Self {
        Self {
            table_size,
            seats: vec![None; usize::from(table_size.get())],
        }
    }

    pub fn table_size(&self) -> TableSize {
        self.table_size
    }

    pub fn occupy(&mut self, seat: SeatId, state: SeatState) -> Result<(), SeatMapError> {
        let index = self.index(seat)?;
        if self.seats[index].is_some() {
            return Err(SeatMapError::SeatOccupied(seat));
        }
        if self
            .seats
            .iter()
            .flatten()
            .any(|occupant| occupant.player_id == state.player_id)
        {
            return Err(SeatMapError::DuplicatePlayer(state.player_id));
        }
        self.seats[index] = Some(state);
        Ok(())
    }

    pub fn seat(&self, seat: SeatId) -> Option<&SeatState> {
        self.seats.get(seat.index())?.as_ref()
    }

    pub fn seat_mut(&mut self, seat: SeatId) -> Option<&mut SeatState> {
        self.seats.get_mut(seat.index())?.as_mut()
    }

    pub fn vacate(&mut self, seat: SeatId) -> Result<SeatState, SeatMapError> {
        let index = self.index(seat)?;
        self.seats[index]
            .take()
            .ok_or(SeatMapError::SeatVacant(seat))
    }

    pub fn seat_for_player(&self, player_id: PlayerId) -> Option<SeatId> {
        self.occupied()
            .find_map(|(seat, state)| (state.player_id == player_id).then_some(seat))
    }

    pub fn occupied(&self) -> impl Iterator<Item = (SeatId, &SeatState)> {
        self.seats.iter().enumerate().filter_map(|(index, state)| {
            state.as_ref().map(|state| {
                let seat = SeatId::new(index as u8).expect("table indexes are valid seat IDs");
                (seat, state)
            })
        })
    }

    pub fn occupied_count(&self) -> usize {
        self.occupied().count()
    }

    pub fn next_for_hand(&self, after: SeatId) -> Option<SeatId> {
        self.table_size.next_eligible(after, |seat| {
            self.seat(seat)
                .is_some_and(SeatState::eligible_for_next_hand)
        })
    }

    pub fn next_to_act(&self, after: SeatId) -> Option<SeatId> {
        self.table_size.next_eligible(after, |seat| {
            self.seat(seat).is_some_and(SeatState::eligible_to_act)
        })
    }

    pub fn next_for_pot(&self, after: SeatId) -> Option<SeatId> {
        self.table_size.next_eligible(after, |seat| {
            self.seat(seat).is_some_and(SeatState::eligible_for_pot)
        })
    }

    pub fn positions(&self, button: SeatId) -> Result<HandPositions, PositionError> {
        if !self
            .seat(button)
            .is_some_and(SeatState::eligible_for_next_hand)
        {
            return Err(PositionError::IneligibleButton(button));
        }

        let eligible_count = self
            .occupied()
            .filter(|(_, state)| state.eligible_for_next_hand())
            .count();
        if eligible_count < 2 {
            return Err(PositionError::TooFewEligiblePlayers(eligible_count));
        }

        if eligible_count == 2 {
            let big_blind = self
                .next_for_hand(button)
                .expect("two eligible seats guarantee a big blind");
            return Ok(HandPositions {
                button,
                small_blind: button,
                big_blind,
                first_preflop: button,
                first_postflop: big_blind,
            });
        }

        let small_blind = self
            .next_for_hand(button)
            .expect("three eligible seats guarantee a small blind");
        let big_blind = self
            .next_for_hand(small_blind)
            .expect("three eligible seats guarantee a big blind");
        let first_preflop = self
            .next_for_hand(big_blind)
            .expect("three eligible seats guarantee preflop action");
        let first_postflop = self
            .next_for_hand(button)
            .expect("three eligible seats guarantee postflop action");

        Ok(HandPositions {
            button,
            small_blind,
            big_blind,
            first_preflop,
            first_postflop,
        })
    }

    fn index(&self, seat: SeatId) -> Result<usize, SeatMapError> {
        let index = seat.index();
        if index >= self.seats.len() {
            return Err(SeatMapError::SeatOutsideTable {
                seat,
                table_size: self.table_size,
            });
        }
        Ok(index)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandPositions {
    pub button: SeatId,
    pub small_blind: SeatId,
    pub big_blind: SeatId,
    pub first_preflop: SeatId,
    pub first_postflop: SeatId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionError {
    IneligibleButton(SeatId),
    TooFewEligiblePlayers(usize),
}

impl fmt::Display for PositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IneligibleButton(seat) => {
                write!(
                    formatter,
                    "button seat {} is not hand-eligible",
                    seat.as_u8()
                )
            }
            Self::TooFewEligiblePlayers(count) => {
                write!(
                    formatter,
                    "at least two hand-eligible players required, found {count}"
                )
            }
        }
    }
}

impl std::error::Error for PositionError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(value: u8) -> SeatId {
        SeatId::new(value).unwrap()
    }

    fn player(value: u64) -> PlayerId {
        PlayerId::new(value)
    }

    fn table(size: u8, occupied: &[u8]) -> TableSeats {
        let mut seats = TableSeats::new(TableSize::new(size).unwrap());
        for &seat_index in occupied {
            seats
                .occupy(
                    seat(seat_index),
                    SeatState::active(player(u64::from(seat_index) + 1), 200),
                )
                .unwrap();
        }
        seats
    }

    #[test]
    fn supports_every_table_size_and_rejects_duplicate_players() {
        for size in 2..=9 {
            let occupied: Vec<u8> = (0..size).collect();
            let mut seats = table(size, &occupied);
            assert_eq!(seats.occupied_count(), usize::from(size));

            let duplicate = SeatState::active(player(1), 200);
            let result = seats.occupy(seat(0), duplicate.clone());
            assert_eq!(result, Err(SeatMapError::SeatOccupied(seat(0))));

            if size < 9 {
                let outside = seat(size);
                assert_eq!(
                    seats.occupy(outside, duplicate),
                    Err(SeatMapError::SeatOutsideTable {
                        seat: outside,
                        table_size: TableSize::new(size).unwrap(),
                    })
                );
            }
        }

        let mut seats = table(3, &[0]);
        assert_eq!(
            seats.occupy(seat(1), SeatState::active(player(1), 100)),
            Err(SeatMapError::DuplicatePlayer(player(1)))
        );
    }

    #[test]
    fn lifecycle_dimensions_drive_independent_eligibility() {
        let mut state = SeatState::active(player(1), 100);
        assert!(state.eligible_for_next_hand());
        assert!(!state.eligible_to_act());
        assert!(!state.eligible_for_pot());

        state.hand_participation = HandParticipation::Live;
        state.connection = ConnectionState::Disconnected;
        state.table_participation = TableParticipation::SittingOut;
        assert!(!state.eligible_for_next_hand());
        assert!(state.eligible_to_act());
        assert!(state.eligible_for_pot());

        state.hand_participation = HandParticipation::AllIn;
        state.stack = 0;
        assert!(!state.eligible_to_act());
        assert!(state.eligible_for_pot());

        state.hand_participation = HandParticipation::Folded;
        assert!(!state.eligible_for_pot());
    }

    #[test]
    fn heads_up_positions_use_button_as_small_blind_and_first_preflop() {
        let seats = table(9, &[1, 8]);
        assert_eq!(
            seats.positions(seat(8)).unwrap(),
            HandPositions {
                button: seat(8),
                small_blind: seat(8),
                big_blind: seat(1),
                first_preflop: seat(8),
                first_postflop: seat(1),
            }
        );
    }

    #[test]
    fn three_handed_positions_skip_empty_physical_seats() {
        let seats = table(9, &[0, 3, 7]);
        assert_eq!(
            seats.positions(seat(0)).unwrap(),
            HandPositions {
                button: seat(0),
                small_blind: seat(3),
                big_blind: seat(7),
                first_preflop: seat(0),
                first_postflop: seat(3),
            }
        );
    }

    #[test]
    fn nine_handed_positions_wrap_clockwise() {
        let seats = table(9, &(0..9).collect::<Vec<_>>());
        let positions = seats.positions(seat(8)).unwrap();
        assert_eq!(positions.small_blind, seat(0));
        assert_eq!(positions.big_blind, seat(1));
        assert_eq!(positions.first_preflop, seat(2));
        assert_eq!(positions.first_postflop, seat(0));
    }

    #[test]
    fn positions_are_defined_at_every_multiway_occupancy() {
        for size in 3..=9 {
            let occupied: Vec<u8> = (0..size).collect();
            let seats = table(size, &occupied);
            let positions = seats.positions(seat(size - 1)).unwrap();

            assert_eq!(positions.small_blind, seat(0), "table size {size}");
            assert_eq!(positions.big_blind, seat(1), "table size {size}");
            assert_eq!(positions.first_preflop, seat(2), "table size {size}");
            assert_eq!(positions.first_postflop, seat(0), "table size {size}");
        }
    }

    #[test]
    fn action_traversal_skips_ineligible_states_but_not_disconnects() {
        let mut seats = table(9, &[0, 2, 4, 6, 8]);
        seats.seat_mut(seat(0)).unwrap().hand_participation = HandParticipation::Live;
        seats.seat_mut(seat(2)).unwrap().hand_participation = HandParticipation::Folded;
        seats.seat_mut(seat(4)).unwrap().hand_participation = HandParticipation::AllIn;
        seats.seat_mut(seat(4)).unwrap().stack = 0;
        seats.seat_mut(seat(6)).unwrap().hand_participation = HandParticipation::NotDealt;
        seats.seat_mut(seat(8)).unwrap().hand_participation = HandParticipation::Live;
        seats.seat_mut(seat(8)).unwrap().connection = ConnectionState::Disconnected;

        assert_eq!(seats.next_to_act(seat(0)), Some(seat(8)));
        assert_eq!(seats.next_to_act(seat(8)), Some(seat(0)));
        assert_eq!(seats.next_for_pot(seat(2)), Some(seat(4)));
    }

    #[test]
    fn next_hand_traversal_skips_sitting_out_leaving_and_zero_stack_seats() {
        let mut seats = table(9, &[0, 2, 4, 6, 8]);
        seats.seat_mut(seat(2)).unwrap().table_participation = TableParticipation::SittingOut;
        seats.seat_mut(seat(4)).unwrap().table_participation = TableParticipation::Leaving;
        seats.seat_mut(seat(6)).unwrap().stack = 0;

        assert_eq!(seats.next_for_hand(seat(0)), Some(seat(8)));
        assert_eq!(seats.next_for_hand(seat(8)), Some(seat(0)));
    }

    #[test]
    fn three_to_two_transition_recalculates_heads_up_positions() {
        let mut seats = table(9, &[1, 4, 8]);
        let three_handed = seats.positions(seat(1)).unwrap();
        assert_eq!(three_handed.small_blind, seat(4));
        assert_eq!(three_handed.big_blind, seat(8));

        seats.seat_mut(seat(4)).unwrap().table_participation = TableParticipation::Leaving;
        let heads_up = seats.positions(seat(1)).unwrap();
        assert_eq!(heads_up.small_blind, seat(1));
        assert_eq!(heads_up.big_blind, seat(8));
        assert_eq!(heads_up.first_preflop, seat(1));
        assert_eq!(heads_up.first_postflop, seat(8));
    }

    #[test]
    fn positions_reject_ineligible_button_and_short_table() {
        let mut seats = table(3, &[0, 1]);
        seats.seat_mut(seat(0)).unwrap().table_participation = TableParticipation::SittingOut;
        assert_eq!(
            seats.positions(seat(0)),
            Err(PositionError::IneligibleButton(seat(0)))
        );
        assert_eq!(
            seats.positions(seat(1)),
            Err(PositionError::TooFewEligiblePlayers(1))
        );
    }
}
