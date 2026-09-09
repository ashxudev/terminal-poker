//! Deterministic two-to-nine-seat no-limit Hold'em hand authority.
//!
//! The existing [`super::state::GameState`] remains the offline heads-up
//! adapter. This module is the neutral multiway domain path: all controllers
//! submit [`SeatCommand`], rejected commands do not mutate state, betting
//! completion is seat-driven, and showdown resolves independently eligible
//! main and side pots.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use super::actions::Action;
use super::command::{ActionError, CommandError, SeatCommand};
use super::deck::{Card, Deck, ShuffleSource};
use super::hand::{evaluate_hand, HandEvaluation};
use super::seat::{SeatId, TableSize};
use super::state::{BIG_BLIND, SMALL_BLIND};
use super::table::HandParticipation;

mod showdown;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiwayPhase {
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
    HandComplete,
}

impl MultiwayPhase {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Preflop => "Preflop",
            Self::Flop => "Flop",
            Self::Turn => "Turn",
            Self::River => "River",
            Self::Showdown => "Showdown",
            Self::HandComplete => "Complete",
        }
    }

    fn accepts_actions(self) -> bool {
        matches!(self, Self::Preflop | Self::Flop | Self::Turn | Self::River)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiwaySeatState {
    pub hole_cards: Vec<Card>,
    pub stack: u32,
    pub street_contribution: u32,
    pub hand_contribution: u32,
    pub participation: HandParticipation,
    /// Wager level this seat most recently acted against on this street.
    /// `None` means the seat still owes an initial response.
    pub last_action_wager: Option<u32>,
}

impl MultiwaySeatState {
    fn new(stack: u32) -> Self {
        Self {
            hole_cards: Vec::with_capacity(2),
            stack,
            street_contribution: 0,
            hand_contribution: 0,
            participation: HandParticipation::Live,
            last_action_wager: None,
        }
    }

    pub fn can_act(&self) -> bool {
        self.stack > 0 && self.participation == HandParticipation::Live
    }

    pub fn eligible_for_pot(&self) -> bool {
        matches!(
            self.participation,
            HandParticipation::Live | HandParticipation::AllIn
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiwayLegalActions {
    pub can_fold: bool,
    pub can_check: bool,
    /// Exact number of chips added by a non-all-in call.
    pub call_amount: Option<u32>,
    /// Minimum total street contribution for a non-all-in bet.
    pub min_bet_to: Option<u32>,
    /// Minimum total street contribution for a non-all-in raise.
    pub min_raise_to: Option<u32>,
    /// Maximum total street contribution, including an all-in.
    pub all_in_to: u32,
    pub raise_reopened: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiwayActionRecord {
    pub sequence: u32,
    pub phase: MultiwayPhase,
    pub seat: SeatId,
    pub action: Action,
    pub wager_after: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForcedPost {
    pub seat: SeatId,
    pub amount: u32,
    pub live: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlindValues {
    pub small_blind: u32,
    pub big_blind: u32,
    pub ante: u32,
}

impl Default for BlindValues {
    fn default() -> Self {
        Self {
            small_blind: SMALL_BLIND,
            big_blind: BIG_BLIND,
            ante: 0,
        }
    }
}

impl BlindValues {
    pub const fn new(small_blind: u32, big_blind: u32, ante: u32) -> Option<Self> {
        if small_blind > 0 && small_blind < big_blind && ante <= big_blind {
            Some(Self {
                small_blind,
                big_blind,
                ante,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pot {
    pub amount: u32,
    pub eligible: Vec<SeatId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contribution {
    pub seat: SeatId,
    pub amount: u32,
    pub eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReturnedExcess {
    pub seat: SeatId,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PotBuild {
    pub pots: Vec<Pot>,
    pub returned: Vec<ReturnedExcess>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatPayout {
    pub seat: SeatId,
    pub amount: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PotAward {
    pub pot_index: usize,
    pub amount: u32,
    pub eligible: Vec<SeatId>,
    pub winners: Vec<SeatId>,
    pub payouts: Vec<SeatPayout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevealedHand {
    pub seat: SeatId,
    pub description: String,
}

/// Public progress only: no future board cards, evaluations, or awards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowdownProgress {
    pub all_in: bool,
    pub order: Vec<SeatId>,
    pub cursor: usize,
    pub mucked: Vec<SeatId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MultiwayConfigError {
    TooFewPlayers(usize),
    SeatOutsideTable(SeatId),
    DuplicateSeat(SeatId),
    EmptyStack(SeatId),
    ButtonNotOccupied(SeatId),
    ChipTotalOverflow,
    InvalidBlinds,
}

impl fmt::Display for MultiwayConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooFewPlayers(count) => {
                write!(
                    formatter,
                    "at least two occupied seats are required, found {count}"
                )
            }
            Self::SeatOutsideTable(seat) => {
                write!(
                    formatter,
                    "seat {} is outside the configured table",
                    seat.as_u8()
                )
            }
            Self::DuplicateSeat(seat) => write!(formatter, "seat {} is duplicated", seat.as_u8()),
            Self::EmptyStack(seat) => write!(formatter, "seat {} has no chips", seat.as_u8()),
            Self::ButtonNotOccupied(seat) => {
                write!(formatter, "button seat {} is not occupied", seat.as_u8())
            }
            Self::ChipTotalOverflow => write!(formatter, "configured chip total exceeds u32"),
            Self::InvalidBlinds => formatter.write_str(
                "small blind must be positive and below big blind; ante cannot exceed big blind",
            ),
        }
    }
}

impl std::error::Error for MultiwayConfigError {}

#[derive(Debug, Clone)]
pub struct MultiwayHand {
    pub phase: MultiwayPhase,
    pub table_size: TableSize,
    pub seats: Vec<Option<MultiwaySeatState>>,
    pub board: Vec<Card>,
    pub button: SeatId,
    pub small_blind: SeatId,
    pub big_blind: SeatId,
    pub blind_values: BlindValues,
    pub to_act: Option<SeatId>,
    pub current_wager: u32,
    pub last_full_raise_size: u32,
    pub action_history: Vec<MultiwayActionRecord>,
    pub pots: Vec<Pot>,
    pub returned_excess: Vec<ReturnedExcess>,
    pub awards: Vec<PotAward>,
    pub revealed_hands: Vec<RevealedHand>,
    pub settled_contributions: Vec<Contribution>,
    pub showdown_progress: Option<ShowdownProgress>,
    pub mucked_hands: Vec<SeatId>,
    pub always_show: BTreeSet<SeatId>,
    paced_showdown: bool,
    showdown_evaluations: BTreeMap<SeatId, HandEvaluation>,
    initial_total: u32,
    deck: Deck,
}

impl MultiwayHand {
    pub fn new(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
    ) -> Result<Self, MultiwayConfigError> {
        Self::with_shuffle_source(
            table_size,
            button,
            stacks,
            &[],
            BlindValues::default(),
            ShuffleSource::production(),
        )
    }

    pub fn new_seeded_for_review(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        seed: u64,
    ) -> Result<Self, MultiwayConfigError> {
        Self::with_shuffle_source(
            table_size,
            button,
            stacks,
            &[],
            BlindValues::default(),
            ShuffleSource::deterministic_for_review(seed),
        )
    }

    pub fn new_with_forced_posts(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        forced_posts: &[ForcedPost],
    ) -> Result<Self, MultiwayConfigError> {
        Self::with_shuffle_source(
            table_size,
            button,
            stacks,
            forced_posts,
            BlindValues::default(),
            ShuffleSource::production(),
        )
    }

    pub fn new_seeded_with_forced_posts(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        forced_posts: &[ForcedPost],
        seed: u64,
    ) -> Result<Self, MultiwayConfigError> {
        Self::with_shuffle_source(
            table_size,
            button,
            stacks,
            forced_posts,
            BlindValues::default(),
            ShuffleSource::deterministic_for_review(seed),
        )
    }

    pub fn new_with_blinds(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        forced_posts: &[ForcedPost],
        blind_values: BlindValues,
    ) -> Result<Self, MultiwayConfigError> {
        Self::with_shuffle_source(
            table_size,
            button,
            stacks,
            forced_posts,
            blind_values,
            ShuffleSource::production(),
        )
    }

    pub fn new_seeded_with_blinds(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        forced_posts: &[ForcedPost],
        blind_values: BlindValues,
        seed: u64,
    ) -> Result<Self, MultiwayConfigError> {
        Self::with_shuffle_source(
            table_size,
            button,
            stacks,
            forced_posts,
            blind_values,
            ShuffleSource::deterministic_for_review(seed),
        )
    }

    fn with_shuffle_source(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        forced_posts: &[ForcedPost],
        blind_values: BlindValues,
        mut shuffle_source: ShuffleSource,
    ) -> Result<Self, MultiwayConfigError> {
        let mut deck = Deck::new();
        shuffle_source.shuffle(&mut deck);
        Self::with_prepared_deck(table_size, button, stacks, forced_posts, blind_values, deck)
    }

    /// Training-only constructor for an explicitly validated chance outcome.
    ///
    /// Policies never receive the prepared deck. All dealing and subsequent
    /// state transitions remain owned by `MultiwayHand`.
    pub(crate) fn new_with_deck_for_training(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        forced_posts: &[ForcedPost],
        deck: Deck,
    ) -> Result<Self, MultiwayConfigError> {
        Self::with_prepared_deck(
            table_size,
            button,
            stacks,
            forced_posts,
            BlindValues::default(),
            deck,
        )
    }

    fn with_prepared_deck(
        table_size: TableSize,
        button: SeatId,
        stacks: &[(SeatId, u32)],
        forced_posts: &[ForcedPost],
        blind_values: BlindValues,
        mut deck: Deck,
    ) -> Result<Self, MultiwayConfigError> {
        if BlindValues::new(
            blind_values.small_blind,
            blind_values.big_blind,
            blind_values.ante,
        )
        .is_none()
        {
            return Err(MultiwayConfigError::InvalidBlinds);
        }
        if stacks.len() < 2 {
            return Err(MultiwayConfigError::TooFewPlayers(stacks.len()));
        }
        let mut seen = BTreeSet::new();
        let mut seats = vec![None; usize::from(table_size.get())];
        let mut initial_total = 0u32;
        for &(seat, stack) in stacks {
            if !table_size.contains(seat) {
                return Err(MultiwayConfigError::SeatOutsideTable(seat));
            }
            if !seen.insert(seat) {
                return Err(MultiwayConfigError::DuplicateSeat(seat));
            }
            if stack == 0 {
                return Err(MultiwayConfigError::EmptyStack(seat));
            }
            initial_total = initial_total
                .checked_add(stack)
                .ok_or(MultiwayConfigError::ChipTotalOverflow)?;
            seats[seat.index()] = Some(MultiwaySeatState::new(stack));
        }
        if seats.get(button.index()).and_then(Option::as_ref).is_none() {
            return Err(MultiwayConfigError::ButtonNotOccupied(button));
        }

        let occupied = clockwise_occupied(table_size, &seats, button);
        for _ in 0..2 {
            for &seat in &occupied {
                seats[seat.index()]
                    .as_mut()
                    .expect("occupied traversal contains occupied seats")
                    .hole_cards
                    .push(deck.deal().expect("a nine-seat deal fits in a deck"));
            }
        }

        let (small_blind, big_blind, first_preflop) = if stacks.len() == 2 {
            let big_blind = next_occupied(table_size, &seats, button)
                .expect("two occupied seats guarantee a big blind");
            (button, big_blind, button)
        } else {
            let small_blind = next_occupied(table_size, &seats, button)
                .expect("three occupied seats guarantee a small blind");
            let big_blind = next_occupied(table_size, &seats, small_blind)
                .expect("three occupied seats guarantee a big blind");
            let first_preflop = next_occupied(table_size, &seats, big_blind)
                .expect("three occupied seats guarantee preflop action");
            (small_blind, big_blind, first_preflop)
        };

        let mut hand = Self {
            phase: MultiwayPhase::Preflop,
            table_size,
            seats,
            board: Vec::with_capacity(5),
            button,
            small_blind,
            big_blind,
            blind_values,
            to_act: None,
            current_wager: 0,
            last_full_raise_size: blind_values.big_blind,
            action_history: Vec::new(),
            pots: Vec::new(),
            returned_excess: Vec::new(),
            awards: Vec::new(),
            revealed_hands: Vec::new(),
            showdown_progress: None,
            mucked_hands: Vec::new(),
            always_show: BTreeSet::new(),
            paced_showdown: true,
            showdown_evaluations: BTreeMap::new(),
            settled_contributions: Vec::new(),
            initial_total,
            deck,
        };
        if blind_values.ante > 0 {
            for seat in hand.occupied_seats().collect::<Vec<_>>() {
                hand.commit_dead_chips(seat, blind_values.ante);
            }
        }
        hand.commit_chips(small_blind, blind_values.small_blind);
        hand.commit_chips(big_blind, blind_values.big_blind);
        for post in forced_posts {
            if !hand.occupied_seats().any(|occupied| occupied == post.seat) {
                return Err(MultiwayConfigError::SeatOutsideTable(post.seat));
            }
            if post.live {
                hand.commit_chips(post.seat, post.amount);
            } else {
                hand.commit_dead_chips(post.seat, post.amount);
            }
        }
        hand.current_wager = hand
            .occupied_seats()
            .map(|seat| hand.seat(seat).street_contribution)
            .max()
            .unwrap_or(0);
        hand.to_act = hand.first_needing_from(first_preflop);
        hand.advance_automatically_if_needed();
        hand.paced_showdown = false;
        Ok(hand)
    }

    pub fn seat(&self, seat: SeatId) -> &MultiwaySeatState {
        self.seats
            .get(seat.index())
            .and_then(Option::as_ref)
            .expect("multiway operations require an occupied seat")
    }

    fn seat_mut(&mut self, seat: SeatId) -> &mut MultiwaySeatState {
        self.seats
            .get_mut(seat.index())
            .and_then(Option::as_mut)
            .expect("multiway operations require an occupied seat")
    }

    pub fn occupied_seats(&self) -> impl Iterator<Item = SeatId> + '_ {
        self.table_size
            .seats()
            .filter(|seat| self.seats[seat.index()].is_some())
    }

    pub fn amount_to_call(&self, seat: SeatId) -> u32 {
        self.current_wager
            .saturating_sub(self.seat(seat).street_contribution)
    }

    pub fn raise_reopened_for(&self, seat: SeatId) -> bool {
        self.seat(seat).last_action_wager.is_none_or(|prior_wager| {
            self.current_wager.saturating_sub(prior_wager) >= self.last_full_raise_size
        })
    }

    pub fn legal_actions_for(&self, seat: SeatId) -> Option<MultiwayLegalActions> {
        if !self.phase.accepts_actions() || self.to_act != Some(seat) || !self.seat(seat).can_act()
        {
            return None;
        }
        let state = self.seat(seat);
        let to_call = self.amount_to_call(seat);
        let all_in_to = state.street_contribution.saturating_add(state.stack);
        let other_live = self
            .occupied_seats()
            .any(|other| other != seat && self.seat(other).can_act());
        let raise_reopened = self.raise_reopened_for(seat);
        let maximum_non_all_in = all_in_to.saturating_sub(1);

        let min_bet_to =
            (to_call == 0 && other_live && maximum_non_all_in >= self.blind_values.big_blind)
                .then_some(self.blind_values.big_blind);
        let minimum_raise = self.current_wager.saturating_add(self.last_full_raise_size);
        let min_raise_to =
            (to_call > 0 && other_live && raise_reopened && maximum_non_all_in >= minimum_raise)
                .then_some(minimum_raise);

        Some(MultiwayLegalActions {
            can_fold: to_call > 0,
            can_check: to_call == 0,
            call_amount: (to_call > 0 && to_call < state.stack).then_some(to_call),
            min_bet_to,
            min_raise_to,
            all_in_to,
            raise_reopened,
        })
    }

    pub fn validate_command(&self, command: SeatCommand) -> Result<(), CommandError> {
        if !self.phase.accepts_actions() {
            return Err(CommandError::HandNotActive);
        }
        let state = self
            .seats
            .get(command.seat.index())
            .and_then(Option::as_ref)
            .ok_or(CommandError::SeatNotOccupied(command.seat))?;
        if !state.can_act() {
            return Err(CommandError::SeatNotEligible(command.seat));
        }
        let expected = self.to_act.ok_or(CommandError::HandNotActive)?;
        if command.seat != expected {
            return Err(CommandError::OutOfTurn {
                expected,
                actual: command.seat,
            });
        }
        self.validate_action(command.seat, command.action)
            .map_err(CommandError::IllegalAction)
    }

    pub fn apply_command(&mut self, command: SeatCommand) -> Result<(), CommandError> {
        self.validate_command(command)?;
        self.apply_validated_action(command.seat, command.action);
        Ok(())
    }

    fn validate_action(&self, seat: SeatId, action: Action) -> Result<(), ActionError> {
        let legal = self
            .legal_actions_for(seat)
            .expect("validated acting seat has legal-action metadata");
        let current = self.seat(seat).street_contribution;
        let maximum_non_all_in = legal.all_in_to.saturating_sub(1);
        match action {
            Action::Fold if legal.can_fold => Ok(()),
            Action::Fold => Err(ActionError::FoldNotAllowed),
            Action::Check if legal.can_check => Ok(()),
            Action::Check => Err(ActionError::CheckNotAllowed),
            Action::Call(actual) if legal.call_amount == Some(actual) => Ok(()),
            Action::Call(actual) if legal.call_amount.is_some() => Err(ActionError::InvalidCall {
                expected: legal.call_amount.expect("checked as present"),
                actual,
            }),
            Action::Call(_) => Err(ActionError::CallNotAllowed),
            Action::Bet(actual) if legal.min_bet_to.is_some() => {
                let minimum = legal.min_bet_to.expect("checked as present");
                if (minimum..=maximum_non_all_in).contains(&actual) {
                    Ok(())
                } else {
                    Err(ActionError::BetOutOfRange {
                        min: minimum,
                        max: maximum_non_all_in,
                        actual,
                    })
                }
            }
            Action::Bet(_) => Err(ActionError::BetNotAllowed),
            Action::Raise(_) if !legal.raise_reopened => Err(ActionError::RaiseNotReopened),
            Action::Raise(actual) if legal.min_raise_to.is_some() => {
                let minimum = legal.min_raise_to.expect("checked as present");
                if (minimum..=maximum_non_all_in).contains(&actual) {
                    Ok(())
                } else {
                    Err(ActionError::RaiseOutOfRange {
                        min: minimum,
                        max: maximum_non_all_in,
                        actual,
                    })
                }
            }
            Action::Raise(_) => Err(ActionError::RaiseNotAllowed),
            Action::AllIn(actual) if actual == legal.all_in_to && actual > current => Ok(()),
            Action::AllIn(actual) => Err(ActionError::InvalidAllIn {
                expected: legal.all_in_to,
                actual,
            }),
        }
    }

    fn apply_validated_action(&mut self, seat: SeatId, action: Action) {
        let phase = self.phase;
        let old_wager = self.current_wager;
        match action {
            Action::Fold => {
                self.seat_mut(seat).participation = HandParticipation::Folded;
            }
            Action::Check => {}
            Action::Call(amount) => {
                let target = self.seat(seat).street_contribution + amount;
                self.commit_to(seat, target);
            }
            Action::Bet(target) | Action::Raise(target) => {
                self.commit_to(seat, target);
                let increase = target.saturating_sub(old_wager);
                self.current_wager = target;
                self.last_full_raise_size = increase;
            }
            Action::AllIn(target) => {
                self.commit_to(seat, target);
                if target > old_wager {
                    let increase = target - old_wager;
                    let full_raise = if old_wager == 0 {
                        target >= self.blind_values.big_blind
                    } else {
                        increase >= self.last_full_raise_size
                    };
                    self.current_wager = target;
                    if full_raise {
                        self.last_full_raise_size = increase;
                    }
                }
                self.seat_mut(seat).participation = HandParticipation::AllIn;
            }
        }

        if self.seat(seat).participation == HandParticipation::Live {
            self.seat_mut(seat).last_action_wager = Some(self.current_wager);
        }
        self.action_history.push(MultiwayActionRecord {
            sequence: self.action_history.len() as u32 + 1,
            phase,
            seat,
            action,
            wager_after: self.current_wager,
        });

        if self.pot_eligible_count() == 1 {
            self.award_fold();
            return;
        }
        if self.betting_round_complete() {
            self.advance_street();
            return;
        }
        self.to_act = self.next_needing_after(seat);
        debug_assert!(
            self.to_act.is_some(),
            "an incomplete round has a next actor"
        );
    }

    fn commit_to(&mut self, seat: SeatId, target: u32) {
        let current = self.seat(seat).street_contribution;
        let addition = target.saturating_sub(current);
        self.commit_chips(seat, addition);
    }

    fn commit_chips(&mut self, seat: SeatId, requested: u32) {
        let state = self.seat_mut(seat);
        let actual = requested.min(state.stack);
        state.stack -= actual;
        state.street_contribution += actual;
        state.hand_contribution += actual;
        if state.stack == 0 {
            state.participation = HandParticipation::AllIn;
        }
    }

    fn commit_dead_chips(&mut self, seat: SeatId, requested: u32) {
        let state = self.seat_mut(seat);
        let actual = requested.min(state.stack);
        state.stack -= actual;
        state.hand_contribution += actual;
        if state.stack == 0 {
            state.participation = HandParticipation::AllIn;
        }
    }

    fn needs_action(&self, seat: SeatId) -> bool {
        let state = self.seat(seat);
        state.can_act()
            && (state.last_action_wager.is_none() || state.street_contribution < self.current_wager)
    }

    fn betting_round_complete(&self) -> bool {
        let live: Vec<SeatId> = self
            .occupied_seats()
            .filter(|&seat| self.seat(seat).can_act())
            .collect();
        match live.as_slice() {
            [] => true,
            [only] => self.seat(*only).street_contribution >= self.current_wager,
            _ => live.iter().all(|&seat| !self.needs_action(seat)),
        }
    }

    fn advance_automatically_if_needed(&mut self) {
        if self.phase.accepts_actions() && self.betting_round_complete() {
            self.advance_street();
        }
    }

    fn advance_street(&mut self) {
        if self.pot_eligible_count() == 1 {
            self.award_fold();
            return;
        }
        let betting_locked = self
            .occupied_seats()
            .filter(|&s| self.seat(s).can_act())
            .count()
            <= 1;
        let any_all_in = self
            .occupied_seats()
            .any(|s| self.seat(s).participation == HandParticipation::AllIn);
        if self.phase == MultiwayPhase::River || (betting_locked && any_all_in) {
            self.begin_showdown(any_all_in);
            return;
        }
        for seat in self.occupied_seats().collect::<Vec<_>>() {
            let state = self.seat_mut(seat);
            state.street_contribution = 0;
            state.last_action_wager = None;
        }
        self.current_wager = 0;
        self.last_full_raise_size = self.blind_values.big_blind;
        self.to_act = None;

        match self.phase {
            MultiwayPhase::Preflop => {
                self.board.extend(self.deck.deal_n(3));
                self.phase = MultiwayPhase::Flop;
            }
            MultiwayPhase::Flop => {
                self.board.extend(self.deck.deal_n(1));
                self.phase = MultiwayPhase::Turn;
            }
            MultiwayPhase::Turn => {
                self.board.extend(self.deck.deal_n(1));
                self.phase = MultiwayPhase::River;
            }
            MultiwayPhase::River => {
                self.resolve_showdown();
                return;
            }
            _ => return,
        }

        let first_postflop = self.next_live_after(self.button).or_else(|| {
            self.occupied_seats()
                .find(|&seat| self.seat(seat).can_act())
        });
        self.to_act = first_postflop.and_then(|seat| self.first_needing_from(seat));
        if self.betting_round_complete() {
            self.advance_street();
        }
    }

    fn award_fold(&mut self) {
        self.showdown_progress = None;
        let winner = self
            .occupied_seats()
            .find(|&seat| self.seat(seat).eligible_for_pot())
            .expect("one pot-eligible seat remains");
        let amount: u32 = self
            .occupied_seats()
            .map(|seat| self.seat(seat).hand_contribution)
            .sum();
        self.settled_contributions = self.contribution_snapshot();
        self.seat_mut(winner).stack += amount;
        self.awards = vec![PotAward {
            pot_index: 0,
            amount,
            eligible: vec![winner],
            winners: vec![winner],
            payouts: vec![SeatPayout {
                seat: winner,
                amount,
            }],
        }];
        self.clear_contributions();
        self.to_act = None;
        self.phase = MultiwayPhase::HandComplete;
    }

    fn resolve_showdown(&mut self) {
        self.settled_contributions = self.contribution_snapshot();
        let build = build_pots(&self.settled_contributions);
        self.pots = build.pots;
        self.returned_excess = build.returned;
        for returned in self.returned_excess.clone() {
            self.seat_mut(returned.seat).stack += returned.amount;
        }

        self.evaluate_showdown_once();
        let evaluations = self.showdown_evaluations.clone();
        // Direct domain review callers use the same ordered reveal policy.
        if self.revealed_hands.is_empty() {
            self.reveal_synchronously();
        }
        for shown in &mut self.revealed_hands {
            if let Some(evaluation) = evaluations.get(&shown.seat) {
                shown.description.clone_from(&evaluation.description);
            }
        }

        let pots = self.pots.clone();
        for (pot_index, pot) in pots.iter().enumerate() {
            let best = pot
                .eligible
                .iter()
                .filter_map(|seat| evaluations.get(seat))
                .max_by(|left, right| compare_hands(left, right))
                .expect("a showdown pot has an eligible evaluated hand");
            let mut winners: Vec<SeatId> = pot
                .eligible
                .iter()
                .copied()
                .filter(|seat| {
                    evaluations
                        .get(seat)
                        .is_some_and(|hand| compare_hands(hand, best) == Ordering::Equal)
                })
                .collect();
            winners.sort_by_key(|seat| clockwise_award_key(self.table_size, self.button, *seat));
            let base = pot.amount / winners.len() as u32;
            let remainder = pot.amount % winners.len() as u32;
            let payouts: Vec<SeatPayout> = winners
                .iter()
                .enumerate()
                .map(|(index, &seat)| SeatPayout {
                    seat,
                    amount: base + u32::from((index as u32) < remainder),
                })
                .collect();
            for payout in &payouts {
                self.seat_mut(payout.seat).stack += payout.amount;
            }
            self.awards.push(PotAward {
                pot_index,
                amount: pot.amount,
                eligible: pot.eligible.clone(),
                winners,
                payouts,
            });
        }

        self.clear_contributions();
        self.to_act = None;
        self.phase = MultiwayPhase::Showdown;
        debug_assert_eq!(self.total_chips(), self.initial_total);
    }

    fn contribution_snapshot(&self) -> Vec<Contribution> {
        self.occupied_seats()
            .map(|seat| Contribution {
                seat,
                amount: self.seat(seat).hand_contribution,
                eligible: self.seat(seat).eligible_for_pot(),
            })
            .collect()
    }

    fn clear_contributions(&mut self) {
        for seat in self.occupied_seats().collect::<Vec<_>>() {
            let state = self.seat_mut(seat);
            state.street_contribution = 0;
            state.hand_contribution = 0;
            state.last_action_wager = None;
        }
    }

    fn pot_eligible_count(&self) -> usize {
        self.occupied_seats()
            .filter(|&seat| self.seat(seat).eligible_for_pot())
            .count()
    }

    fn first_needing_from(&self, start: SeatId) -> Option<SeatId> {
        (0..self.table_size.get())
            .map(|offset| seat_wrapped(self.table_size, start, offset))
            .find(|&seat| self.seats[seat.index()].is_some() && self.needs_action(seat))
    }

    fn next_needing_after(&self, after: SeatId) -> Option<SeatId> {
        (1..=self.table_size.get())
            .map(|offset| seat_wrapped(self.table_size, after, offset))
            .find(|&seat| self.seats[seat.index()].is_some() && self.needs_action(seat))
    }

    fn next_live_after(&self, after: SeatId) -> Option<SeatId> {
        (1..=self.table_size.get())
            .map(|offset| seat_wrapped(self.table_size, after, offset))
            .find(|&seat| {
                self.seats[seat.index()]
                    .as_ref()
                    .is_some_and(MultiwaySeatState::can_act)
            })
    }

    pub fn total_chips(&self) -> u32 {
        let stacks: u32 = self
            .occupied_seats()
            .map(|seat| self.seat(seat).stack)
            .sum();
        let committed: u32 = self
            .occupied_seats()
            .map(|seat| self.seat(seat).hand_contribution)
            .sum();
        stacks + committed
    }

    pub const fn initial_total(&self) -> u32 {
        self.initial_total
    }
}

/// Builds ordered pot layers from total hand contributions.
///
/// A layer reached by only one contributor is unmatched excess and is
/// returned. Folded contributors still fund layers but never appear in their
/// eligibility set.
pub fn build_pots(contributions: &[Contribution]) -> PotBuild {
    let mut levels: Vec<u32> = contributions
        .iter()
        .filter_map(|entry| (entry.amount > 0).then_some(entry.amount))
        .collect();
    levels.sort_unstable();
    levels.dedup();

    let mut previous = 0u32;
    let mut pots = Vec::new();
    let mut returned = Vec::new();
    for level in levels {
        let contributors: Vec<&Contribution> = contributions
            .iter()
            .filter(|entry| entry.amount >= level)
            .collect();
        let layer = (level - previous) * contributors.len() as u32;
        if contributors.len() == 1 {
            returned.push(ReturnedExcess {
                seat: contributors[0].seat,
                amount: layer,
            });
        } else if layer > 0 {
            pots.push(Pot {
                amount: layer,
                eligible: contributors
                    .iter()
                    .filter_map(|entry| entry.eligible.then_some(entry.seat))
                    .collect(),
            });
        }
        previous = level;
    }
    PotBuild { pots, returned }
}

fn compare_hands(left: &HandEvaluation, right: &HandEvaluation) -> Ordering {
    left.rank
        .cmp(&right.rank)
        .then_with(|| left.kickers.cmp(&right.kickers))
}

fn clockwise_occupied(
    table_size: TableSize,
    seats: &[Option<MultiwaySeatState>],
    after: SeatId,
) -> Vec<SeatId> {
    (1..=table_size.get())
        .map(|offset| seat_wrapped(table_size, after, offset))
        .filter(|seat| seats[seat.index()].is_some())
        .collect()
}

fn next_occupied(
    table_size: TableSize,
    seats: &[Option<MultiwaySeatState>],
    after: SeatId,
) -> Option<SeatId> {
    (1..=table_size.get())
        .map(|offset| seat_wrapped(table_size, after, offset))
        .find(|seat| seats[seat.index()].is_some())
}

fn seat_wrapped(table_size: TableSize, start: SeatId, offset: u8) -> SeatId {
    SeatId::new((start.as_u8() + offset) % table_size.get())
        .expect("wrapped table seats are supported seat IDs")
}

fn clockwise_award_key(table_size: TableSize, button: SeatId, seat: SeatId) -> u8 {
    let distance = (seat.as_u8() + table_size.get() - button.as_u8()) % table_size.get();
    if distance == 0 {
        table_size.get()
    } else {
        distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    fn table(size: u8, stacks: &[u32]) -> MultiwayHand {
        MultiwayHand::new_seeded_for_review(
            TableSize::new(size).unwrap(),
            seat(0),
            &stacks
                .iter()
                .enumerate()
                .map(|(index, &stack)| (seat(index as u8), stack))
                .collect::<Vec<_>>(),
            20_260_830,
        )
        .unwrap()
    }

    fn apply(hand: &mut MultiwayHand, action: Action) {
        let actor = hand.to_act.expect("test hand has an actor");
        hand.apply_command(SeatCommand::new(actor, action)).unwrap();
    }

    #[test]
    fn legal_actions_are_seat_specific_at_every_occupancy() {
        for size in 2..=9 {
            let hand = table(size, &vec![200; size as usize]);
            let actor = hand.to_act.unwrap();
            let legal = hand.legal_actions_for(actor).unwrap();
            assert!(legal.can_fold, "occupancy {size}");
            assert_eq!(
                legal.call_amount,
                Some(if size == 2 { 1 } else { 2 }),
                "occupancy {size}"
            );
            assert_eq!(legal.min_raise_to, Some(4), "occupancy {size}");
            assert_eq!(legal.all_in_to, 200, "occupancy {size}");
            assert!(hand
                .occupied_seats()
                .filter(|&seat| seat != actor)
                .all(|seat| hand.legal_actions_for(seat).is_none()));
        }
    }

    #[test]
    fn three_way_round_only_advances_after_every_live_seat_responds() {
        let mut hand = table(3, &[200, 200, 200]);
        assert_eq!(hand.to_act, Some(seat(0)));
        apply(&mut hand, Action::Call(2));
        assert_eq!(hand.phase, MultiwayPhase::Preflop);
        apply(&mut hand, Action::Call(1));
        assert_eq!(hand.phase, MultiwayPhase::Preflop);
        apply(&mut hand, Action::Check);
        assert_eq!(hand.phase, MultiwayPhase::Flop);
        assert_eq!(hand.to_act, Some(seat(1)));

        apply(&mut hand, Action::Check);
        apply(&mut hand, Action::Check);
        assert_eq!(hand.phase, MultiwayPhase::Flop);
        apply(&mut hand, Action::Check);
        assert_eq!(hand.phase, MultiwayPhase::Turn);
        assert_eq!(hand.total_chips(), hand.initial_total());
    }

    #[test]
    fn passive_hands_complete_at_every_occupancy_and_conserve_chips() {
        for size in 2..=9 {
            let mut hand = table(size, &vec![200; size as usize]);
            while hand.phase.accepts_actions() {
                let actor = hand.to_act.expect("active passive hand has an actor");
                let legal = hand.legal_actions_for(actor).unwrap();
                let action = legal.call_amount.map_or(Action::Check, Action::Call);
                hand.apply_command(SeatCommand::new(actor, action)).unwrap();
            }
            assert_eq!(hand.phase, MultiwayPhase::Showdown, "occupancy {size}");
            assert_eq!(hand.board.len(), 5, "occupancy {size}");
            assert_eq!(hand.total_chips(), hand.initial_total(), "occupancy {size}");
        }
    }

    #[test]
    fn multiway_bet_raise_call_fold_sequence_completes_the_street() {
        let mut hand = table(4, &[200, 200, 200, 200]);
        apply(&mut hand, Action::Call(2));
        apply(&mut hand, Action::Call(2));
        apply(&mut hand, Action::Call(1));
        apply(&mut hand, Action::Check);
        assert_eq!(hand.phase, MultiwayPhase::Flop);
        assert_eq!(hand.to_act, Some(seat(1)));

        apply(&mut hand, Action::Bet(10));
        apply(&mut hand, Action::Call(10));
        apply(&mut hand, Action::Fold);
        apply(&mut hand, Action::Raise(30));
        apply(&mut hand, Action::Call(20));
        assert_eq!(hand.phase, MultiwayPhase::Flop);
        apply(&mut hand, Action::Call(20));

        assert_eq!(hand.phase, MultiwayPhase::Turn);
        assert_eq!(hand.to_act, Some(seat(1)));
        assert_eq!(hand.seat(seat(3)).participation, HandParticipation::Folded);
        assert_eq!(hand.total_chips(), 800);
    }

    #[test]
    fn a_single_short_all_in_does_not_reopen_raising() {
        let mut hand = table(4, &[100, 14, 100, 100]);
        assert_eq!(hand.to_act, Some(seat(3)));
        apply(&mut hand, Action::Raise(10));
        apply(&mut hand, Action::Call(10));
        apply(&mut hand, Action::AllIn(14));
        apply(&mut hand, Action::Call(12));
        assert_eq!(hand.to_act, Some(seat(3)));
        let legal = hand.legal_actions_for(seat(3)).unwrap();
        assert!(!legal.raise_reopened);
        assert_eq!(legal.min_raise_to, None);
        assert_eq!(
            hand.validate_command(SeatCommand::new(seat(3), Action::Raise(22))),
            Err(CommandError::IllegalAction(ActionError::RaiseNotReopened))
        );
    }

    #[test]
    fn cumulative_short_all_ins_reopen_once_they_equal_a_full_raise() {
        let mut hand = table(4, &[100, 14, 18, 100]);
        apply(&mut hand, Action::Raise(10));
        apply(&mut hand, Action::Call(10));
        apply(&mut hand, Action::AllIn(14));
        apply(&mut hand, Action::AllIn(18));
        assert_eq!(hand.to_act, Some(seat(3)));
        let legal = hand.legal_actions_for(seat(3)).unwrap();
        assert!(legal.raise_reopened);
        assert_eq!(legal.min_raise_to, Some(26));
    }

    #[test]
    fn rejected_closed_raise_leaves_authorized_state_unchanged() {
        let mut hand = table(4, &[100, 14, 100, 100]);
        apply(&mut hand, Action::Raise(10));
        apply(&mut hand, Action::Call(10));
        apply(&mut hand, Action::AllIn(14));
        apply(&mut hand, Action::Call(12));
        let before = format!("{hand:?}");
        assert_eq!(
            hand.apply_command(SeatCommand::new(seat(3), Action::Raise(22))),
            Err(CommandError::IllegalAction(ActionError::RaiseNotReopened))
        );
        assert_eq!(format!("{hand:?}"), before);
    }

    #[test]
    fn arbitrary_pots_include_folded_chips_and_return_unmatched_excess() {
        let build = build_pots(&[
            Contribution {
                seat: seat(0),
                amount: 40,
                eligible: true,
            },
            Contribution {
                seat: seat(1),
                amount: 100,
                eligible: false,
            },
            Contribution {
                seat: seat(2),
                amount: 200,
                eligible: true,
            },
            Contribution {
                seat: seat(3),
                amount: 260,
                eligible: true,
            },
        ]);
        assert_eq!(
            build.pots,
            vec![
                Pot {
                    amount: 160,
                    eligible: vec![seat(0), seat(2), seat(3)],
                },
                Pot {
                    amount: 180,
                    eligible: vec![seat(2), seat(3)],
                },
                Pot {
                    amount: 200,
                    eligible: vec![seat(2), seat(3)],
                },
            ]
        );
        assert_eq!(
            build.returned,
            vec![ReturnedExcess {
                seat: seat(3),
                amount: 60,
            }]
        );
        assert_eq!(
            build.pots.iter().map(|pot| pot.amount).sum::<u32>()
                + build.returned.iter().map(|item| item.amount).sum::<u32>(),
            600
        );
    }

    #[test]
    fn four_all_ins_run_out_and_resolve_three_pot_layers() {
        let mut hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(4).unwrap(),
            seat(0),
            &[
                (seat(0), 40),
                (seat(1), 100),
                (seat(2), 200),
                (seat(3), 200),
            ],
            13,
        )
        .unwrap();
        apply(&mut hand, Action::AllIn(200));
        apply(&mut hand, Action::AllIn(40));
        apply(&mut hand, Action::AllIn(100));
        apply(&mut hand, Action::AllIn(200));
        assert_eq!(hand.phase, MultiwayPhase::Showdown);
        assert_eq!(
            hand.pots.iter().map(|pot| pot.amount).collect::<Vec<_>>(),
            [160, 180, 200]
        );
        assert_eq!(hand.awards.len(), 3);
        assert_eq!(hand.awards[0].winners, [seat(1)]);
        assert_eq!(hand.awards[1].winners, [seat(1)]);
        assert_eq!(hand.awards[2].winners, [seat(2)]);
        assert_eq!(
            hand.settled_contributions
                .iter()
                .map(|entry| entry.amount)
                .collect::<Vec<_>>(),
            [40, 100, 200, 200]
        );
        assert_eq!(hand.board.len(), 5);
        assert_eq!(hand.total_chips(), 540);
        assert_eq!(hand.initial_total(), 540);
    }

    #[test]
    fn tied_odd_chip_is_awarded_clockwise_from_button() {
        use super::super::deck::{Rank, Suit};

        let mut hand = table(3, &[10, 10, 10]);
        for index in 0..3 {
            let state = hand.seat_mut(seat(index));
            state.stack = 9;
            state.street_contribution = 1;
            state.hand_contribution = 1;
            state.participation = if index == 2 {
                HandParticipation::Folded
            } else {
                HandParticipation::AllIn
            };
        }
        hand.board = vec![
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Ace, Suit::Spades),
        ];
        hand.phase = MultiwayPhase::River;
        hand.resolve_showdown();

        assert_eq!(
            hand.pots,
            [Pot {
                amount: 3,
                eligible: vec![seat(0), seat(1)],
            }]
        );
        assert_eq!(hand.awards[0].winners, [seat(1), seat(0)]);
        assert_eq!(
            hand.awards[0].payouts,
            [
                SeatPayout {
                    seat: seat(1),
                    amount: 2,
                },
                SeatPayout {
                    seat: seat(0),
                    amount: 1,
                },
            ]
        );
        assert_eq!(hand.total_chips(), 30);
    }

    #[test]
    fn two_pair_kickers_select_main_and_side_pot_winners_at_every_occupancy() {
        use super::super::deck::{Rank::*, Suit::*};
        let holdings = [
            [(Ace, Clubs), (Four, Spades)],
            [(King, Clubs), (Five, Spades)],
            [(Queen, Clubs), (Six, Spades)],
            [(Ten, Clubs), (Seven, Spades)],
            [(Nine, Clubs), (Eight, Spades)],
            [(Four, Diamonds), (Five, Diamonds)],
            [(Six, Diamonds), (Seven, Diamonds)],
            [(Eight, Diamonds), (Nine, Diamonds)],
            [(Ten, Diamonds), (Queen, Diamonds)],
        ];
        for size in 2..=9 {
            let mut hand = table(size, &vec![10; size as usize]);
            hand.board = [
                (Jack, Spades),
                (Jack, Hearts),
                (Two, Diamonds),
                (Two, Clubs),
                (Three, Spades),
            ]
            .map(|(rank, suit)| Card::new(rank, suit))
            .to_vec();
            for index in 0..size {
                let state = hand.seat_mut(seat(index));
                let contributed = if index == 0 { 4 } else { 6 };
                state.stack = 10 - contributed;
                state.street_contribution = contributed;
                state.hand_contribution = contributed;
                state.participation = HandParticipation::Live;
                state.hole_cards = holdings[index as usize]
                    .map(|(rank, suit)| Card::new(rank, suit))
                    .to_vec();
            }
            hand.phase = MultiwayPhase::River;
            hand.resolve_showdown();
            assert_eq!(hand.awards[0].winners, [seat(0)], "occupancy {size}");
            assert_eq!(hand.awards[0].amount, 4 * u32::from(size));
            if size > 2 {
                assert_eq!(hand.awards.len(), 2);
                assert_eq!(hand.awards[1].winners, [seat(1)], "occupancy {size}");
                assert_eq!(hand.awards[1].amount, 2 * u32::from(size - 1));
            }
            assert_eq!(hand.seat(seat(0)).stack, 6 + 4 * u32::from(size));
            assert_eq!(hand.seat(seat(1)).stack, 4 + 2 * u32::from(size - 1));
            assert_eq!(hand.total_chips(), 10 * u32::from(size));
        }
    }

    #[test]
    fn gapped_table_skips_absent_and_folded_seats() {
        let mut hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(9).unwrap(),
            seat(8),
            &[
                (seat(0), 100),
                (seat(3), 100),
                (seat(7), 100),
                (seat(8), 100),
            ],
            99,
        )
        .unwrap();
        assert_eq!(hand.small_blind, seat(0));
        assert_eq!(hand.big_blind, seat(3));
        assert_eq!(hand.to_act, Some(seat(7)));
        apply(&mut hand, Action::Fold);
        assert_eq!(hand.to_act, Some(seat(8)));
        apply(&mut hand, Action::Call(2));
        assert_eq!(hand.to_act, Some(seat(0)));
    }

    #[test]
    fn overflowing_initial_chip_total_is_rejected() {
        assert_eq!(
            MultiwayHand::new_seeded_for_review(
                TableSize::new(2).unwrap(),
                seat(0),
                &[(seat(0), u32::MAX), (seat(1), 1)],
                1,
            )
            .unwrap_err(),
            MultiwayConfigError::ChipTotalOverflow
        );
    }

    #[test]
    fn tournament_level_posts_antes_and_uses_dynamic_minimum_raise() {
        let blinds = BlindValues::new(10, 20, 3).unwrap();
        let hand = MultiwayHand::new_seeded_with_blinds(
            TableSize::new(3).unwrap(),
            seat(0),
            &[(seat(0), 100), (seat(1), 100), (seat(2), 100)],
            &[],
            blinds,
            2026,
        )
        .unwrap();

        assert_eq!(hand.blind_values, blinds);
        assert_eq!(hand.current_wager, 20);
        assert_eq!(hand.last_full_raise_size, 20);
        assert_eq!(hand.seat(seat(0)).stack, 97);
        assert_eq!(hand.seat(seat(1)).stack, 87);
        assert_eq!(hand.seat(seat(2)).stack, 77);
        assert_eq!(hand.seat(seat(0)).hand_contribution, 3);
        assert_eq!(hand.seat(seat(1)).hand_contribution, 13);
        assert_eq!(hand.seat(seat(2)).hand_contribution, 23);
    }
}
