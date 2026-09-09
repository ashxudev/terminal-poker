use super::actions::{Action, AvailableActions};
use super::command::{ActionError, CommandError, SeatCommand};
use super::deck::{Card, Deck, ShuffleSource};
use super::hand::{evaluate_hand, HandEvaluation};
use super::seat::{PlayerId, SeatId, TableSize};
use super::table::{HandParticipation, SeatState, TableSeats};
use serde::{Deserialize, Serialize};

pub const BIG_BLIND: u32 = 2;
pub const SMALL_BLIND: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase {
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
    HandComplete,
    SessionEnd,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
}

impl From<GamePhase> for Option<Street> {
    fn from(phase: GamePhase) -> Self {
        match phase {
            GamePhase::Preflop => Some(Street::Preflop),
            GamePhase::Flop => Some(Street::Flop),
            GamePhase::Turn => Some(Street::Turn),
            GamePhase::River => Some(Street::River),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeatSessionStats {
    pub hands_won: u32,
    pub biggest_pot_won: u32,
    pub biggest_pot_lost: u32,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub phase: GamePhase,
    pub deck: Deck,
    pub seats: TableSeats,
    pub board: Vec<Card>,
    pub pot: u32,
    pub to_act: SeatId,
    pub button: SeatId,
    pub last_aggressor: Option<SeatId>,
    pub preflop_aggressor: Option<SeatId>,
    pub last_raise_size: u32,
    pub hand_number: u32,
    pub hands_played: u32,
    pub last_action: Option<(SeatId, Action)>,
    pub showdown_result: Option<ShowdownResult>,
    pub actions_this_street: u8,
    starting_stacks: Vec<u32>,
    seat_stats: Vec<SeatSessionStats>,
    shuffle_source: ShuffleSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatHandEvaluation {
    pub seat: SeatId,
    pub hand: HandEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShowdownResult {
    pub winner: Option<SeatId>,
    pub hands: Vec<SeatHandEvaluation>,
    pub pot_won: u32,
}

impl ShowdownResult {
    pub fn hand_for(&self, seat: SeatId) -> Option<&HandEvaluation> {
        self.hands
            .iter()
            .find(|evaluation| evaluation.seat == seat)
            .map(|evaluation| &evaluation.hand)
    }
}

impl GameState {
    /// Creates the current offline two-seat game using neutral domain identities.
    /// General multiway betting is intentionally not enabled by this constructor.
    pub fn new(starting_stack_bb: u32) -> Self {
        Self::with_shuffle_source(starting_stack_bb, ShuffleSource::production())
    }

    /// Deterministic construction for executable tests and review evidence.
    /// Production callers must use [`GameState::new`].
    pub fn new_seeded_for_review(starting_stack_bb: u32, seed: u64) -> Self {
        Self::with_shuffle_source(
            starting_stack_bb,
            ShuffleSource::deterministic_for_review(seed),
        )
    }

    fn with_shuffle_source(starting_stack_bb: u32, shuffle_source: ShuffleSource) -> Self {
        let starting_stack = starting_stack_bb * BIG_BLIND;
        let table_size = TableSize::new(2).expect("heads-up is a valid table size");
        let first_seat = SeatId::new(0).expect("offline seat is valid");
        let second_seat = SeatId::new(1).expect("offline seat is valid");
        let mut seats = TableSeats::new(table_size);
        seats
            .occupy(
                first_seat,
                SeatState::active(PlayerId::new(1), starting_stack),
            )
            .expect("offline first seat is unique");
        seats
            .occupy(
                second_seat,
                SeatState::active(PlayerId::new(2), starting_stack),
            )
            .expect("offline second seat is unique");

        let mut state = Self {
            phase: GamePhase::Preflop,
            deck: Deck::new(),
            seats,
            board: Vec::new(),
            pot: 0,
            to_act: first_seat,
            button: second_seat,
            last_aggressor: None,
            preflop_aggressor: None,
            last_raise_size: BIG_BLIND,
            hand_number: 0,
            hands_played: 0,
            last_action: None,
            showdown_result: None,
            actions_this_street: 0,
            starting_stacks: vec![starting_stack; 2],
            seat_stats: vec![SeatSessionStats::default(); 2],
            shuffle_source,
        };
        state.start_new_hand();
        state
    }

    pub fn start_new_hand(&mut self) {
        self.hand_number += 1;
        self.button = self
            .seats
            .next_for_hand(self.button)
            .expect("offline game always has another hand-eligible seat");
        self.phase = GamePhase::Preflop;
        self.deck = Deck::new();
        self.shuffle_source.shuffle(&mut self.deck);
        self.board.clear();
        self.pot = 0;
        self.last_aggressor = None;
        self.preflop_aggressor = None;
        self.last_raise_size = BIG_BLIND;
        self.last_action = None;
        self.showdown_result = None;
        self.actions_this_street = 0;

        let dealt_seats: Vec<SeatId> = self
            .seats
            .occupied()
            .filter(|(_, state)| state.eligible_for_next_hand())
            .map(|(seat, _)| seat)
            .collect();
        for seat in dealt_seats {
            let cards = self.deck.deal_n(2);
            let state = self.seat_mut(seat);
            state.hole_cards = cards;
            state.street_bet = 0;
            state.hand_participation = HandParticipation::Live;
        }

        let positions = self
            .seats
            .positions(self.button)
            .expect("offline game always has valid heads-up positions");
        self.post_blind(positions.small_blind, SMALL_BLIND);
        self.post_blind(positions.big_blind, BIG_BLIND);
        self.to_act = positions.first_preflop;
    }

    pub fn seat(&self, seat: SeatId) -> &SeatState {
        self.seats
            .seat(seat)
            .expect("game operations require an occupied seat")
    }

    pub fn seat_mut(&mut self, seat: SeatId) -> &mut SeatState {
        self.seats
            .seat_mut(seat)
            .expect("game operations require an occupied seat")
    }

    pub fn stack(&self, seat: SeatId) -> u32 {
        self.seat(seat).stack
    }

    pub fn street_bet(&self, seat: SeatId) -> u32 {
        self.seat(seat).street_bet
    }

    pub fn hole_cards(&self, seat: SeatId) -> &[Card] {
        &self.seat(seat).hole_cards
    }

    pub fn seat_stats(&self, seat: SeatId) -> &SeatSessionStats {
        &self.seat_stats[seat.index()]
    }

    /// Validates a controller command without changing authoritative state.
    pub fn validate_command(&self, command: SeatCommand) -> Result<(), CommandError> {
        if !matches!(
            self.phase,
            GamePhase::Preflop | GamePhase::Flop | GamePhase::Turn | GamePhase::River
        ) {
            return Err(CommandError::HandNotActive);
        }

        let seat_state = self
            .seats
            .seat(command.seat)
            .ok_or(CommandError::SeatNotOccupied(command.seat))?;
        if !seat_state.eligible_to_act() {
            return Err(CommandError::SeatNotEligible(command.seat));
        }
        if command.seat != self.to_act {
            return Err(CommandError::OutOfTurn {
                expected: self.to_act,
                actual: command.seat,
            });
        }

        self.validate_action(command.seat, command.action)
            .map_err(CommandError::IllegalAction)
    }

    /// The sole authoritative mutation path for player, bot, and future
    /// network controllers.
    pub fn apply_command(&mut self, command: SeatCommand) -> Result<(), CommandError> {
        self.validate_command(command)?;
        self.apply_validated_action(command.seat, command.action);
        Ok(())
    }

    fn validate_action(&self, seat: SeatId, action: Action) -> Result<(), ActionError> {
        let to_call = self.amount_to_call(seat);
        let street_bet = self.street_bet(seat);
        let stack = self.stack(seat);
        let maximum_total = street_bet.saturating_add(stack);
        let minimum_raise_to = self
            .max_bet()
            .saturating_add(self.last_raise_size.max(BIG_BLIND));

        match action {
            Action::Fold if to_call > 0 => Ok(()),
            Action::Fold => Err(ActionError::FoldNotAllowed),
            Action::Check if to_call == 0 => Ok(()),
            Action::Check => Err(ActionError::CheckNotAllowed),
            Action::Call(_) if to_call == 0 || to_call >= stack => Err(ActionError::CallNotAllowed),
            Action::Call(actual) if actual == to_call => Ok(()),
            Action::Call(actual) => Err(ActionError::InvalidCall {
                expected: to_call,
                actual,
            }),
            Action::Bet(_) if to_call > 0 => Err(ActionError::BetNotAllowed),
            Action::Bet(actual) => {
                let minimum = if self.max_bet() == 0 {
                    BIG_BLIND
                } else {
                    minimum_raise_to
                };
                let maximum = maximum_total.saturating_sub(1);
                if actual >= minimum && actual <= maximum {
                    Ok(())
                } else {
                    Err(ActionError::BetOutOfRange {
                        min: minimum,
                        max: maximum,
                        actual,
                    })
                }
            }
            Action::Raise(_) if to_call == 0 => Err(ActionError::RaiseNotAllowed),
            Action::Raise(actual) => {
                let maximum = maximum_total.saturating_sub(1);
                if actual >= minimum_raise_to && actual <= maximum {
                    Ok(())
                } else {
                    Err(ActionError::RaiseOutOfRange {
                        min: minimum_raise_to,
                        max: maximum,
                        actual,
                    })
                }
            }
            Action::AllIn(actual) if actual == maximum_total && stack > 0 => Ok(()),
            Action::AllIn(actual) => Err(ActionError::InvalidAllIn {
                expected: maximum_total,
                actual,
            }),
        }
    }

    fn apply_validated_action(&mut self, seat: SeatId, action: Action) {
        self.last_action = Some((seat, action));
        self.actions_this_street += 1;

        match action {
            Action::Fold => {
                self.handle_fold(seat);
                return;
            }
            Action::Check => {}
            Action::Call(amount) => {
                self.add_chips(seat, amount);
            }
            Action::Bet(amount) | Action::Raise(amount) => {
                let current_bet = self.street_bet(seat);
                let to_add = amount - current_bet;
                let old_max = self.max_bet();
                self.add_chips(seat, to_add);
                self.last_aggressor = Some(seat);
                self.last_raise_size = amount - old_max;
                if self.phase == GamePhase::Preflop {
                    self.preflop_aggressor = Some(seat);
                }
            }
            Action::AllIn(amount) => {
                let current_bet = self.street_bet(seat);
                let to_add = amount - current_bet;
                let old_max = self.max_bet();
                self.add_chips(seat, to_add);
                self.seat_mut(seat).hand_participation = HandParticipation::AllIn;
                if amount > old_max {
                    self.last_aggressor = Some(seat);
                    self.last_raise_size = amount - old_max;
                    if self.phase == GamePhase::Preflop {
                        self.preflop_aggressor = Some(seat);
                    }
                }
            }
        }

        if self.is_betting_round_complete() {
            self.advance_phase();
        } else {
            self.to_act = self
                .seats
                .next_to_act(seat)
                .expect("an incomplete heads-up round has another actor");
        }
    }

    pub fn max_bet(&self) -> u32 {
        self.seats
            .occupied()
            .map(|(_, state)| state.street_bet)
            .max()
            .unwrap_or(0)
    }

    pub fn amount_to_call(&self, seat: SeatId) -> u32 {
        self.max_bet().saturating_sub(self.street_bet(seat))
    }

    pub fn available_actions(&self) -> AvailableActions {
        let stack = self.stack(self.to_act);
        let to_call = self.amount_to_call(self.to_act);
        let min_raise_to = self.max_bet() + self.last_raise_size.max(BIG_BLIND);
        AvailableActions::new(to_call, min_raise_to, stack, BIG_BLIND)
    }

    #[allow(dead_code)]
    pub fn pot_odds(&self, seat: SeatId) -> Option<(f64, f64)> {
        let to_call = self.amount_to_call(seat);
        if to_call == 0 {
            return None;
        }

        let pot_after_call = self.pot + to_call;
        let ratio = pot_after_call as f64 / to_call as f64;
        let equity_needed = to_call as f64 / pot_after_call as f64;
        Some((ratio, equity_needed))
    }

    pub fn is_turn(&self, seat: SeatId) -> bool {
        self.to_act == seat
            && !matches!(
                self.phase,
                GamePhase::Showdown
                    | GamePhase::HandComplete
                    | GamePhase::SessionEnd
                    | GamePhase::Summary
            )
    }

    pub fn session_profit_bb(&self, seat: SeatId) -> f64 {
        let current = self.stack(seat) as f64;
        let starting = self.starting_stacks[seat.index()] as f64;
        (current - starting) / BIG_BLIND as f64
    }

    pub fn advance_phase(&mut self) {
        self.return_uncalled_heads_up_excess();

        let occupied: Vec<SeatId> = self.seats.occupied().map(|(seat, _)| seat).collect();
        for seat in occupied {
            self.seat_mut(seat).street_bet = 0;
        }
        self.last_aggressor = None;
        self.actions_this_street = 0;

        match self.phase {
            GamePhase::Preflop => {
                self.board.extend(self.deck.deal_n(3));
                self.phase = GamePhase::Flop;
            }
            GamePhase::Flop => {
                self.board.extend(self.deck.deal_n(1));
                self.phase = GamePhase::Turn;
            }
            GamePhase::Turn => {
                self.board.extend(self.deck.deal_n(1));
                self.phase = GamePhase::River;
            }
            GamePhase::River => {
                self.resolve_showdown();
                return;
            }
            _ => return,
        }

        if let Some(first_postflop) = self.seats.next_to_act(self.button) {
            self.to_act = first_postflop;
        }
    }

    fn post_blind(&mut self, seat: SeatId, blind: u32) {
        self.add_chips(seat, blind);
        if self.stack(seat) == 0 {
            self.seat_mut(seat).hand_participation = HandParticipation::AllIn;
        }
    }

    fn add_chips(&mut self, seat: SeatId, amount: u32) {
        let state = self.seat_mut(seat);
        let actual = amount.min(state.stack);
        state.stack -= actual;
        state.street_bet += actual;
        self.pot += actual;
    }

    fn handle_fold(&mut self, folder: SeatId) {
        self.seat_mut(folder).hand_participation = HandParticipation::Folded;
        let winner = self
            .seats
            .next_for_pot(folder)
            .expect("heads-up fold leaves one pot-eligible winner");
        let pot = self.pot;
        self.seat_mut(winner).stack += pot;
        self.record_result(winner, pot);
        self.pot = 0;
        self.hands_played += 1;
        self.phase = GamePhase::HandComplete;
    }

    fn is_betting_round_complete(&self) -> bool {
        let [first, second] = self.heads_up_seats();
        let first_stack = self.stack(first);
        let second_stack = self.stack(second);
        let first_bet = self.street_bet(first);
        let second_bet = self.street_bet(second);

        if first_stack == 0 && second_stack == 0 {
            return true;
        }

        if first_stack == 0 || second_stack == 0 {
            let (all_in_bet, other_bet) = if first_stack == 0 {
                (first_bet, second_bet)
            } else {
                (second_bet, first_bet)
            };
            if all_in_bet <= other_bet {
                return true;
            }
            return first_bet == second_bet;
        }

        if first_bet != second_bet {
            return false;
        }

        if self.phase == GamePhase::Preflop && self.last_aggressor.is_none() {
            let big_blind = self
                .seats
                .positions(self.button)
                .expect("offline positions remain valid")
                .big_blind;
            return self
                .last_action
                .is_some_and(|(actor, action)| actor == big_blind && action == Action::Check);
        }

        if self.last_aggressor.is_none() {
            return self.actions_this_street >= 2;
        }

        true
    }

    fn return_uncalled_heads_up_excess(&mut self) {
        let [first, second] = self.heads_up_seats();
        let first_bet = self.street_bet(first);
        let second_bet = self.street_bet(second);
        if first_bet == second_bet {
            return;
        }

        let effective = first_bet.min(second_bet);
        let excess_seat = if first_bet > effective { first } else { second };
        let excess = self.street_bet(excess_seat) - effective;
        let state = self.seat_mut(excess_seat);
        state.stack += excess;
        state.street_bet -= excess;
        self.pot -= excess;
    }

    fn resolve_showdown(&mut self) {
        let [first, second] = self.heads_up_seats();
        let first_eval = evaluate_hand(self.hole_cards(first), &self.board);
        let second_eval = evaluate_hand(self.hole_cards(second), &self.board);

        let winner = match first_eval.rank.cmp(&second_eval.rank) {
            std::cmp::Ordering::Greater => Some(first),
            std::cmp::Ordering::Less => Some(second),
            std::cmp::Ordering::Equal => match first_eval.kickers.cmp(&second_eval.kickers) {
                std::cmp::Ordering::Greater => Some(first),
                std::cmp::Ordering::Less => Some(second),
                std::cmp::Ordering::Equal => None,
            },
        };

        let pot = self.pot;
        match winner {
            Some(winner) => {
                self.seat_mut(winner).stack += pot;
                self.record_result(winner, pot);
            }
            None => {
                let half = pot / 2;
                let remainder = pot % 2;
                let odd_chip_seat = self
                    .seats
                    .next_for_pot(self.button)
                    .expect("a showdown has a pot-eligible seat after the button");
                self.seat_mut(first).stack += half + u32::from(first == odd_chip_seat) * remainder;
                self.seat_mut(second).stack +=
                    half + u32::from(second == odd_chip_seat) * remainder;
            }
        }

        self.showdown_result = Some(ShowdownResult {
            winner,
            hands: vec![
                SeatHandEvaluation {
                    seat: first,
                    hand: first_eval,
                },
                SeatHandEvaluation {
                    seat: second,
                    hand: second_eval,
                },
            ],
            pot_won: pot,
        });
        self.pot = 0;
        self.hands_played += 1;
        self.phase = GamePhase::Showdown;
    }

    fn record_result(&mut self, winner: SeatId, pot: u32) {
        for seat in self.heads_up_seats() {
            let stats = &mut self.seat_stats[seat.index()];
            if seat == winner {
                stats.hands_won += 1;
                stats.biggest_pot_won = stats.biggest_pot_won.max(pot);
            } else {
                stats.biggest_pot_lost = stats.biggest_pot_lost.max(pot);
            }
        }
    }

    fn heads_up_seats(&self) -> [SeatId; 2] {
        let seats: Vec<SeatId> = self.seats.occupied().map(|(seat, _)| seat).collect();
        seats.try_into().unwrap_or_else(|_| {
            panic!("offline betting engine requires exactly two occupied seats")
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct AuthoritativeSnapshot {
        phase: GamePhase,
        deck: Deck,
        seats: TableSeats,
        board: Vec<Card>,
        pot: u32,
        to_act: SeatId,
        button: SeatId,
        last_aggressor: Option<SeatId>,
        preflop_aggressor: Option<SeatId>,
        last_raise_size: u32,
        hand_number: u32,
        hands_played: u32,
        last_action: Option<(SeatId, Action)>,
        showdown_result: Option<ShowdownResult>,
        actions_this_street: u8,
        starting_stacks: Vec<u32>,
        seat_stats: Vec<SeatSessionStats>,
    }

    impl From<&GameState> for AuthoritativeSnapshot {
        fn from(state: &GameState) -> Self {
            Self {
                phase: state.phase,
                deck: state.deck.clone(),
                seats: state.seats.clone(),
                board: state.board.clone(),
                pot: state.pot,
                to_act: state.to_act,
                button: state.button,
                last_aggressor: state.last_aggressor,
                preflop_aggressor: state.preflop_aggressor,
                last_raise_size: state.last_raise_size,
                hand_number: state.hand_number,
                hands_played: state.hands_played,
                last_action: state.last_action,
                showdown_result: state.showdown_result.clone(),
                actions_this_street: state.actions_this_street,
                starting_stacks: state.starting_stacks.clone(),
                seat_stats: state.seat_stats.clone(),
            }
        }
    }

    fn assert_rejected_without_mutation(
        state: &mut GameState,
        command: SeatCommand,
        expected: CommandError,
    ) {
        let before = AuthoritativeSnapshot::from(&*state);
        assert_eq!(state.apply_command(command), Err(expected));
        assert_eq!(AuthoritativeSnapshot::from(&*state), before);
    }

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    #[test]
    fn offline_constructor_uses_neutral_seat_ownership() {
        let state = GameState::new(100);

        assert_eq!(state.seats.occupied_count(), 2);
        assert_eq!(state.seat(seat(0)).player_id(), PlayerId::new(1));
        assert_eq!(state.seat(seat(1)).player_id(), PlayerId::new(2));
        assert_eq!(state.hole_cards(seat(0)).len(), 2);
        assert_eq!(state.hole_cards(seat(1)).len(), 2);
    }

    #[test]
    fn initial_offline_hand_conserves_chips_and_deals_unique_cards() {
        let state = GameState::new(100);
        let total_chips = state.pot + state.stack(seat(0)) + state.stack(seat(1));
        assert_eq!(total_chips, 400);

        let cards: HashSet<Card> = state
            .seats
            .occupied()
            .flat_map(|(_, seat_state)| seat_state.hole_cards.iter().copied())
            .collect();
        assert_eq!(cards.len(), 4);
    }

    #[test]
    fn seeded_review_states_reproduce_the_same_initial_hand() {
        let first = GameState::new_seeded_for_review(100, 20260830);
        let second = GameState::new_seeded_for_review(100, 20260830);

        assert_eq!(first.hole_cards(seat(0)), second.hole_cards(seat(0)));
        assert_eq!(first.hole_cards(seat(1)), second.hole_cards(seat(1)));
        assert_eq!(first.deck, second.deck);
    }

    #[test]
    fn different_review_seeds_change_the_initial_hand() {
        let first = GameState::new_seeded_for_review(100, 1);
        let second = GameState::new_seeded_for_review(100, 2);

        assert_ne!(first.deck, second.deck);
    }

    #[test]
    fn accepted_seat_command_is_the_authoritative_mutation_path() {
        let mut state = GameState::new_seeded_for_review(100, 20260830);
        let actor = state.to_act;

        state
            .apply_command(SeatCommand::new(actor, Action::Call(1)))
            .unwrap();

        assert_eq!(state.last_action, Some((actor, Action::Call(1))));
        assert_eq!(state.pot, 4);
        assert_ne!(state.to_act, actor);
    }

    #[test]
    fn out_of_turn_command_is_rejected_without_authoritative_mutation() {
        let mut state = GameState::new_seeded_for_review(100, 20260830);
        let expected = state.to_act;
        let actual = seat(1);

        assert_rejected_without_mutation(
            &mut state,
            SeatCommand::new(actual, Action::Check),
            CommandError::OutOfTurn { expected, actual },
        );
    }

    #[test]
    fn ineligible_seat_command_is_rejected_without_authoritative_mutation() {
        let mut state = GameState::new_seeded_for_review(100, 20260830);
        let actor = seat(1);
        state.to_act = actor;
        state.seat_mut(actor).hand_participation = HandParticipation::Folded;

        assert_rejected_without_mutation(
            &mut state,
            SeatCommand::new(actor, Action::Check),
            CommandError::SeatNotEligible(actor),
        );
    }

    #[test]
    fn unoccupied_seat_command_is_rejected_without_authoritative_mutation() {
        let mut state = GameState::new_seeded_for_review(100, 20260830);
        let actor = seat(2);

        assert_rejected_without_mutation(
            &mut state,
            SeatCommand::new(actor, Action::Check),
            CommandError::SeatNotOccupied(actor),
        );
    }

    #[test]
    fn terminal_hand_command_is_rejected_without_authoritative_mutation() {
        let mut state = GameState::new_seeded_for_review(100, 20260830);
        let actor = state.to_act;
        state.phase = GamePhase::HandComplete;

        assert_rejected_without_mutation(
            &mut state,
            SeatCommand::new(actor, Action::Check),
            CommandError::HandNotActive,
        );
    }

    #[test]
    fn illegal_action_amount_is_rejected_without_authoritative_mutation() {
        let mut state = GameState::new_seeded_for_review(100, 20260830);
        let actor = state.to_act;

        assert_rejected_without_mutation(
            &mut state,
            SeatCommand::new(actor, Action::Call(2)),
            CommandError::IllegalAction(ActionError::InvalidCall {
                expected: 1,
                actual: 2,
            }),
        );
    }

    #[test]
    fn malformed_all_in_is_rejected_without_authoritative_mutation() {
        let mut state = GameState::new_seeded_for_review(100, 20260830);
        let actor = state.to_act;
        let expected = state.street_bet(actor) + state.stack(actor);

        assert_rejected_without_mutation(
            &mut state,
            SeatCommand::new(actor, Action::AllIn(expected - 1)),
            CommandError::IllegalAction(ActionError::InvalidAllIn {
                expected,
                actual: expected - 1,
            }),
        );
    }
}
