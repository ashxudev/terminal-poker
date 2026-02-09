use super::actions::{Action, AvailableActions};
use super::deck::{Card, Deck};
use super::hand::{evaluate_hand, HandEvaluation};
use serde::{Deserialize, Serialize};

pub const BIG_BLIND: u32 = 2;
pub const SMALL_BLIND: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Player {
    Human,
    Bot,
}

impl Player {
    pub fn opponent(&self) -> Self {
        match self {
            Player::Human => Player::Bot,
            Player::Bot => Player::Human,
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct GameState {
    pub phase: GamePhase,
    pub deck: Deck,
    pub player_cards: Vec<Card>,
    pub bot_cards: Vec<Card>,
    pub board: Vec<Card>,
    pub pot: u32,
    pub player_stack: u32,
    pub bot_stack: u32,
    pub player_bet: u32,
    pub bot_bet: u32,
    pub to_act: Player,
    pub button: Player,
    pub last_aggressor: Option<Player>,
    pub last_raise_size: u32,
    pub hand_number: u32,
    pub starting_stack: u32,
    pub hands_played: u32,
    pub hands_won: u32,
    pub biggest_pot_won: u32,
    pub biggest_pot_lost: u32,
    pub last_action: Option<(Player, Action)>,
    pub showdown_result: Option<ShowdownResult>,
    pub actions_this_street: u8,
}

#[derive(Debug, Clone)]
pub struct ShowdownResult {
    pub winner: Option<Player>,
    pub player_hand: HandEvaluation,
    pub bot_hand: HandEvaluation,
    pub pot_won: u32,
}

impl GameState {
    pub fn new(starting_stack_bb: u32) -> Self {
        let starting_stack = starting_stack_bb * BIG_BLIND;
        let mut state = Self {
            phase: GamePhase::Preflop,
            deck: Deck::new(),
            player_cards: Vec::new(),
            bot_cards: Vec::new(),
            board: Vec::new(),
            pot: 0,
            player_stack: starting_stack,
            bot_stack: starting_stack,
            player_bet: 0,
            bot_bet: 0,
            to_act: Player::Human,
            button: Player::Bot,
            last_aggressor: None,
            last_raise_size: BIG_BLIND,
            hand_number: 0,
            starting_stack,
            hands_played: 0,
            hands_won: 0,
            biggest_pot_won: 0,
            biggest_pot_lost: 0,
            last_action: None,
            showdown_result: None,
            actions_this_street: 0,
        };
        state.start_new_hand();
        state
    }

    pub fn start_new_hand(&mut self) {
        self.hand_number += 1;
        self.button = self.button.opponent();
        self.phase = GamePhase::Preflop;
        self.deck = Deck::new();
        self.deck.shuffle();
        self.player_cards = self.deck.deal_n(2);
        self.bot_cards = self.deck.deal_n(2);
        self.board.clear();
        self.pot = 0;
        self.player_bet = 0;
        self.bot_bet = 0;
        self.last_aggressor = None;
        self.last_raise_size = BIG_BLIND;
        self.last_action = None;
        self.showdown_result = None;
        self.actions_this_street = 0;

        // Post blinds - button posts SB, other player posts BB
        // In heads-up, button acts first preflop
        match self.button {
            Player::Human => {
                // Human is button (SB), Bot is BB
                let sb = SMALL_BLIND.min(self.player_stack);
                let bb = BIG_BLIND.min(self.bot_stack);
                self.player_stack -= sb;
                self.player_bet = sb;
                self.bot_stack -= bb;
                self.bot_bet = bb;
                self.pot = sb + bb;
                self.to_act = Player::Human; // Button acts first preflop
            }
            Player::Bot => {
                // Bot is button (SB), Human is BB
                let sb = SMALL_BLIND.min(self.bot_stack);
                let bb = BIG_BLIND.min(self.player_stack);
                self.bot_stack -= sb;
                self.bot_bet = sb;
                self.player_stack -= bb;
                self.player_bet = bb;
                self.pot = sb + bb;
                self.to_act = Player::Bot; // Button acts first preflop
            }
        }
    }

    pub fn apply_action(&mut self, player: Player, action: Action) {
        self.last_action = Some((player, action));
        self.actions_this_street += 1;

        match action {
            Action::Fold => {
                self.handle_fold(player);
                return;
            }
            Action::Check => {
                // Nothing to do
            }
            Action::Call(amount) => {
                self.add_chips(player, amount);
            }
            Action::Bet(amount) | Action::Raise(amount) => {
                let current_bet = self.current_bet(player);
                let to_add = amount - current_bet;
                let old_max = self.max_bet(); // Capture before mutation
                self.add_chips(player, to_add);
                self.last_aggressor = Some(player);
                self.last_raise_size = amount - old_max;
            }
            Action::AllIn(amount) => {
                let current_bet = self.current_bet(player);
                let to_add = amount - current_bet;
                let old_max = self.max_bet(); // Capture before mutation
                self.add_chips(player, to_add);
                if amount > old_max {
                    self.last_aggressor = Some(player);
                    self.last_raise_size = amount - old_max;
                }
            }
        }

        // Check if betting round is complete
        if self.is_betting_round_complete() {
            self.advance_phase();
        } else {
            self.to_act = player.opponent();
        }
    }

    fn add_chips(&mut self, player: Player, amount: u32) {
        match player {
            Player::Human => {
                let actual = amount.min(self.player_stack);
                self.player_stack -= actual;
                self.player_bet += actual;
                self.pot += actual;
            }
            Player::Bot => {
                let actual = amount.min(self.bot_stack);
                self.bot_stack -= actual;
                self.bot_bet += actual;
                self.pot += actual;
            }
        }
    }

    fn current_bet(&self, player: Player) -> u32 {
        match player {
            Player::Human => self.player_bet,
            Player::Bot => self.bot_bet,
        }
    }

    fn max_bet(&self) -> u32 {
        self.player_bet.max(self.bot_bet)
    }

    fn handle_fold(&mut self, folder: Player) {
        let winner = folder.opponent();
        let pot = self.pot;

        match winner {
            Player::Human => {
                self.player_stack += pot;
                self.hands_won += 1;
                if pot > self.biggest_pot_won {
                    self.biggest_pot_won = pot;
                }
            }
            Player::Bot => {
                self.bot_stack += pot;
                if pot > self.biggest_pot_lost {
                    self.biggest_pot_lost = pot;
                }
            }
        }

        self.pot = 0;
        self.hands_played += 1;
        self.phase = GamePhase::HandComplete;
    }

    fn is_betting_round_complete(&self) -> bool {
        // All-in situations: round complete when bets are equal (opponent called/matched)
        if self.player_stack == 0 || self.bot_stack == 0 {
            return self.player_bet == self.bot_bet;
        }

        // Bets must be equal
        if self.player_bet != self.bot_bet {
            return false;
        }

        // Preflop special case: BB gets option if no raise
        if self.phase == GamePhase::Preflop {
            if self.last_aggressor.is_none() {
                // No raise yet, BB gets option. Round complete only when BB has checked.
                let bb_player = self.button.opponent();
                return self.last_action
                    .map(|(actor, action)| actor == bb_player && action == Action::Check)
                    .unwrap_or(false);
            }
        }

        // Postflop: both players must have acted for round to complete
        // (With no aggressor and equal bets, need at least 2 actions = both checked)
        if self.last_aggressor.is_none() {
            return self.actions_this_street >= 2;
        }

        // With an aggressor, if bets are equal, the other player called
        true
    }

    fn advance_phase(&mut self) {
        // Reset street bets and action counter
        self.player_bet = 0;
        self.bot_bet = 0;
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

        // Postflop: BB (non-button) acts first
        self.to_act = self.button.opponent();
    }

    fn resolve_showdown(&mut self) {
        let player_eval = evaluate_hand(&self.player_cards, &self.board);
        let bot_eval = evaluate_hand(&self.bot_cards, &self.board);

        let winner = match player_eval.rank.cmp(&bot_eval.rank) {
            std::cmp::Ordering::Greater => Some(Player::Human),
            std::cmp::Ordering::Less => Some(Player::Bot),
            std::cmp::Ordering::Equal => {
                match player_eval.kickers.cmp(&bot_eval.kickers) {
                    std::cmp::Ordering::Greater => Some(Player::Human),
                    std::cmp::Ordering::Less => Some(Player::Bot),
                    std::cmp::Ordering::Equal => None, // Split pot
                }
            }
        };

        let pot = self.pot;
        match winner {
            Some(Player::Human) => {
                self.player_stack += pot;
                self.hands_won += 1;
                if pot > self.biggest_pot_won {
                    self.biggest_pot_won = pot;
                }
            }
            Some(Player::Bot) => {
                self.bot_stack += pot;
                if pot > self.biggest_pot_lost {
                    self.biggest_pot_lost = pot;
                }
            }
            None => {
                // Split pot - odd chip goes to out-of-position player (non-button)
                let half = pot / 2;
                let remainder = pot % 2;
                if self.button == Player::Human {
                    // Bot is out of position, gets odd chip
                    self.player_stack += half;
                    self.bot_stack += half + remainder;
                } else {
                    // Human is out of position, gets odd chip
                    self.player_stack += half + remainder;
                    self.bot_stack += half;
                }
            }
        }

        self.showdown_result = Some(ShowdownResult {
            winner,
            player_hand: player_eval,
            bot_hand: bot_eval,
            pot_won: pot,
        });

        self.pot = 0;
        self.hands_played += 1;
        self.phase = GamePhase::Showdown;
    }

    pub fn amount_to_call(&self, player: Player) -> u32 {
        let current = self.current_bet(player);
        let max = self.max_bet();
        if max > current {
            max - current
        } else {
            0
        }
    }

    pub fn available_actions(&self) -> AvailableActions {
        let stack = match self.to_act {
            Player::Human => self.player_stack,
            Player::Bot => self.bot_stack,
        };

        let to_call = self.amount_to_call(self.to_act);
        let min_raise_to = self.max_bet() + self.last_raise_size.max(BIG_BLIND);

        AvailableActions::new(to_call, min_raise_to, stack, BIG_BLIND)
    }

    pub fn pot_odds(&self) -> Option<(f64, f64)> {
        let to_call = self.amount_to_call(Player::Human);
        if to_call == 0 {
            return None;
        }

        let pot_after_call = self.pot + to_call;
        let ratio = pot_after_call as f64 / to_call as f64;
        let equity_needed = to_call as f64 / pot_after_call as f64;

        Some((ratio, equity_needed))
    }

    pub fn is_player_turn(&self) -> bool {
        self.to_act == Player::Human
            && !matches!(
                self.phase,
                GamePhase::Showdown | GamePhase::HandComplete | GamePhase::SessionEnd | GamePhase::Summary
            )
    }

    pub fn session_profit_bb(&self) -> f64 {
        let current = self.player_stack as f64;
        let starting = self.starting_stack as f64;
        (current - starting) / BIG_BLIND as f64
    }
}
