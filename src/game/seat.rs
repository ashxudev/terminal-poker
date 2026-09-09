use std::fmt;

use serde::{Deserialize, Serialize};

pub const MIN_TABLE_SEATS: u8 = 2;
pub const MAX_TABLE_SEATS: u8 = 9;

/// Stable identity for a player, independent of their current seat or controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PlayerId(u64);

impl PlayerId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Stable zero-based physical position at a poker table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct SeatId(u8);

impl SeatId {
    pub const fn new(index: u8) -> Result<Self, InvalidSeatId> {
        if index < MAX_TABLE_SEATS {
            Ok(Self(index))
        } else {
            Err(InvalidSeatId(index))
        }
    }

    pub const fn index(self) -> usize {
        self.0 as usize
    }

    pub const fn as_u8(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for SeatId {
    type Error = InvalidSeatId;

    fn try_from(index: u8) -> Result<Self, Self::Error> {
        Self::new(index)
    }
}

impl From<SeatId> for u8 {
    fn from(seat: SeatId) -> Self {
        seat.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSeatId(u8);

impl InvalidSeatId {
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl fmt::Display for InvalidSeatId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "seat index {} is outside the supported range 0..{}",
            self.0, MAX_TABLE_SEATS
        )
    }
}

impl std::error::Error for InvalidSeatId {}

/// Validated table capacity for Hold'em games supported by this project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct TableSize(u8);

impl TableSize {
    pub fn new(seats: u8) -> Result<Self, InvalidTableSize> {
        Self::try_from(seats)
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn contains(self, seat: SeatId) -> bool {
        seat.0 < self.0
    }

    pub fn seats(self) -> impl ExactSizeIterator<Item = SeatId> {
        (0..self.0).map(SeatId)
    }

    /// Finds the first eligible seat clockwise after `start`.
    ///
    /// The starting seat is deliberately excluded. If no other seat matches,
    /// this returns `None`, even when `start` itself is eligible.
    pub fn next_eligible<F>(self, start: SeatId, mut eligible: F) -> Option<SeatId>
    where
        F: FnMut(SeatId) -> bool,
    {
        if !self.contains(start) {
            return None;
        }

        for offset in 1..self.0 {
            let candidate = SeatId((start.0 + offset) % self.0);
            if eligible(candidate) {
                return Some(candidate);
            }
        }

        None
    }
}

impl TryFrom<u8> for TableSize {
    type Error = InvalidTableSize;

    fn try_from(seats: u8) -> Result<Self, Self::Error> {
        if (MIN_TABLE_SEATS..=MAX_TABLE_SEATS).contains(&seats) {
            Ok(Self(seats))
        } else {
            Err(InvalidTableSize(seats))
        }
    }
}

impl From<TableSize> for u8 {
    fn from(size: TableSize) -> Self {
        size.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTableSize(u8);

impl InvalidTableSize {
    pub const fn value(self) -> u8 {
        self.0
    }
}

impl fmt::Display for InvalidTableSize {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "table size {} is outside the supported range {}..={}",
            self.0, MIN_TABLE_SEATS, MAX_TABLE_SEATS
        )
    }
}

impl std::error::Error for InvalidTableSize {}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).expect("test seat must be valid")
    }

    fn eligible_from<const N: usize>(states: [bool; N]) -> impl FnMut(SeatId) -> bool {
        move |candidate| states[candidate.index()]
    }

    #[test]
    fn identifiers_are_neutral_and_stable() {
        let player = PlayerId::new(42);
        let table_seat = seat(7);

        assert_eq!(player.value(), 42);
        assert_eq!(table_seat.index(), 7);
        assert_eq!(table_seat.as_u8(), 7);
    }

    #[test]
    fn validates_supported_table_and_seat_ranges() {
        assert_eq!(TableSize::new(2).expect("heads-up is valid").get(), 2);
        assert_eq!(TableSize::new(9).expect("full ring is valid").get(), 9);
        assert_eq!(
            TableSize::new(1).expect_err("one seat is invalid").value(),
            1
        );
        assert_eq!(
            TableSize::new(10)
                .expect_err("ten seats are unsupported")
                .value(),
            10
        );
        assert_eq!(
            SeatId::new(9)
                .expect_err("seat indexes end at eight")
                .value(),
            9
        );
    }

    #[test]
    fn serialized_values_cannot_bypass_validation() {
        assert!(serde_json::from_str::<SeatId>("9").is_err());
        assert!(serde_json::from_str::<TableSize>("1").is_err());
        assert_eq!(
            serde_json::to_string(&seat(8)).expect("serialize seat"),
            "8"
        );
    }

    #[test]
    fn traverses_three_seats_and_skips_an_ineligible_seat() {
        let table = TableSize::new(3).expect("valid table");
        let eligible = [true, false, true];

        assert_eq!(
            table.next_eligible(seat(0), eligible_from(eligible)),
            Some(seat(2))
        );
        assert_eq!(
            table.next_eligible(seat(2), eligible_from(eligible)),
            Some(seat(0))
        );
    }

    #[test]
    fn traverses_heads_up_to_the_other_seat() {
        let table = TableSize::new(2).expect("heads-up table");
        let eligible = [true, true];

        assert_eq!(
            table.next_eligible(seat(0), eligible_from(eligible)),
            Some(seat(1))
        );
        assert_eq!(
            table.next_eligible(seat(1), eligible_from(eligible)),
            Some(seat(0))
        );
    }

    #[test]
    fn traverses_six_seats_with_clockwise_wraparound() {
        let table = TableSize::new(6).expect("valid table");
        let eligible = [false, true, false, false, true, false];

        assert_eq!(
            table.next_eligible(seat(4), eligible_from(eligible)),
            Some(seat(1))
        );
        assert_eq!(
            table.next_eligible(seat(1), eligible_from(eligible)),
            Some(seat(4))
        );
    }

    #[test]
    fn traverses_nine_seats_and_skips_multiple_states() {
        let table = TableSize::new(9).expect("valid table");
        let eligible = [true, false, false, true, false, false, false, false, true];

        assert_eq!(
            table.next_eligible(seat(3), eligible_from(eligible)),
            Some(seat(8))
        );
        assert_eq!(
            table.next_eligible(seat(8), eligible_from(eligible)),
            Some(seat(0))
        );
    }

    #[test]
    fn excludes_the_starting_seat_when_it_is_the_only_match() {
        let table = TableSize::new(3).expect("valid table");
        let eligible = [false, true, false];

        assert_eq!(table.next_eligible(seat(1), eligible_from(eligible)), None);
    }

    #[test]
    fn rejects_a_starting_seat_outside_the_table_capacity() {
        let table = TableSize::new(3).expect("valid table");

        assert_eq!(table.next_eligible(seat(7), |_| true), None);
    }

    #[test]
    fn iterates_every_physical_seat_in_order() {
        let table = TableSize::new(6).expect("valid table");
        let indexes: Vec<_> = table.seats().map(SeatId::index).collect();

        assert_eq!(indexes, vec![0, 1, 2, 3, 4, 5]);
    }
}
