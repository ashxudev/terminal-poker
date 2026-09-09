use terminal_poker::game::actions::Action;
use terminal_poker::game::command::SeatCommand;
use terminal_poker::game::seat::SeatId;
use terminal_poker::game::state::GameState;

fn apply_action(state: &mut GameState, seat: SeatId, action: Action) {
    state
        .apply_command(SeatCommand::new(seat, action))
        .expect("integration-test action must be legal");
}

fn local_seat() -> SeatId {
    SeatId::new(0).unwrap()
}

fn bot_seat() -> SeatId {
    SeatId::new(1).unwrap()
}

mod game_logic {
    // Test the game state machine directly
    use std::path::PathBuf;
    use std::process::Command;

    /// Tests that the release binary runs and shows help.
    /// Run with: `cargo build --release && cargo test -- --ignored`
    #[test]
    #[ignore = "requires release binary: run `cargo build --release` first"]
    fn test_binary_runs() {
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("release")
            .join("terminal-poker");

        assert!(
            binary.exists(),
            "Release binary not found at {:?}. Run `cargo build --release` first.",
            binary
        );

        let output = Command::new(&binary)
            .arg("--help")
            .output()
            .expect("Failed to run binary");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("heads-up"));
        assert!(stdout.contains("--stack"));
        assert!(stdout.contains("--aggression"));
    }

    /// Tests that the release binary shows correct version.
    /// Run with: `cargo build --release && cargo test -- --ignored`
    #[test]
    #[ignore = "requires release binary: run `cargo build --release` first"]
    fn test_version() {
        let binary = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("release")
            .join("terminal-poker");

        assert!(
            binary.exists(),
            "Release binary not found at {:?}. Run `cargo build --release` first.",
            binary
        );

        let output = Command::new(&binary)
            .arg("--version")
            .output()
            .expect("Failed to run binary");

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("terminal-poker"));
        assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
    }
}

// Test game state machine
#[cfg(test)]
mod state_machine_tests {
    use super::{apply_action, bot_seat, local_seat};

    #[test]
    fn test_full_hand_to_showdown() {
        // This tests the core game logic path
        use terminal_poker::game::actions::Action;
        use terminal_poker::game::state::{GamePhase, GameState};

        let mut state = GameState::new(100);

        // Verify initial state
        assert_eq!(state.phase, GamePhase::Preflop);
        assert_eq!(state.hole_cards(local_seat()).len(), 2);
        assert_eq!(state.hole_cards(bot_seat()).len(), 2);
        assert!(state.board.is_empty());

        // Both players call/check to showdown
        // This simulates a passive hand
        let mut iterations = 0;
        while !matches!(
            state.phase,
            GamePhase::Showdown | GamePhase::HandComplete | GamePhase::SessionEnd
        ) {
            iterations += 1;
            if iterations > 100 {
                panic!("Game loop stuck");
            }

            let to_call = state.amount_to_call(state.to_act);
            let action = if to_call > 0 {
                Action::Call(to_call)
            } else {
                Action::Check
            };

            let actor = state.to_act;
            apply_action(&mut state, actor, action);
        }

        // Should reach showdown with 5 board cards
        if state.phase == GamePhase::Showdown {
            assert_eq!(state.board.len(), 5);
            assert!(state.showdown_result.is_some());
        }
    }

    #[test]
    fn test_fold_ends_hand() {
        use terminal_poker::game::actions::Action;
        use terminal_poker::game::state::{GamePhase, GameState};

        let mut state = GameState::new(100);

        // First player folds
        let actor = state.to_act;
        apply_action(&mut state, actor, Action::Fold);

        assert_eq!(state.phase, GamePhase::HandComplete);
    }

    #[test]
    fn test_pot_odds_calculation() {
        use terminal_poker::game::state::GameState;

        let state = GameState::new(100);

        // After blinds are posted, there should be pot odds to calculate
        if let Some((ratio, equity_needed)) = state.pot_odds(local_seat()) {
            assert!(ratio > 1.0);
            assert!(equity_needed > 0.0);
            assert!(equity_needed < 1.0);
        }
    }

    #[test]
    fn test_button_alternates() {
        use terminal_poker::game::state::GameState;

        let mut state = GameState::new(100);
        let first_button = state.button;

        state.start_new_hand();
        assert_ne!(state.button, first_button);

        state.start_new_hand();
        assert_eq!(state.button, first_button);
    }
}

// Test hand evaluation
#[cfg(test)]
mod hand_eval_tests {
    use terminal_poker::game::deck::{Card, Rank, Suit};
    use terminal_poker::game::hand::{evaluate_hand, HandRank};

    #[test]
    fn test_royal_flush() {
        let hole = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
        ];
        let board = vec![
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Jack, Suit::Spades),
            Card::new(Rank::Ten, Suit::Spades),
            Card::new(Rank::Two, Suit::Hearts),
            Card::new(Rank::Three, Suit::Diamonds),
        ];

        let eval = evaluate_hand(&hole, &board);
        assert_eq!(eval.rank, HandRank::StraightFlush);
    }

    #[test]
    fn test_full_house() {
        let hole = vec![
            Card::new(Rank::King, Suit::Spades),
            Card::new(Rank::King, Suit::Hearts),
        ];
        let board = vec![
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::Queen, Suit::Spades),
            Card::new(Rank::Queen, Suit::Hearts),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Diamonds),
        ];

        let eval = evaluate_hand(&hole, &board);
        assert_eq!(eval.rank, HandRank::FullHouse);
    }

    #[test]
    fn test_two_pair() {
        let hole = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::Ace, Suit::Hearts),
        ];
        let board = vec![
            Card::new(Rank::King, Suit::Diamonds),
            Card::new(Rank::King, Suit::Clubs),
            Card::new(Rank::Two, Suit::Spades),
            Card::new(Rank::Three, Suit::Hearts),
            Card::new(Rank::Four, Suit::Diamonds),
        ];

        let eval = evaluate_hand(&hole, &board);
        assert_eq!(eval.rank, HandRank::TwoPair);
    }
}

// Test bot behavior
#[cfg(test)]
mod bot_tests {
    use super::bot_seat;
    use terminal_poker::bot::rule_based::RuleBasedBot;
    use terminal_poker::game::actions::Action;
    use terminal_poker::game::state::GameState;

    #[test]
    fn test_bot_always_returns_valid_action() {
        let bot = RuleBasedBot::new(0.5, bot_seat());
        let state = GameState::new(100);

        // Run 100 times to account for randomness
        for _ in 0..100 {
            let action = bot.decide(&state);
            // Should never panic and should return a valid action
            match action {
                Action::Fold
                | Action::Check
                | Action::Call(_)
                | Action::Bet(_)
                | Action::Raise(_)
                | Action::AllIn(_) => {}
            }
        }
    }

    #[test]
    fn test_passive_bot() {
        let bot = RuleBasedBot::new(0.0, bot_seat());
        let state = GameState::new(100);

        let mut aggressive_actions = 0;
        for _ in 0..50 {
            let action = bot.decide(&state);
            if matches!(action, Action::Bet(_) | Action::Raise(_)) {
                aggressive_actions += 1;
            }
        }

        // Passive bot should rarely bet/raise
        assert!(
            aggressive_actions < 25,
            "Passive bot too aggressive: {}",
            aggressive_actions
        );
    }
}

// Regression tests for betting logic bugs
#[cfg(test)]
mod betting_logic_tests {
    use super::apply_action;
    use terminal_poker::game::actions::Action;
    use terminal_poker::game::state::{GamePhase, GameState};

    /// Tests that last_raise_size is correctly calculated after a bet.
    /// Regression test for bug where last_raise_size was calculated AFTER
    /// add_chips() mutated state, resulting in last_raise_size = 0.
    #[test]
    fn test_last_raise_size_after_bet() {
        let mut state = GameState::new(100);

        // Skip to postflop where player_bet starts at 0
        // First, both players call preflop to get to flop
        while state.phase == GamePhase::Preflop {
            let to_call = state.amount_to_call(state.to_act);
            let action = if to_call > 0 {
                Action::Call(to_call)
            } else {
                Action::Check
            };
            let actor = state.to_act;
            apply_action(&mut state, actor, action);
        }

        assert_eq!(state.phase, GamePhase::Flop);

        // Now on the flop, first to act makes a bet of 10
        let actor = state.to_act;
        apply_action(&mut state, actor, Action::Bet(10));

        // last_raise_size should be 10 (the bet amount minus old max of 0)
        assert_eq!(
            state.last_raise_size, 10,
            "last_raise_size should be 10 after a bet of 10 from 0"
        );
    }

    /// Tests that last_raise_size is correctly calculated after a raise.
    #[test]
    fn test_last_raise_size_after_raise() {
        let mut state = GameState::new(100);

        // Skip to postflop
        while state.phase == GamePhase::Preflop {
            let to_call = state.amount_to_call(state.to_act);
            let action = if to_call > 0 {
                Action::Call(to_call)
            } else {
                Action::Check
            };
            let actor = state.to_act;
            apply_action(&mut state, actor, action);
        }

        assert_eq!(state.phase, GamePhase::Flop);

        // First player bets 10
        let first_actor = state.to_act;
        apply_action(&mut state, first_actor, Action::Bet(10));
        assert_eq!(state.last_raise_size, 10);

        // Second player raises to 30 (a raise of 20)
        let second_actor = state.to_act;
        apply_action(&mut state, second_actor, Action::Raise(30));

        // last_raise_size should be 20 (30 - 10)
        assert_eq!(
            state.last_raise_size, 20,
            "last_raise_size should be 20 after raising from 10 to 30"
        );
    }

    /// Tests that minimum raise calculation works correctly.
    /// After a raise of 20, the next min raise should be current_bet + 20.
    #[test]
    fn test_minimum_raise_calculation() {
        let mut state = GameState::new(100);

        // Skip to postflop
        while state.phase == GamePhase::Preflop {
            let to_call = state.amount_to_call(state.to_act);
            let action = if to_call > 0 {
                Action::Call(to_call)
            } else {
                Action::Check
            };
            let actor = state.to_act;
            apply_action(&mut state, actor, action);
        }

        // First player bets 10
        let actor = state.to_act;
        apply_action(&mut state, actor, Action::Bet(10));

        // Second player raises to 30 (raise of 20)
        let actor = state.to_act;
        apply_action(&mut state, actor, Action::Raise(30));

        // Now first player faces a raise, min re-raise should be 30 + 20 = 50
        let available = state.available_actions();
        assert_eq!(
            available.min_raise,
            Some(50),
            "After a raise to 30 (raise of 20), min re-raise should be 50"
        );
    }

    /// Tests that last_raise_size is correctly calculated for all-in raises.
    #[test]
    fn test_last_raise_size_after_allin() {
        let mut state = GameState::new(50); // Small stack for easier all-in

        // Skip to postflop
        while state.phase == GamePhase::Preflop {
            let to_call = state.amount_to_call(state.to_act);
            let action = if to_call > 0 {
                Action::Call(to_call)
            } else {
                Action::Check
            };
            let actor = state.to_act;
            apply_action(&mut state, actor, action);
        }

        // First player bets 10
        let actor = state.to_act;
        apply_action(&mut state, actor, Action::Bet(10));

        // Second player goes all-in (stack was ~97 after blinds, now betting street)
        let actor = state.to_act;
        let allin_amount = state.street_bet(actor) + state.stack(actor);

        let old_max = 10; // The bet from first player
        apply_action(&mut state, actor, Action::AllIn(allin_amount));

        // last_raise_size should be allin_amount - old_max
        let expected_raise_size = allin_amount - old_max;
        assert_eq!(
            state.last_raise_size, expected_raise_size,
            "last_raise_size should be {} after all-in of {} over bet of {}",
            expected_raise_size, allin_amount, old_max
        );
    }
}

// Regression tests for split pot logic
#[cfg(test)]
mod split_pot_tests {
    use super::{bot_seat, local_seat};
    use terminal_poker::game::deck::{Card, Rank, Suit};
    use terminal_poker::game::state::GameState;

    /// Tests that odd chip in split pot goes to the out-of-position player.
    /// This is the player who is NOT the button (acts first postflop).
    #[test]
    fn test_split_pot_odd_chip_distribution() {
        // We need to create a scenario with an odd pot and a tie
        // This is tricky to set up deterministically, so we test the logic directly

        let mut state = GameState::new(100);

        // Create an odd pot with no outstanding street contribution.
        state.pot = 101; // Odd pot
        state.seat_mut(local_seat()).stack = 100;
        state.seat_mut(local_seat()).street_bet = 0;
        state.seat_mut(bot_seat()).stack = 100;
        state.seat_mut(bot_seat()).street_bet = 0;

        // Force specific hands that will tie (same cards different suits)
        state.seat_mut(local_seat()).hole_cards = vec![
            Card::new(Rank::Ace, Suit::Spades),
            Card::new(Rank::King, Suit::Spades),
        ];
        state.seat_mut(bot_seat()).hole_cards = vec![
            Card::new(Rank::Ace, Suit::Hearts),
            Card::new(Rank::King, Suit::Hearts),
        ];

        // Board that makes both have same hand (pair of aces, king kicker)
        state.board = vec![
            Card::new(Rank::Ace, Suit::Diamonds),
            Card::new(Rank::Two, Suit::Clubs),
            Card::new(Rank::Three, Suit::Clubs),
            Card::new(Rank::Four, Suit::Diamonds),
            Card::new(Rank::Five, Suit::Hearts),
        ];

        // Resolve the river. In the initial hand the local seat is the button,
        // so the bot is first clockwise from the button and receives the odd chip.
        state.phase = terminal_poker::game::state::GamePhase::River;
        state.advance_phase();

        assert_eq!(state.stack(local_seat()), 150);
        assert_eq!(state.stack(bot_seat()), 151);
        assert_eq!(state.pot, 0);
        assert_eq!(state.showdown_result.as_ref().unwrap().winner, None);
    }
}
