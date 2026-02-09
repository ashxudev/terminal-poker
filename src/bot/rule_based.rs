use crate::game::actions::Action;
use crate::game::hand::evaluate_hand;
use crate::game::state::{GameState, Player, BIG_BLIND};
use rand::Rng;

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
        let hand_strength = self.evaluate_strength(state);
        let pot_odds = self.calculate_pot_odds(state);
        let position_bonus = self.position_bonus(state);

        let to_call = state.amount_to_call(Player::Bot);
        let stack = state.bot_stack;
        let is_preflop = state.board.is_empty();
        let available = state.available_actions();
        // Use Option to properly handle cases where no raise/bet is available
        let min_raise_to = available.min_raise.or(available.min_bet);

        // Effective threshold combines hand strength, position, and aggression
        let threshold = hand_strength + position_bonus + (self.aggression - 0.5) * 0.15;

        // Add randomness for unpredictability
        let mut rng = rand::thread_rng();
        let noise: f64 = rng.gen_range(-0.05..0.05);
        let adjusted_threshold = threshold + noise;

        // Decision logic
        if to_call == 0 {
            // Can check - decide whether to bet/raise
            self.decide_bet(
                adjusted_threshold,
                stack,
                state.pot,
                state.bot_bet,
                is_preflop,
                min_raise_to,
            )
        } else {
            // Facing a bet
            self.decide_facing_bet(
                adjusted_threshold,
                pot_odds,
                to_call,
                stack,
                state.pot,
                state.bot_bet,
                min_raise_to,
            )
        }
    }

    fn evaluate_strength(&self, state: &GameState) -> f64 {
        let eval = evaluate_hand(&state.bot_cards, &state.board);
        eval.strength()
    }

    fn calculate_pot_odds(&self, state: &GameState) -> f64 {
        let to_call = state.amount_to_call(Player::Bot);
        if to_call == 0 {
            return 0.0;
        }
        let pot_after = state.pot + to_call;
        to_call as f64 / pot_after as f64
    }

    fn position_bonus(&self, state: &GameState) -> f64 {
        // In position (button) gives an advantage postflop
        let is_preflop = state.board.is_empty();
        if is_preflop {
            // Preflop: button acts first (slight disadvantage)
            if state.button == Player::Bot {
                -0.02
            } else {
                0.02
            }
        } else {
            // Postflop: button acts last (advantage)
            if state.button == Player::Bot {
                0.08
            } else {
                -0.03
            }
        }
    }

    fn decide_bet(
        &self,
        threshold: f64,
        stack: u32,
        pot: u32,
        bot_bet: u32,
        is_preflop: bool,
        min_raise_to: Option<u32>,
    ) -> Action {
        // If no bet/raise is available, just check
        let min_raise_to = match min_raise_to {
            Some(v) => v,
            None => return Action::Check,
        };

        let max_bet = bot_bet + stack;

        // Helper to create the correct action type
        // Preflop with existing bet (BB) requires Raise, not Bet
        let make_bet_action = |bet_size: u32| -> Action {
            if is_preflop && bot_bet > 0 {
                // This is a raise over our blind
                let raise_to = (bot_bet + bet_size).max(min_raise_to);
                if raise_to >= max_bet {
                    Action::AllIn(max_bet)
                } else {
                    Action::Raise(raise_to)
                }
            } else {
                // Normal bet (postflop or no existing bet)
                if bet_size >= stack {
                    Action::AllIn(max_bet)
                } else {
                    Action::Bet(bet_size)
                }
            }
        };

        if threshold > 0.75 {
            // Strong hand - value bet
            let bet_size = self.calculate_bet_size(0.67, pot, stack);
            make_bet_action(bet_size)
        } else if threshold > 0.55 && self.aggression > 0.4 {
            // Medium hand with aggression - smaller bet
            let bet_size = self.calculate_bet_size(0.5, pot, stack);
            make_bet_action(bet_size)
        } else if threshold < 0.3 && self.aggression > 0.6 {
            // Weak hand but aggressive - occasional bluff
            let mut rng = rand::thread_rng();
            if rng.gen_bool(0.2) {
                let bet_size = self.calculate_bet_size(0.5, pot, stack);
                return make_bet_action(bet_size);
            }
            Action::Check
        } else {
            Action::Check
        }
    }

    fn decide_facing_bet(
        &self,
        threshold: f64,
        pot_odds: f64,
        to_call: u32,
        stack: u32,
        pot: u32,
        bot_bet: u32,
        min_raise_to: Option<u32>,
    ) -> Action {
        let max_bet = bot_bet + stack;
        let can_raise = min_raise_to.is_some();

        // Very strong hand - raise for value (if possible) or call
        if threshold > 0.8 {
            if let Some(min_raise) = min_raise_to {
                let raise_to = self.calculate_raise_size(min_raise, pot, stack, bot_bet);
                if raise_to >= max_bet {
                    return Action::AllIn(max_bet);
                }
                return Action::Raise(raise_to);
            }
            // Can't raise, just call
            if to_call <= stack {
                return Action::Call(to_call);
            }
            return Action::AllIn(max_bet);
        }

        // Strong hand - call or raise
        if threshold > 0.6 {
            if can_raise && self.aggression > 0.5 {
                let mut rng = rand::thread_rng();
                if rng.gen_bool(0.3) {
                    let min_raise = min_raise_to.unwrap();
                    let raise_to = self.calculate_raise_size(min_raise, pot, stack, bot_bet);
                    if raise_to < max_bet {
                        return Action::Raise(raise_to);
                    }
                }
            }
            if to_call <= stack {
                return Action::Call(to_call);
            }
            return Action::AllIn(max_bet);
        }

        // Medium hand - call if pot odds are good
        if threshold > pot_odds {
            if to_call <= stack {
                return Action::Call(to_call);
            }
            return Action::AllIn(max_bet);
        }

        // Weak hand but consider bluff-raise
        if can_raise && self.aggression > 0.7 && threshold < 0.25 {
            let mut rng = rand::thread_rng();
            if rng.gen_bool(0.1) {
                let min_raise = min_raise_to.unwrap();
                let raise_to = self.calculate_raise_size(min_raise, pot, stack, bot_bet);
                if raise_to < max_bet {
                    return Action::Raise(raise_to);
                }
            }
        }

        Action::Fold
    }

    fn calculate_bet_size(&self, pot_fraction: f64, pot: u32, stack: u32) -> u32 {
        let base = (pot as f64 * pot_fraction) as u32;
        let min_bet = BIG_BLIND;
        base.max(min_bet).min(stack)
    }

    fn calculate_raise_size(&self, min_raise_to: u32, pot: u32, stack: u32, bot_bet: u32) -> u32 {
        // Calculate desired raise as ~67% of pot
        let raise_to = (pot as f64 * 0.67) as u32 + bot_bet;
        // Ensure we meet minimum raise requirement and don't exceed our stack
        let max_bet = bot_bet + stack;
        raise_to.max(min_raise_to).min(max_bet)
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
}
