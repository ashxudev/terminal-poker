use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::bot::rule_based::RuleBasedBot;
use crate::game::actions::Action;
use crate::game::state::{GamePhase, GameState, Player};
use crate::stats::persistence::StatsStore;

const DELAY_BOT_ACTION_MS: u64 = 700;
const DELAY_STREET_PAUSE_MS: u64 = 500;
const DELAY_NEW_HAND_MS: u64 = 1200;
#[derive(Debug, Clone)]
pub enum GameEvent {
    BotAction,
    StartNewHand,
    RecordShowdownStats,
}

#[derive(Debug, Clone)]
pub struct ActionLogEntry {
    pub street: String,
    pub text: String,
}

pub struct App {
    pub game_state: GameState,
    pub bot: RuleBasedBot,
    pub show_help: bool,
    pub show_stats: bool,
    pub raise_input: String,
    pub message: Option<String>,
    pub action_log: Vec<ActionLogEntry>,
    pub pending_events: VecDeque<GameEvent>,
    pub next_event_at: Option<Instant>,
    pub raise_mode: bool,
    starting_stack_bb: u32,
    last_phase: GamePhase,
    saw_flop_this_hand: bool,
    recorded_hand_this_round: bool,
    recorded_vpip_this_hand: bool,
}

impl App {
    pub fn new(starting_stack_bb: u32, aggression: f64) -> Self {
        let game_state = GameState::new(starting_stack_bb);
        let initial_phase = game_state.phase;
        Self {
            game_state,
            bot: RuleBasedBot::new(aggression),
            show_help: false,
            show_stats: false,
            raise_input: String::new(),
            message: None,
            action_log: Vec::new(),
            pending_events: VecDeque::new(),
            next_event_at: None,
            raise_mode: false,
            starting_stack_bb,
            last_phase: initial_phase,
            saw_flop_this_hand: false,
            recorded_hand_this_round: false,
            recorded_vpip_this_hand: false,
        }
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
        if self.show_help {
            self.show_stats = false;
        }
    }

    pub fn toggle_stats(&mut self) {
        self.show_stats = !self.show_stats;
        if self.show_stats {
            self.show_help = false;
        }
    }

    pub fn new_session(&mut self) {
        self.game_state = GameState::new(self.starting_stack_bb);
        self.last_phase = self.game_state.phase;
        self.saw_flop_this_hand = false;
        self.recorded_hand_this_round = false;
        self.recorded_vpip_this_hand = false;
        self.action_log.clear();
        self.pending_events.clear();
        self.next_event_at = None;
        self.raise_mode = false;
        self.raise_input.clear();
        self.message = Some("New session started!".to_string());
    }

    pub fn has_pending_events(&self) -> bool {
        !self.pending_events.is_empty()
    }

    fn phase_name(phase: GamePhase) -> &'static str {
        match phase {
            GamePhase::Preflop => "Pre-Flop",
            GamePhase::Flop => "Flop",
            GamePhase::Turn => "Turn",
            GamePhase::River => "River",
            _ => "",
        }
    }

    fn log_action(&mut self, street: &str, text: String) {
        self.action_log.push(ActionLogEntry {
            street: street.to_string(),
            text,
        });
        // Keep a reasonable history (visible area handles scrolling)
        if self.action_log.len() > 100 {
            self.action_log.drain(..50);
        }
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

        self.raise_mode = false;
        self.raise_input.clear();

        // Capture phase before apply_action (which may advance the phase)
        let street = Self::phase_name(self.game_state.phase);
        let desc = action.description_for("You");
        self.game_state.apply_action(Player::Human, action);
        self.log_action(street, format!("You {}", desc));
        self.message = Some(format!("You {}", desc));

        // Enqueue follow-up events
        self.enqueue_next_events(stats);
    }

    /// Inspect the current game state and enqueue exactly one pending event
    /// with an appropriate delay. Does nothing if events are already pending.
    pub fn enqueue_next_events(&mut self, stats: &mut StatsStore) {
        if !self.pending_events.is_empty() {
            return;
        }

        // Track flop stat
        if !self.saw_flop_this_hand && self.game_state.board.len() >= 3 {
            self.saw_flop_this_hand = true;
            stats.record_saw_flop();
        }

        // Detect street transitions for extra pause
        let phase_changed = self.game_state.phase != self.last_phase;
        if phase_changed {
            self.last_phase = self.game_state.phase;
        }

        match self.game_state.phase {
            GamePhase::HandComplete => {
                if self.game_state.player_stack > 0 && self.game_state.bot_stack > 0 {
                    // Log the fold result
                    if let Some((player, _)) = self.game_state.last_action {
                        let winner_text = if player == Player::Bot {
                            "You win the pot"
                        } else {
                            "Bot wins the pot"
                        };
                        self.log_action("", winner_text.to_string());
                    }
                    self.pending_events.push_back(GameEvent::StartNewHand);
                    self.next_event_at =
                        Some(Instant::now() + Duration::from_millis(DELAY_NEW_HAND_MS));
                }
                // else: session over, main loop detects busted stacks
            }
            GamePhase::Showdown => {
                self.pending_events
                    .push_back(GameEvent::RecordShowdownStats);
                self.next_event_at = Some(Instant::now()); // immediate
            }
            GamePhase::SessionEnd | GamePhase::Summary => {
                // Terminal states — nothing to enqueue
            }
            _ => {
                if self.game_state.to_act == Player::Bot {
                    let delay = if phase_changed {
                        DELAY_STREET_PAUSE_MS + DELAY_BOT_ACTION_MS
                    } else {
                        DELAY_BOT_ACTION_MS
                    };
                    self.pending_events.push_back(GameEvent::BotAction);
                    self.next_event_at =
                        Some(Instant::now() + Duration::from_millis(delay));
}
                // else: player's turn, wait for input
            }
        }
    }

    /// Process the next pending event if its delay has elapsed.
    /// Called every iteration of the main loop.
    pub fn process_next_event(&mut self, stats: &mut StatsStore) {
        let event_time = match self.next_event_at {
            Some(t) => t,
            None => return,
        };
        if Instant::now() < event_time {
            return;
        }

        let event = match self.pending_events.pop_front() {
            Some(e) => e,
            None => {
                self.next_event_at = None;
                return;
            }
        };

        match event {
            GameEvent::BotAction => {
                let street = Self::phase_name(self.game_state.phase);
                let bot_action = self.bot.decide(&self.game_state);
                let desc = bot_action.description_for("Bot");
                self.game_state.apply_action(Player::Bot, bot_action);
                self.log_action(street, format!("Bot {}", desc));
                self.message = Some(format!("Bot {}", desc));
            }
            GameEvent::StartNewHand => {
                self.saw_flop_this_hand = false;
                self.recorded_hand_this_round = false;
                self.recorded_vpip_this_hand = false;
                self.raise_mode = false;
                self.raise_input.clear();
                self.game_state.start_new_hand();
                self.last_phase = self.game_state.phase;
                // Add a separator for the new hand in the historical log
                self.action_log.push(ActionLogEntry {
                    street: String::new(),
                    text: format!("── Hand #{} ──", self.game_state.hand_number),
                });
            }
            GameEvent::RecordShowdownStats => {
                if let Some(ref result) = self.game_state.showdown_result {
                    let won = result.winner == Some(Player::Human);
                    stats.record_showdown(won);
                    if won {
                        stats.record_pot_won(result.pot_won);
                    } else if result.winner == Some(Player::Bot) {
                        stats.record_pot_lost(result.pot_won);
                    }
                }
                self.next_event_at = None;
                return; // Don't enqueue follow-ups; showdown waits for user keypress
            }
        }

        // Clear timer and check what to do next
        self.next_event_at = None;
        self.enqueue_next_events(stats);
    }

    pub fn initialize(&mut self, stats: &mut StatsStore) {
        self.enqueue_next_events(stats);
    }

    pub fn continue_after_showdown(&mut self, _stats: &mut StatsStore) {
        if self.game_state.phase == GamePhase::Showdown {
            if self.game_state.player_stack > 0 && self.game_state.bot_stack > 0 {
                self.pending_events.push_back(GameEvent::StartNewHand);
                self.next_event_at = Some(Instant::now()); // immediate — user pressed key
            } else {
                self.game_state.phase = GamePhase::SessionEnd;
            }
            // Ensure follow-up events get scheduled after the new hand starts
            // (handled by process_next_event -> enqueue_next_events chain)
        }
    }
}
