//! Trusted, training-only chance plans.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::game::deck::{Card, Deck, ShuffleSource};
use crate::game::seat::{SeatId, TableSize};

pub const DEAL_PLAN_VERSION: u16 = 1;
pub const WEIGHTED_RANGE_VERSION: u16 = 1;
pub const MAX_RANGE_NAME_BYTES: usize = 64;
pub const MAX_WEIGHTED_COMBOS: usize = 1_326;

/// A complete authoritative deal order for one training hand.
///
/// The first cards follow the engine's ordinary dealing order. The plan stays
/// inside the trusted arena and is never part of a policy observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DealPlanV1 {
    pub version: u16,
    pub cards: Vec<Card>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedHoleCombo {
    pub cards: [Card; 2],
    pub weight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedRangeV1 {
    pub version: u16,
    pub name: String,
    pub combos: Vec<WeightedHoleCombo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DealPlanError {
    UnsupportedVersion(u16),
    WrongCardCount(usize),
    DuplicateCard(Card),
    InvalidOccupancy(usize),
    SeatOutsideTable(SeatId),
    DuplicateSeat(SeatId),
    ButtonNotOccupied(SeatId),
    HoleAssignmentMismatch,
    InvalidRunoutSeatCount(usize),
    InvalidRangeName,
    InvalidRangeSize(usize),
    InvalidComboWeight,
    WeightOverflow,
    NoAvailableCombo,
}

impl Display for DealPlanError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => {
                write!(formatter, "deal schema version {version} is unsupported")
            }
            Self::WrongCardCount(count) => {
                write!(
                    formatter,
                    "a complete deal order requires 52 cards, found {count}"
                )
            }
            Self::DuplicateCard(card) => write!(formatter, "deal contains duplicate card {card}"),
            Self::InvalidOccupancy(count) => {
                write!(
                    formatter,
                    "training deals require 2 to 9 occupied seats, found {count}"
                )
            }
            Self::SeatOutsideTable(seat) => {
                write!(
                    formatter,
                    "seat {} is outside the deal's table",
                    seat.as_u8()
                )
            }
            Self::DuplicateSeat(seat) => {
                write!(formatter, "seat {} appears more than once", seat.as_u8())
            }
            Self::ButtonNotOccupied(seat) => {
                write!(formatter, "button seat {} is not occupied", seat.as_u8())
            }
            Self::HoleAssignmentMismatch => {
                write!(
                    formatter,
                    "hole-card assignments must exactly match occupied seats"
                )
            }
            Self::InvalidRunoutSeatCount(count) => write!(
                formatter,
                "public-runout branching requires 2 to 9 dealt seats, found {count}"
            ),
            Self::InvalidRangeName => write!(
                formatter,
                "range name must contain 1 to {MAX_RANGE_NAME_BYTES} printable ASCII bytes"
            ),
            Self::InvalidRangeSize(count) => write!(
                formatter,
                "weighted range must contain 1 to {MAX_WEIGHTED_COMBOS} combos, found {count}"
            ),
            Self::InvalidComboWeight => write!(formatter, "range combo weights must be positive"),
            Self::WeightOverflow => write!(formatter, "available range weights overflow u64"),
            Self::NoAvailableCombo => {
                write!(
                    formatter,
                    "no weighted range combo remains after card removal"
                )
            }
        }
    }
}

impl Error for DealPlanError {}

impl DealPlanV1 {
    pub fn new(cards: Vec<Card>) -> Result<Self, DealPlanError> {
        let plan = Self {
            version: DEAL_PLAN_VERSION,
            cards,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Reproduces the current review-seed shuffle exactly.
    pub fn seeded(seed: u64) -> Self {
        let mut deck = Deck::new();
        ShuffleSource::deterministic_for_review(seed).shuffle(&mut deck);
        Self {
            version: DEAL_PLAN_VERSION,
            cards: deck.deal_n(52),
        }
    }

    /// Builds a complete plan from exact private assignments and a public board.
    /// Remaining cards retain a deterministic, independently seeded filler order.
    pub fn from_assignments(
        table_size: TableSize,
        button: SeatId,
        occupied: &[SeatId],
        hole_cards: &BTreeMap<SeatId, [Card; 2]>,
        board: [Card; 5],
        filler_seed: u64,
    ) -> Result<Self, DealPlanError> {
        let occupied = validate_occupied(table_size, button, occupied)?;
        let assigned = hole_cards.keys().copied().collect::<BTreeSet<_>>();
        if assigned != occupied {
            return Err(DealPlanError::HoleAssignmentMismatch);
        }

        let clockwise = (1..=table_size.get())
            .map(|offset| {
                SeatId::new((button.as_u8() + offset) % table_size.get())
                    .expect("wrapped table seat is valid")
            })
            .filter(|seat| occupied.contains(seat))
            .collect::<Vec<_>>();
        let mut prefix = Vec::with_capacity(occupied.len() * 2 + board.len());
        for seat in &clockwise {
            prefix.push(hole_cards[seat][0]);
        }
        for seat in &clockwise {
            prefix.push(hole_cards[seat][1]);
        }
        prefix.extend(board);
        complete_from_prefix(prefix, &Self::seeded(filler_seed).cards)
    }

    /// Replaces the five future public cards while retaining the private deal.
    pub fn branch_public_runout(
        &self,
        dealt_seat_count: usize,
        board: [Card; 5],
    ) -> Result<Self, DealPlanError> {
        self.validate()?;
        if !(2..=9).contains(&dealt_seat_count) {
            return Err(DealPlanError::InvalidRunoutSeatCount(dealt_seat_count));
        }
        let private_count = dealt_seat_count * 2;
        let mut prefix = self.cards[..private_count].to_vec();
        prefix.extend(board);
        complete_from_prefix(prefix, &self.cards)
    }

    pub fn validate(&self) -> Result<(), DealPlanError> {
        if self.version != DEAL_PLAN_VERSION {
            return Err(DealPlanError::UnsupportedVersion(self.version));
        }
        if self.cards.len() != 52 {
            return Err(DealPlanError::WrongCardCount(self.cards.len()));
        }
        ensure_unique(&self.cards)
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub(crate) fn into_deck(self) -> Result<Deck, DealPlanError> {
        self.validate()?;
        Ok(Deck::from_ordered_cards_for_training(self.cards))
    }
}

impl WeightedRangeV1 {
    pub fn new(
        name: impl Into<String>,
        combos: Vec<WeightedHoleCombo>,
    ) -> Result<Self, DealPlanError> {
        let range = Self {
            version: WEIGHTED_RANGE_VERSION,
            name: name.into(),
            combos,
        };
        range.validate()?;
        Ok(range)
    }

    pub fn validate(&self) -> Result<(), DealPlanError> {
        if self.version != WEIGHTED_RANGE_VERSION {
            return Err(DealPlanError::UnsupportedVersion(self.version));
        }
        if self.name.is_empty()
            || self.name.len() > MAX_RANGE_NAME_BYTES
            || !self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        {
            return Err(DealPlanError::InvalidRangeName);
        }
        if self.combos.is_empty() || self.combos.len() > MAX_WEIGHTED_COMBOS {
            return Err(DealPlanError::InvalidRangeSize(self.combos.len()));
        }
        for combo in &self.combos {
            if combo.weight == 0 {
                return Err(DealPlanError::InvalidComboWeight);
            }
            ensure_unique(&combo.cards)?;
        }
        Ok(())
    }

    /// Samples after removing known cards. The caller supplies a policy- or
    /// scenario-specific RNG; this never consumes the deck shuffle RNG.
    pub fn sample_available<R: Rng + ?Sized>(
        &self,
        blocked: &HashSet<Card>,
        rng: &mut R,
    ) -> Result<[Card; 2], DealPlanError> {
        self.validate()?;
        let available = self
            .combos
            .iter()
            .filter(|combo| combo.cards.iter().all(|card| !blocked.contains(card)))
            .collect::<Vec<_>>();
        let total = available.iter().try_fold(0u64, |sum, combo| {
            sum.checked_add(u64::from(combo.weight))
                .ok_or(DealPlanError::WeightOverflow)
        })?;
        if total == 0 {
            return Err(DealPlanError::NoAvailableCombo);
        }
        let mut draw = rng.gen_range(0..total);
        for combo in available {
            let weight = u64::from(combo.weight);
            if draw < weight {
                return Ok(combo.cards);
            }
            draw -= weight;
        }
        unreachable!("a bounded weighted draw selects one available combo")
    }
}

fn validate_occupied(
    table_size: TableSize,
    button: SeatId,
    occupied: &[SeatId],
) -> Result<BTreeSet<SeatId>, DealPlanError> {
    if !(2..=usize::from(table_size.get())).contains(&occupied.len()) {
        return Err(DealPlanError::InvalidOccupancy(occupied.len()));
    }
    let mut seats = BTreeSet::new();
    for &seat in occupied {
        if !table_size.contains(seat) {
            return Err(DealPlanError::SeatOutsideTable(seat));
        }
        if !seats.insert(seat) {
            return Err(DealPlanError::DuplicateSeat(seat));
        }
    }
    if !seats.contains(&button) {
        return Err(DealPlanError::ButtonNotOccupied(button));
    }
    Ok(seats)
}

fn complete_from_prefix(prefix: Vec<Card>, filler: &[Card]) -> Result<DealPlanV1, DealPlanError> {
    ensure_unique(&prefix)?;
    let used = prefix.iter().copied().collect::<HashSet<_>>();
    let mut cards = prefix;
    cards.extend(filler.iter().copied().filter(|card| !used.contains(card)));
    DealPlanV1::new(cards)
}

fn ensure_unique(cards: &[Card]) -> Result<(), DealPlanError> {
    let mut seen = HashSet::with_capacity(cards.len());
    for &card in cards {
        if !seen.insert(card) {
            return Err(DealPlanError::DuplicateCard(card));
        }
    }
    Ok(())
}
