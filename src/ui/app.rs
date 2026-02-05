use crate::bot::rule_based::RuleBasedBot;
use crate::game::actions::Action;
use crate::game::state::{GamePhase, GameState, Player};
use crate::stats::persistence::StatsStore;

pub struct App {
    pub game_state: GameState,
    pub bot: RuleBasedBot,
    pub show_help: bool,
    pub show_stats: bool,
    pub raise_input: String,
    pub message: Option<String>,
    starting_stack_bb: u32,
    saw_flop_this_hand: bool,
    recorded_hand_this_round: bool,
    recorded_vpip_this_hand: bool,
}

impl App {
    pub fn new(starting_stack_bb: u32, aggression: f64) -> Self {
        Self {
            game_state: GameState::new(starting_stack_bb),
            bot: RuleBasedBot::new(aggression),
            show_help: false,
            show_stats: false,
            raise_input: String::new(),
            message: None,
            starting_stack_bb,
            saw_flop_this_hand: false,
            recorded_hand_this_round: false,
            recorded_vpip_this_hand: false,
        }
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn toggle_stats(&mut self) {
        self.show_stats = !self.show_stats;
    }

    pub fn new_session(&mut self) {
        self.game_state = GameState::new(self.starting_stack_bb);
        self.saw_flop_this_hand = false;
        self.recorded_hand_this_round = false;
        self.recorded_vpip_this_hand = false;
        self.message = Some("New session started!".to_string());
    }

    pub fn apply_player_action(&mut self, action: Action, stats: &mut StatsStore) {
        if !self.game_state.is_player_turn() {
            return;
        }

        // Record stats - only count hand once
        if !self.recorded_hand_this_round {
            stats.record_hand_start();
            self.recorded_hand_this_round = true;
        }

        match action {
            Action::Call(_) => stats.record_call(),
            Action::Bet(_) => {
                stats.record_bet();
                if self.game_state.board.is_empty() {
                    stats.record_pfr();
                }
            }
            Action::Raise(_) | Action::AllIn(_) => {
                stats.record_raise();
                if self.game_state.board.is_empty() {
                    stats.record_pfr();
                }
            }
            _ => {}
        }

        // VPIP: only track preflop voluntary money, once per hand
        if !self.recorded_vpip_this_hand
            && self.game_state.board.is_empty()
            && !matches!(action, Action::Fold | Action::Check)
        {
            stats.record_vpip();
            self.recorded_vpip_this_hand = true;
        }

        // Apply the action
        self.game_state.apply_action(Player::Human, action);
        self.message = Some(format!("You {}", action.description()));

        // Check if we need bot action or hand is over
        self.process_game_state(stats);
    }

    fn process_game_state(&mut self, stats: &mut StatsStore) {
        loop {
            // Track when player sees the flop (WTSD opportunity)
            if !self.saw_flop_this_hand && self.game_state.board.len() >= 3 {
                self.saw_flop_this_hand = true;
                stats.record_saw_flop();
            }

            match self.game_state.phase {
                GamePhase::HandComplete => {
                    // Start next hand if both players have chips
                    if self.game_state.player_stack > 0 && self.game_state.bot_stack > 0 {
                        self.saw_flop_this_hand = false;
                        self.recorded_hand_this_round = false;
                        self.recorded_vpip_this_hand = false;
                        self.game_state.start_new_hand();
                        continue; // Let bot act if it's their turn
                    }
                    break; // Session over (someone busted)
                }
                GamePhase::Showdown => {
                    // Record showdown stats
                    if let Some(ref result) = self.game_state.showdown_result {
                        let won = result.winner == Some(Player::Human);
                        stats.record_showdown(won);
                        if won {
                            stats.record_pot_won(result.pot_won);
                        } else if result.winner == Some(Player::Bot) {
                            stats.record_pot_lost(result.pot_won);
                        }
                    }
                    break;
                }
                GamePhase::SessionEnd | GamePhase::Summary => {
                    stats.record_session_end();
                    stats.record_profit((self.game_state.session_profit_bb() * 2.0).round() as i64);
                    break;
                }
                _ => {
                    if self.game_state.to_act == Player::Bot {
                        let bot_action = self.bot.decide(&self.game_state);
                        self.game_state.apply_action(Player::Bot, bot_action);
                        self.message = Some(format!("Bot {}", bot_action.description()));
                    } else {
                        break;
                    }
                }
            }
        }
    }

    pub fn initialize(&mut self, stats: &mut StatsStore) {
        self.process_game_state(stats);
    }

    pub fn continue_after_showdown(&mut self) {
        if self.game_state.phase == GamePhase::Showdown {
            if self.game_state.player_stack > 0 && self.game_state.bot_stack > 0 {
                self.saw_flop_this_hand = false;
                self.recorded_hand_this_round = false;
                self.recorded_vpip_this_hand = false;
                self.game_state.start_new_hand();
            } else {
                self.game_state.phase = GamePhase::SessionEnd;
            }
        }
    }
}
