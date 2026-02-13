use crate::game::actions::Action;
use crate::game::deck::{Card, Suit};
use crate::game::hand::evaluate_hand;
use crate::game::state::{GamePhase, GameState, Player, BIG_BLIND};

use super::draws::detect_draws;
use super::preflop::preflop_strength;

use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardTexture {
    Dry,
    Medium,
    Wet,
}

#[derive(Debug, Clone, Copy)]
enum BetSize {
    Small,
    Medium,
    Large,
}

impl BetSize {
    fn pot_fraction(self) -> f64 {
        match self {
            BetSize::Small => 0.30,
            BetSize::Medium => 0.60,
            BetSize::Large => 0.85,
        }
    }
}

pub struct RuleBasedBot {
    pub aggression: f64,
}

impl RuleBasedBot {
    pub fn new(aggression: f64) -> Self {
        Self {
            aggression: aggression.clamp(0.0, 1.0),
        }
    }

    pub fn decide(&self, state: &GameState) -> Action {
        match state.phase {
            GamePhase::Preflop => self.decide_preflop(state),
            GamePhase::Flop | GamePhase::Turn => self.decide_postflop(state),
            GamePhase::River => self.decide_river(state),
            _ => Action::Check,
        }
    }

    // ── Preflop ─────────────────────────────────────────────

    fn decide_preflop(&self, state: &GameState) -> Action {
        let strength = preflop_strength(&state.bot_cards);
        let to_call = state.amount_to_call(Player::Bot);
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;

        let mut rng = rand::thread_rng();
        let noise: f64 = rng.gen_range(-0.05..0.05);
        let aggression_adj = (self.aggression - 0.5) * 0.10;
        let adjusted = strength + aggression_adj + noise;

        if to_call == 0 {
            // BB option: check or raise
            if adjusted > 0.70 && self.aggression > 0.2 {
                return self.preflop_raise(3.0, state);
            }
            if adjusted > 0.55 && self.aggression > 0.3 {
                return self.preflop_raise(2.5, state);
            }
            if adjusted > 0.45 && self.aggression > 0.5 && rng.gen_bool(0.25) {
                return self.preflop_raise(2.5, state);
            }
            return Action::Check;
        }

        let facing_raise = state.last_aggressor.is_some();

        if !facing_raise {
            // SB open: Playable+ raises, Marginal limps, Trash folds
            if adjusted > 0.50 && self.aggression > 0.15 {
                let mult = if adjusted > 0.80 { 3.0 } else { 2.5 };
                return self.preflop_raise(mult, state);
            }
            if adjusted > 0.35 {
                return self.make_call(to_call, stack, bot_bet);
            }
            if self.aggression > 0.7 && rng.gen_bool(0.08) {
                return self.preflop_raise(3.0, state);
            }
            return Action::Fold;
        }

        // Facing a raise
        if adjusted > 0.80 {
            if let Some(min_raise) = available.min_raise {
                let raise_to = ((state.player_bet as f64) * 3.0) as u32;
                let raise_to = raise_to.max(min_raise);
                if raise_to >= max_bet {
                    return Action::AllIn(max_bet);
                }
                return Action::Raise(raise_to);
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.65 {
            if available.min_raise.is_some() && self.aggression > 0.5 && rng.gen_bool(0.25) {
                let min_raise = available.min_raise.unwrap();
                let raise_to = ((state.player_bet as f64) * 2.5) as u32;
                let raise_to = raise_to.max(min_raise);
                if raise_to < max_bet {
                    return Action::Raise(raise_to);
                }
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.50 {
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.35 && to_call <= BIG_BLIND * 3 {
            return self.make_call(to_call, stack, bot_bet);
        }

        if self.aggression > 0.7 && rng.gen_bool(0.05) {
            if let Some(min_raise) = available.min_raise {
                let raise_to = (BIG_BLIND * 7).max(min_raise);
                if raise_to < max_bet {
                    return Action::Raise(raise_to);
                }
            }
        }

        Action::Fold
    }

    fn preflop_raise(&self, bb_multiplier: f64, state: &GameState) -> Action {
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;
        let raise_to = (BIG_BLIND as f64 * bb_multiplier) as u32;

        if state.amount_to_call(Player::Bot) == 0 {
            // BB option — emit Bet (raise over posted blind)
            let min = available.min_bet.unwrap_or(BIG_BLIND);
            let amount = raise_to.max(min);
            if amount >= max_bet {
                Action::AllIn(max_bet)
            } else {
                Action::Bet(amount)
            }
        } else {
            // SB or facing bet — emit Raise
            let min = available.min_raise.unwrap_or(raise_to);
            let amount = raise_to.max(min);
            if amount >= max_bet {
                Action::AllIn(max_bet)
            } else {
                Action::Raise(amount)
            }
        }
    }

    // ── Postflop (Flop / Turn) ──────────────────────────────

    fn decide_postflop(&self, state: &GameState) -> Action {
        let made = evaluate_hand(&state.bot_cards, &state.board).strength();
        let street_factor = match state.phase {
            GamePhase::Flop => 1.0,
            GamePhase::Turn => 0.5,
            _ => 0.0,
        };
        let draws = detect_draws(&state.bot_cards, &state.board);
        let draw_boost = draws.equity_boost(street_factor);
        let effective = made + draw_boost;
        let adjusted = self.adjust_strength(effective, state);
        let texture = analyze_board_texture(&state.board);
        let to_call = state.amount_to_call(Player::Bot);

        if to_call == 0 {
            self.postflop_bet_or_check(adjusted, texture, state)
        } else {
            self.postflop_facing_bet(adjusted, to_call, state)
        }
    }

    fn postflop_bet_or_check(
        &self,
        adjusted: f64,
        texture: BoardTexture,
        state: &GameState,
    ) -> Action {
        let mut rng = rand::thread_rng();

        if adjusted > 0.45 {
            return self.make_bet(BetSize::Large, state);
        }

        if adjusted > 0.25 {
            let size = match texture {
                BoardTexture::Dry => BetSize::Small,
                BoardTexture::Medium => BetSize::Medium,
                BoardTexture::Wet => BetSize::Large,
            };
            return self.make_bet(size, state);
        }

        if adjusted > 0.15 && self.aggression > 0.4 {
            return self.make_bet(BetSize::Small, state);
        }

        if adjusted < 0.10 && self.aggression > 0.6 && rng.gen_bool(0.20) {
            let size = match texture {
                BoardTexture::Dry => BetSize::Small,
                _ => BetSize::Medium,
            };
            return self.make_bet(size, state);
        }

        Action::Check
    }

    // ── River ───────────────────────────────────────────────

    fn decide_river(&self, state: &GameState) -> Action {
        let made = evaluate_hand(&state.bot_cards, &state.board).strength();
        let adjusted = self.adjust_strength(made, state);
        let to_call = state.amount_to_call(Player::Bot);

        if to_call == 0 {
            self.river_bet_or_check(adjusted, state)
        } else {
            self.postflop_facing_bet(adjusted, to_call, state)
        }
    }

    fn river_bet_or_check(&self, adjusted: f64, state: &GameState) -> Action {
        let mut rng = rand::thread_rng();

        if adjusted > 0.45 {
            return self.make_bet(BetSize::Large, state);
        }
        if adjusted > 0.20 {
            return self.make_bet(BetSize::Small, state);
        }
        if adjusted < 0.08 && self.aggression > 0.6 && rng.gen_bool(0.15) {
            return self.make_bet(BetSize::Large, state);
        }
        Action::Check
    }

    // ── Facing a bet (all postflop streets) ─────────────────

    fn postflop_facing_bet(&self, adjusted: f64, to_call: u32, state: &GameState) -> Action {
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;
        let mut rng = rand::thread_rng();

        if adjusted > 0.35 {
            if let Some(min_raise) = available.min_raise {
                let raise_to = self.calculate_raise_size(min_raise, state.pot, stack, bot_bet);
                if raise_to >= max_bet {
                    return Action::AllIn(max_bet);
                }
                return Action::Raise(raise_to);
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.20 {
            if available.min_raise.is_some() && self.aggression > 0.5 && rng.gen_bool(0.30) {
                let min_raise = available.min_raise.unwrap();
                let raise_to = self.calculate_raise_size(min_raise, state.pot, stack, bot_bet);
                if raise_to < max_bet {
                    return Action::Raise(raise_to);
                }
            }
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted > 0.12 {
            return self.make_call(to_call, stack, bot_bet);
        }

        if adjusted < 0.08 && self.aggression > 0.7 && rng.gen_bool(0.10) {
            if let Some(min_raise) = available.min_raise {
                let raise_to = self.calculate_raise_size(min_raise, state.pot, stack, bot_bet);
                if raise_to < max_bet {
                    return Action::Raise(raise_to);
                }
            }
        }

        Action::Fold
    }

    // ── Helpers ─────────────────────────────────────────────

    fn adjust_strength(&self, effective: f64, state: &GameState) -> f64 {
        let mut rng = rand::thread_rng();
        let noise: f64 = rng.gen_range(-0.05..0.05);
        let position = if state.button == Player::Bot {
            0.06 // In position postflop (button acts last)
        } else {
            -0.04 // Out of position
        };
        let aggression_adj = (self.aggression - 0.5) * 0.12;
        effective + position + aggression_adj + noise
    }

    fn make_bet(&self, size: BetSize, state: &GameState) -> Action {
        let available = state.available_actions();
        let stack = state.bot_stack;
        let bot_bet = state.bot_bet;
        let max_bet = bot_bet + stack;

        let min_bet = match available.min_bet {
            Some(v) => v,
            None => return Action::Check,
        };

        let raw = (state.pot as f64 * size.pot_fraction()) as u32;
        let amount = raw.max(min_bet).min(stack);

        if amount >= stack {
            Action::AllIn(max_bet)
        } else {
            Action::Bet(amount)
        }
    }

    fn make_call(&self, to_call: u32, stack: u32, bot_bet: u32) -> Action {
        if to_call >= stack {
            Action::AllIn(bot_bet + stack)
        } else {
            Action::Call(to_call)
        }
    }

    fn calculate_raise_size(&self, min_raise_to: u32, pot: u32, stack: u32, bot_bet: u32) -> u32 {
        let raise_to = (pot as f64 * 0.70) as u32 + bot_bet;
        let max_bet = bot_bet + stack;
        raise_to.max(min_raise_to).min(max_bet)
    }
}

// ── Board texture analysis ──────────────────────────────────

fn analyze_board_texture(board: &[Card]) -> BoardTexture {
    if board.is_empty() {
        return BoardTexture::Dry;
    }

    let mut wetness: i32 = 0;

    // Suit concentration
    let mut suit_counts = [0u8; 4];
    for card in board {
        let idx = match card.suit {
            Suit::Spades => 0,
            Suit::Hearts => 1,
            Suit::Diamonds => 2,
            Suit::Clubs => 3,
        };
        suit_counts[idx] += 1;
    }
    let max_suit = *suit_counts.iter().max().unwrap();
    if max_suit >= 3 {
        wetness += 2; // Monotone or near-monotone
    } else if max_suit == 2 {
        wetness += 1; // Two-tone
    }

    // Rank connectivity: count adjacent-ish card pairs
    let mut ranks: Vec<u8> = board.iter().map(|c| c.rank as u8).collect();
    ranks.sort();
    for window in ranks.windows(2) {
        if window[1] - window[0] <= 2 {
            wetness += 1;
        }
    }

    // Paired board
    if ranks.windows(2).any(|w| w[0] == w[1]) {
        wetness += 1;
    }

    match wetness {
        0..=1 => BoardTexture::Dry,
        2..=3 => BoardTexture::Medium,
        _ => BoardTexture::Wet,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deck::{Card, Rank, Suit};

    #[test]
    fn test_bot_creation() {
        let bot = RuleBasedBot::new(0.5);
        assert!((bot.aggression - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_aggression_clamping() {
        let bot = RuleBasedBot::new(1.5);
        assert!((bot.aggression - 1.0).abs() < f64::EPSILON);

        let bot = RuleBasedBot::new(-0.5);
        assert!(bot.aggression.abs() < f64::EPSILON);
    }

    // ── Board texture ───────────────────────────────────────

    #[test]
    fn test_board_texture_dry() {
        // K♠ 7♥ 2♦ — rainbow, spread ranks
        let board = vec![
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Seven, Suit::Hearts),
            Card::new(Rank::Two, Suit::Diamonds),
        ];
        assert_eq!(analyze_board_texture(&board), BoardTexture::Dry);
    }

    #[test]
    fn test_board_texture_wet() {
        // J♥ T♥ 9♥ — monotone + connected
        let board = vec![
            Card::new(Rank::Jack, Suit::Hearts),
            Card::new(Rank::Ten, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
        ];
        assert_eq!(analyze_board_texture(&board), BoardTexture::Wet);
    }

    #[test]
    fn test_board_texture_medium() {
        // K♥ 7♠ 5♠ — two-tone, some connectivity
        let board = vec![
            Card::new(Rank::King, Suit::Hearts),
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Five, Suit::Spades),
        ];
        assert_eq!(analyze_board_texture(&board), BoardTexture::Medium);
    }

    // ── Postflop decision tests ─────────────────────────────

    /// Create a postflop state where the bot faces a bet.
    fn facing_bet_state(
        bot_cards: Vec<Card>,
        board: Vec<Card>,
        phase: GamePhase,
        pot: u32,
        player_bet: u32,
        bot_is_ip: bool,
    ) -> GameState {
        let mut state = GameState::new(100);
        state.phase = phase;
        state.bot_cards = bot_cards;
        state.board = board;
        state.pot = pot;
        state.player_bet = player_bet;
        state.bot_bet = 0;
        state.to_act = Player::Bot;
        state.button = if bot_is_ip { Player::Bot } else { Player::Human };
        state.bot_stack = 180;
        state.player_stack = 180;
        state.last_aggressor = Some(Player::Human);
        state.last_raise_size = player_bet;
        state
    }

    #[test]
    fn test_trips_facing_bet_never_folds() {
        // Trip Kings: made strength ≈ 0.47, adjusted OOP ≈ 0.43 ± 0.05
        // Even worst case 0.38 >> fold threshold (0.12)
        let bot = RuleBasedBot::new(0.5);
        let bot_cards = vec![
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
        ];
        let board = vec![
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Two, Suit::Spades),
            Card::new(Rank::Nine, Suit::Hearts),
        ];

        for _ in 0..50 {
            let state = facing_bet_state(
                bot_cards.clone(),
                board.clone(),
                GamePhase::Turn,
                40,
                10,
                false, // OOP — harder case
            );
            let action = bot.decide(&state);
            assert_ne!(action, Action::Fold, "Trips should never fold to a bet");
        }
    }

    #[test]
    fn test_air_oop_facing_bet_folds() {
        // 7♠ 2♥ on K♦ Q♣ 4♠ 9♥ — high card, no draws (rainbow, disconnected)
        // strength ≈ 0.092, adjusted OOP ≈ 0.052 ± 0.05, max 0.102 < 0.12
        let bot = RuleBasedBot::new(0.5);
        let bot_cards = vec![
            Card::new(Rank::Seven, Suit::Spades),
            Card::new(Rank::Two, Suit::Hearts),
        ];
        let board = vec![
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Queen, Suit::Clubs),
            Card::new(Rank::Four, Suit::Spades),
            Card::new(Rank::Nine, Suit::Hearts),
        ];

        for _ in 0..50 {
            let state = facing_bet_state(
                bot_cards.clone(),
                board.clone(),
                GamePhase::Turn,
                40,
                10,
                false, // OOP
            );
            let action = bot.decide(&state);
            assert_eq!(action, Action::Fold, "Air OOP should fold to a bet");
        }
    }

    #[test]
    fn test_top_pair_facing_bet_calls() {
        // K♠ 7♥ on K♦ 5♣ 2♠ 9♥ — top pair
        // strength ≈ 0.22, adjusted IP ≈ 0.28 ± 0.05, min 0.23 > 0.12
        let bot = RuleBasedBot::new(0.5);
        let bot_cards = vec![
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::Seven, Suit::Hearts),
        ];
        let board = vec![
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Clubs),
            Card::new(Rank::Two, Suit::Spades),
            Card::new(Rank::Nine, Suit::Hearts),
        ];

        for _ in 0..50 {
            let state = facing_bet_state(
                bot_cards.clone(),
                board.clone(),
                GamePhase::Turn,
                40,
                10,
                true, // IP
            );
            let action = bot.decide(&state);
            assert_ne!(action, Action::Fold, "Top pair IP should not fold to a bet");
        }
    }

    #[test]
    fn test_flush_draw_on_flop_calls() {
        // 8♥ 9♥ on 2♥ 5♥ K♠ — flush draw
        // effective ≈ 0.09 + 0.18 = 0.27, adjusted IP ≈ 0.33 ± 0.05, min 0.28 > 0.12
        let bot = RuleBasedBot::new(0.5);
        let bot_cards = vec![
            Card::new(Rank::Eight, Suit::Hearts),
            Card::new(Rank::Nine, Suit::Hearts),
        ];
        let board = vec![
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Five, Suit::Hearts),
            Card::new(Rank::King, Suit::Spades),
        ];

        for _ in 0..50 {
            let state = facing_bet_state(
                bot_cards.clone(),
                board.clone(),
                GamePhase::Flop,
                30,
                10,
                true, // IP
            );
            let action = bot.decide(&state);
            assert_ne!(
                action,
                Action::Fold,
                "Flush draw on flop should not fold to a bet"
            );
        }
    }
}
