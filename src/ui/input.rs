use crate::game::actions::Action;
use crate::game::state::{GameState, Player};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key(
    key: KeyEvent,
    game_state: &GameState,
    raise_input: &mut String,
) -> Option<Action> {
    if !game_state.is_player_turn() {
        return None;
    }

    let available = game_state.available_actions();
    let to_call = game_state.amount_to_call(Player::Human);
    let stack = game_state.player_stack;

    match key.code {
        // Fold
        KeyCode::Char('f') | KeyCode::Char('F') => {
            if available.can_fold {
                Some(Action::Fold)
            } else {
                None
            }
        }

        // Check
        KeyCode::Char('x') | KeyCode::Char('X') => {
            if available.can_check {
                Some(Action::Check)
            } else {
                None
            }
        }

        // Call
        KeyCode::Char('c') | KeyCode::Char('C') => {
            if let Some(amount) = available.can_call {
                Some(Action::Call(amount))
            } else if available.can_check {
                Some(Action::Check)
            } else {
                None
            }
        }

        // All-in
        KeyCode::Char('a') | KeyCode::Char('A') => {
            if stack > 0 {
                Some(Action::AllIn(game_state.player_bet + stack))
            } else {
                None
            }
        }

        // Raise/Bet - enter raise mode or use last input
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if !raise_input.is_empty() {
                if let Ok(amount) = raise_input.parse::<u32>() {
                    raise_input.clear();
                    let min_raise = available.min_raise.unwrap_or(available.min_bet.unwrap_or(2));
                    let max_bet = game_state.player_bet + stack;
                    let actual = amount.max(min_raise).min(max_bet);
                    if actual >= max_bet {
                        return Some(Action::AllIn(max_bet));
                    }
                    if to_call > 0 {
                        return Some(Action::Raise(actual));
                    } else {
                        return Some(Action::Bet(actual));
                    }
                }
            }
            None
        }

        // Pot-sized bet shortcuts - MUST come before general digit handler
        KeyCode::Char('1') if raise_input.is_empty() => {
            // 33% pot raise
            let raise_size = (game_state.pot as f64 * 0.33) as u32;
            pot_sized_action(raise_size, &available, game_state.player_bet, stack, to_call)
        }
        KeyCode::Char('2') if raise_input.is_empty() => {
            // 50% pot raise
            let raise_size = (game_state.pot as f64 * 0.5) as u32;
            pot_sized_action(raise_size, &available, game_state.player_bet, stack, to_call)
        }
        KeyCode::Char('3') if raise_input.is_empty() => {
            // 67% pot raise
            let raise_size = (game_state.pot as f64 * 0.67) as u32;
            pot_sized_action(raise_size, &available, game_state.player_bet, stack, to_call)
        }
        KeyCode::Char('4') if raise_input.is_empty() => {
            // 100% pot raise
            pot_sized_action(game_state.pot, &available, game_state.player_bet, stack, to_call)
        }

        // Numeric input for raise amount - AFTER specific shortcuts
        KeyCode::Char(c) if c.is_ascii_digit() => {
            raise_input.push(c);
            None
        }

        // Backspace to clear raise input
        KeyCode::Backspace => {
            raise_input.pop();
            None
        }

        // Enter to submit raise
        KeyCode::Enter => {
            if !raise_input.is_empty() {
                if let Ok(amount) = raise_input.parse::<u32>() {
                    raise_input.clear();
                    let min_raise = available.min_raise.unwrap_or(available.min_bet.unwrap_or(2));
                    let max_bet = game_state.player_bet + stack;
                    let actual = amount.max(min_raise).min(max_bet);
                    if actual >= max_bet {
                        return Some(Action::AllIn(max_bet));
                    }
                    if to_call > 0 {
                        return Some(Action::Raise(actual));
                    } else {
                        return Some(Action::Bet(actual));
                    }
                }
            }
            None
        }

        _ => None,
    }
}

/// Creates a pot-sized bet or raise action.
/// `raise_size` is the amount to raise BY (X% of pot), not the total.
fn pot_sized_action(
    raise_size: u32,
    available: &crate::game::actions::AvailableActions,
    player_bet: u32,
    stack: u32,
    to_call: u32,
) -> Option<Action> {
    let min_raise = available.min_raise.unwrap_or(available.min_bet.unwrap_or(2));
    let max_bet = player_bet + stack;

    // Calculate raise-to amount: current max bet + raise size
    // current_max_bet = player_bet + to_call (what opponent has bet)
    let current_max_bet = player_bet + to_call;
    let raise_to = (current_max_bet + raise_size).max(min_raise).min(max_bet);

    if raise_to >= max_bet {
        return Some(Action::AllIn(max_bet));
    }

    if available.can_call.is_some() {
        Some(Action::Raise(raise_to))
    } else {
        Some(Action::Bet(raise_to))
    }
}
