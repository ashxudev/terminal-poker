use crate::game::actions::Action;
use crate::game::state::{GameState, Player, BIG_BLIND};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key(
    key: KeyEvent,
    game_state: &GameState,
    raise_input: &mut String,
    raise_mode: &mut bool,
) -> Option<Action> {
    if !game_state.is_player_turn() {
        return None;
    }

    let available = game_state.available_actions();
    let stack = game_state.player_stack;

    // When in raise mode, only raise-related keys are accepted
    if *raise_mode {
        return handle_raise_mode_key(key, game_state, raise_input, raise_mode);
    }

    match key.code {
        // Fold
        KeyCode::Char('f') | KeyCode::Char('F') => {
            if available.can_fold {
                Some(Action::Fold)
            } else {
                None
            }
        }

        // Call / Check
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

        // Enter raise mode
        KeyCode::Char('r') | KeyCode::Char('R') => {
            if available.min_raise.is_some() || available.min_bet.is_some() {
                *raise_mode = true;
                raise_input.clear();
            }
            None
        }

        _ => None,
    }
}

fn handle_raise_mode_key(
    key: KeyEvent,
    game_state: &GameState,
    raise_input: &mut String,
    raise_mode: &mut bool,
) -> Option<Action> {
    let available = game_state.available_actions();
    let to_call = game_state.amount_to_call(Player::Human);
    let stack = game_state.player_stack;

    match key.code {
        // Digits: append to BB input
        KeyCode::Char(c) if c.is_ascii_digit() => {
            raise_input.push(c);
            None
        }

        // Backspace: delete last digit
        KeyCode::Backspace => {
            raise_input.pop();
            None
        }

        // Up arrow: +1BB
        KeyCode::Up => {
            let current_bb = raise_input.parse::<u32>().unwrap_or(0);
            let min_bb = min_raise_bb(&available);
            let max_bb = (game_state.player_bet + stack) / BIG_BLIND;
            let new_bb = (current_bb + 1).min(max_bb).max(min_bb);
            *raise_input = new_bb.to_string();
            None
        }

        // Down arrow: -1BB
        KeyCode::Down => {
            let current_bb = raise_input.parse::<u32>().unwrap_or(0);
            let min_bb = min_raise_bb(&available);
            let new_bb = if current_bb > min_bb {
                current_bb - 1
            } else {
                min_bb
            };
            *raise_input = new_bb.to_string();
            None
        }

        // Enter or R: confirm raise
        KeyCode::Enter | KeyCode::Char('r') | KeyCode::Char('R') => {
            if let Some(action) = submit_raise(raise_input, game_state, &available, to_call, stack)
            {
                *raise_mode = false;
                raise_input.clear();
                return Some(action);
            }
            None
        }

        // Escape: cancel raise mode
        KeyCode::Esc => {
            *raise_mode = false;
            raise_input.clear();
            None
        }

        // All other keys ignored in raise mode
        _ => None,
    }
}

fn min_raise_bb(available: &crate::game::actions::AvailableActions) -> u32 {
    let min_chips = available
        .min_raise
        .unwrap_or(available.min_bet.unwrap_or(BIG_BLIND));
    // Convert chips to BB, rounding up
    (min_chips + BIG_BLIND - 1) / BIG_BLIND
}

fn submit_raise(
    raise_input: &str,
    game_state: &GameState,
    available: &crate::game::actions::AvailableActions,
    to_call: u32,
    stack: u32,
) -> Option<Action> {
    let typed_bb = raise_input.parse::<u32>().ok()?;
    if typed_bb == 0 {
        return None;
    }

    let chips = typed_bb * BIG_BLIND;
    let min_raise = available
        .min_raise
        .unwrap_or(available.min_bet.unwrap_or(BIG_BLIND));
    let max_bet = game_state.player_bet + stack;

    let actual = chips.max(min_raise).min(max_bet);

    if actual >= max_bet {
        Some(Action::AllIn(max_bet))
    } else if to_call > 0 {
        Some(Action::Raise(actual))
    } else {
        Some(Action::Bet(actual))
    }
}
