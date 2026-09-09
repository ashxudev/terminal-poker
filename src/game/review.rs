//! Deterministic, player-authorized executable evidence for sprint reviews.
//!
//! This module is not a production replay or protocol format. It intentionally
//! exposes the public board, the local seat's cards, and showdown-revealed cards
//! while keeping the remaining deck and pre-showdown opponent cards private.

use serde::Serialize;

use super::actions::Action;
use super::command::SeatCommand;
use super::deck::{Card, Suit};
use super::seat::SeatId;
use super::state::{GamePhase, GameState, BIG_BLIND, SMALL_BLIND};

pub const REVIEW_SEED: u64 = 20_260_830;
pub const BUILD_ID: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "-v",
    env!("CARGO_PKG_VERSION"),
    "-sprint2-review-v2"
);
pub const FIXTURE_ID: &str = "deterministic-heads-up-command-path";
pub const HAND_ID: &str = "fixture-hand-0001";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewFrame {
    pub sequence: u8,
    pub screenshot_stem: String,
    pub phase: String,
    pub actor: String,
    pub accepted_action_or_event: String,
    pub board: String,
    pub pot: u32,
    pub local_stack: u32,
    pub local_contribution: u32,
    pub bot_stack: u32,
    pub bot_contribution: u32,
    pub terminal_view: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReviewEvidence {
    pub build_id: String,
    pub fixture_id: String,
    pub hand_id: String,
    pub seed: u64,
    pub rejection_error: String,
    pub rejected_state_unchanged: bool,
    pub rejection_view: String,
    pub frames: Vec<ReviewFrame>,
}

pub fn run_deterministic_review_hand() -> ReviewEvidence {
    let local = seat(0);
    let bot = seat(1);
    let mut state = GameState::new_seeded_for_review(100, REVIEW_SEED);

    let before_rejection = authorized_signature(&state, local);
    let rejection = state
        .apply_command(SeatCommand::new(bot, Action::Check))
        .expect_err("the bot is out of turn in the initial fixture state");
    let after_rejection = authorized_signature(&state, local);
    let rejected_state_unchanged = before_rejection == after_rejection;
    let rejection_error = rejection.to_string();
    let rejection_view =
        format_rejection_view(&state, local, &rejection_error, rejected_state_unchanged);

    let mut frames = vec![capture_frame(
        1,
        "01-preflop",
        &state,
        local,
        "-",
        "Blinds posted; deterministic local cards dealt",
    )];
    let mut transition_actions = Vec::new();

    while matches!(
        state.phase,
        GamePhase::Preflop | GamePhase::Flop | GamePhase::Turn | GamePhase::River
    ) {
        let actor = state.to_act;
        let to_call = state.amount_to_call(actor);
        let action = if to_call > 0 {
            Action::Call(to_call)
        } else {
            Action::Check
        };
        let prior_phase = state.phase;
        state
            .apply_command(SeatCommand::new(actor, action))
            .expect("the scripted passive action must be legal");
        transition_actions.push(format_action(actor, action));

        if state.phase != prior_phase {
            let event = if state.phase == GamePhase::Showdown {
                format!(
                    "{}; showdown resolved and pot awarded",
                    transition_actions.join("; ")
                )
            } else {
                format!(
                    "{}; {} dealt",
                    transition_actions.join("; "),
                    phase_name(state.phase)
                )
            };
            let sequence = frames.len() as u8 + 1;
            let stem = format!("{sequence:02}-{}", phase_name(state.phase).to_lowercase());
            frames.push(capture_frame(
                sequence,
                &stem,
                &state,
                local,
                "Automatic transition",
                &event,
            ));
            transition_actions.clear();
        }
    }

    ReviewEvidence {
        build_id: BUILD_ID.to_string(),
        fixture_id: FIXTURE_ID.to_string(),
        hand_id: HAND_ID.to_string(),
        seed: REVIEW_SEED,
        rejection_error,
        rejected_state_unchanged,
        rejection_view,
        frames,
    }
}

fn capture_frame(
    sequence: u8,
    screenshot_stem: &str,
    state: &GameState,
    local: SeatId,
    actor: &str,
    event: &str,
) -> ReviewFrame {
    let bot = seat(1);
    let reveal_bot = state.phase == GamePhase::Showdown;
    let board = cards_text(&state.board);
    let terminal_view = format_terminal_view(state, local, reveal_bot, event);

    ReviewFrame {
        sequence,
        screenshot_stem: screenshot_stem.to_string(),
        phase: phase_name(state.phase).to_string(),
        actor: actor.to_string(),
        accepted_action_or_event: event.to_string(),
        board,
        pot: state.pot,
        local_stack: state.stack(local),
        local_contribution: state.street_bet(local),
        bot_stack: state.stack(bot),
        bot_contribution: state.street_bet(bot),
        terminal_view,
    }
}

fn format_terminal_view(state: &GameState, local: SeatId, reveal_bot: bool, event: &str) -> String {
    let bot = seat(1);
    let to_act = if matches!(
        state.phase,
        GamePhase::Preflop | GamePhase::Flop | GamePhase::Turn | GamePhase::River
    ) {
        format!("Seat {}", state.to_act.as_u8())
    } else {
        "-".to_string()
    };
    let bot_cards = if reveal_bot {
        cards_text(state.hole_cards(bot))
    } else {
        "?? ??".to_string()
    };
    let outcome = state
        .showdown_result
        .as_ref()
        .map(|result| {
            let winner = match result.winner {
                Some(winner) => format!("Seat {}", winner.as_u8()),
                None => "Split pot".to_string(),
            };
            format!(
                "\nSHOWDOWN  Winner: {winner}  Awarded: {}\nHANDS     Local: {} | Opp: {}",
                result.pot_won,
                result
                    .hand_for(local)
                    .map(|hand| hand.description.as_str())
                    .unwrap_or("-"),
                result
                    .hand_for(bot)
                    .map(|hand| hand.description.as_str())
                    .unwrap_or("-")
            )
        })
        .unwrap_or_default();

    format!(
        "==============================================================================\n\
         TERMINAL POKER - EXECUTABLE SPRINT REVIEW\n\
         BUILD    {BUILD_ID}\n\
         FIXTURE  {FIXTURE_ID}\n\
         HAND     {HAND_ID}    SEED {REVIEW_SEED}\n\
         ------------------------------------------------------------------------------\n\
         PHASE    {phase}    BOARD {board}    POT {pot}\n\
         BUTTON   Seat {button}    TO ACT {to_act}    BLINDS {SMALL_BLIND}/{BIG_BLIND}\n\
         ------------------------------------------------------------------------------\n\
         Seat 0   LOCAL   stack {local_stack}   committed {local_bet}   cards {local_cards}\n\
         Seat 1   BOT     stack {bot_stack}   committed {bot_bet}   cards {bot_cards}\n\
         ------------------------------------------------------------------------------\n\
         ACCEPTED {event}\n\
         ==============================================================================\
         {outcome}",
        phase = phase_name(state.phase),
        board = cards_text(&state.board),
        pot = state.pot,
        button = state.button.as_u8(),
        local_stack = state.stack(local),
        local_bet = state.street_bet(local),
        local_cards = cards_text(state.hole_cards(local)),
        bot_stack = state.stack(bot),
        bot_bet = state.street_bet(bot),
    )
}

fn format_rejection_view(
    state: &GameState,
    local: SeatId,
    rejection: &str,
    unchanged: bool,
) -> String {
    format!(
        "==============================================================================\n\
         VALIDATED SEAT COMMAND - EXECUTABLE REJECTION PROOF\n\
         BUILD    {BUILD_ID}\n\
         FIXTURE  {FIXTURE_ID}\n\
         HAND     {HAND_ID}    SEED {REVIEW_SEED}\n\
         ------------------------------------------------------------------------------\n\
         SUBMIT   Seat 1 -> Check\n\
         EXPECT   Seat {actor}\n\
         REJECT   {rejection}\n\
         NO MUTATION   {result}\n\
         ------------------------------------------------------------------------------\n\
         AUTHORIZED STATE\n\
         {signature}\n\
         ==============================================================================",
        actor = state.to_act.as_u8(),
        result = if unchanged { "PASS" } else { "FAIL" },
        signature = authorized_signature(state, local),
    )
}

fn authorized_signature(state: &GameState, local: SeatId) -> String {
    let bot = seat(1);
    format!(
        "{}|pot={}|actor={}|board={}|s0={}/{}|s1={}/{}|local={}",
        phase_name(state.phase),
        state.pot,
        state.to_act.as_u8(),
        cards_text(&state.board),
        state.stack(local),
        state.street_bet(local),
        state.stack(bot),
        state.street_bet(bot),
        cards_text(state.hole_cards(local))
    )
}

fn format_action(actor: SeatId, action: Action) -> String {
    let action = match action {
        Action::Fold => "folds".to_string(),
        Action::Check => "checks".to_string(),
        Action::Call(amount) => format!("calls {amount}"),
        Action::Bet(amount) => format!("bets to {amount}"),
        Action::Raise(amount) => format!("raises to {amount}"),
        Action::AllIn(amount) => format!("all-in to {amount}"),
    };
    format!("Seat {} {action}", actor.as_u8())
}

fn cards_text(cards: &[Card]) -> String {
    if cards.is_empty() {
        return "-".to_string();
    }
    cards.iter().map(card_text).collect::<Vec<_>>().join(" ")
}

fn card_text(card: &Card) -> String {
    let suit = match card.suit {
        Suit::Spades => 'S',
        Suit::Hearts => 'H',
        Suit::Diamonds => 'D',
        Suit::Clubs => 'C',
    };
    format!("{}{suit}", card.rank.symbol())
}

fn phase_name(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::Preflop => "Preflop",
        GamePhase::Flop => "Flop",
        GamePhase::Turn => "Turn",
        GamePhase::River => "River",
        GamePhase::Showdown => "Showdown",
        GamePhase::HandComplete => "Complete",
        GamePhase::SessionEnd => "SessionEnd",
        GamePhase::Summary => "Summary",
    }
}

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("review fixture seat is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_hand_is_reproducible_from_initial_state_through_showdown() {
        let first = run_deterministic_review_hand();
        let second = run_deterministic_review_hand();

        assert_eq!(first, second);
        assert!(first.rejected_state_unchanged);
        assert_eq!(
            first
                .frames
                .iter()
                .map(|frame| frame.phase.as_str())
                .collect::<Vec<_>>(),
            ["Preflop", "Flop", "Turn", "River", "Showdown"]
        );
        assert_eq!(first.frames.last().unwrap().pot, 0);
        assert_eq!(
            first.frames.last().unwrap().local_stack + first.frames.last().unwrap().bot_stack,
            400
        );
    }

    #[test]
    fn review_views_preserve_card_privacy_until_showdown() {
        let evidence = run_deterministic_review_hand();

        for frame in &evidence.frames[..evidence.frames.len() - 1] {
            assert!(frame.terminal_view.contains("cards ?? ??"));
            assert!(!frame.terminal_view.contains("SHOWDOWN  Winner:"));
        }
        let final_frame = evidence.frames.last().unwrap();
        assert!(!final_frame.terminal_view.contains("cards ?? ??"));
        assert!(final_frame.terminal_view.contains("SHOWDOWN  Winner:"));
        assert!(!evidence.rejection_view.to_lowercase().contains("deck"));
    }

    #[test]
    fn every_review_frame_conserves_the_initial_chip_total() {
        let evidence = run_deterministic_review_hand();

        for frame in evidence.frames {
            assert_eq!(frame.pot + frame.local_stack + frame.bot_stack, 400);
        }
    }
}
