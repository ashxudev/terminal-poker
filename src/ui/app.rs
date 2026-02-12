use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::bot::rule_based::RuleBasedBot;
use crate::game::actions::Action;
use crate::game::state::{GamePhase, GameState, Player, BIG_BLIND, SMALL_BLIND};
use crate::stats::persistence::StatsStore;

const DELAY_BOT_ACTION_MS: u64 = 2500;
const DELAY_BOT_ACTION_AFTER_REVEAL_MS: u64 = 3500;
const DELAY_CARD_REVEAL_MS: u64 = 500;
const DELAY_CARD_REVEAL_AFTER_BOT_MS: u64 = 2500;
const DELAY_NEW_HAND_MS: u64 = 1200;
const DELAY_SHOWDOWN_REVEAL_MS: u64 = 1000;
const DELAY_SHOWDOWN_RESULT_MS: u64 = 1000;
const DELAY_POST_SB_MS: u64 = 400;
const DELAY_POST_BB_MS: u64 = 800;

#[derive(Debug, Clone)]
pub enum GameEvent {
    BotAction,
    StartNewHand,
    PostSmallBlind,
    PostBigBlind,
    RevealCards,
    RevealShowdown,
    ShowResult,
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
    pub visible_board_len: usize,
    pub visible_player_bet: u32,
    pub visible_bot_bet: u32,
    pub player_last_action: Option<Action>,
    pub bot_last_action: Option<Action>,
    pub bot_thinking: bool,
    pub tick_count: u64,
    pub thinking_start_tick: u64,
    pub showdown_revealed: bool,
    pub showdown_result_shown: bool,
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
            visible_board_len: 0,

            visible_player_bet: 0,
            visible_bot_bet: 0,
            player_last_action: None,
            bot_last_action: None,
            bot_thinking: false,
            tick_count: 0,
            thinking_start_tick: 0,
            showdown_revealed: false,
            showdown_result_shown: false,
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

    pub fn new_session(&mut self, stats: &mut StatsStore) {
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
        self.visible_board_len = 0;
        self.player_last_action = None;
        self.bot_last_action = None;
        self.bot_thinking = false;
        self.showdown_revealed = false;
        self.showdown_result_shown = false;
        self.message = Some("New session started!".to_string());
        self.initialize(stats);
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
        self.player_last_action = Some(action);

        // Snapshot visible state before apply_action (which may advance phase and clear bets/pot)
        self.visible_player_bet = self.projected_bet(Player::Human, action);
        self.visible_bot_bet = self.game_state.bot_bet;

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
                            "Opp wins the pot"
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
                    .push_back(GameEvent::RevealShowdown);
                self.next_event_at =
                    Some(Instant::now() + Duration::from_millis(DELAY_SHOWDOWN_REVEAL_MS));
            }
            GamePhase::SessionEnd | GamePhase::Summary => {
                // Terminal states — nothing to enqueue
            }
            _ => {
                if phase_changed && self.visible_board_len < self.game_state.board.len() {
                    // Street transition: longer pause after bot's closing action
                    let reveal_delay =
                        if self.game_state.last_action.map(|(p, _)| p) == Some(Player::Bot) {
                            DELAY_CARD_REVEAL_AFTER_BOT_MS
                        } else {
                            DELAY_CARD_REVEAL_MS
                        };
                    self.pending_events.push_back(GameEvent::RevealCards);
                    self.next_event_at =
                        Some(Instant::now() + Duration::from_millis(reveal_delay));
                } else if self.game_state.to_act == Player::Bot {
                    self.pending_events.push_back(GameEvent::BotAction);
                    self.next_event_at =
                        Some(Instant::now() + Duration::from_millis(DELAY_BOT_ACTION_MS));
                    self.bot_thinking = true;
                    self.thinking_start_tick = self.tick_count;
                    self.bot_last_action = None;
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
                self.bot_thinking = false;
                let street = Self::phase_name(self.game_state.phase);
                let bot_action = self.bot.decide(&self.game_state);
                self.bot_last_action = Some(bot_action);

                // Snapshot visible bets before apply_action (which may advance phase and clear bets)
                self.visible_bot_bet = self.projected_bet(Player::Bot, bot_action);
                self.visible_player_bet = self.game_state.player_bet;

                let desc = bot_action.description_for("Opp");
                self.game_state.apply_action(Player::Bot, bot_action);
                self.log_action(street, format!("Opp {}", desc));
                self.message = Some(format!("Opp {}", desc));
            }
            GameEvent::StartNewHand => {
                self.saw_flop_this_hand = false;
                self.recorded_hand_this_round = false;
                self.recorded_vpip_this_hand = false;
                self.raise_mode = false;
                self.raise_input.clear();
                self.player_last_action = None;
                self.bot_last_action = None;
                self.showdown_revealed = false;
                self.showdown_result_shown = false;
                self.game_state.start_new_hand();
                self.visible_board_len = 0;
                self.visible_player_bet = 0;
                self.visible_bot_bet = 0;
                self.last_phase = self.game_state.phase;
                // Add a separator for the new hand in the historical log
                self.action_log.push(ActionLogEntry {
                    street: String::new(),
                    text: format!("── Hand #{} ──", self.game_state.hand_number),
                });
                self.log_blinds();
                self.pending_events.push_back(GameEvent::PostSmallBlind);
                self.next_event_at =
                    Some(Instant::now() + Duration::from_millis(DELAY_POST_SB_MS));
                return;
            }
            GameEvent::PostSmallBlind => {
                match self.game_state.button {
                    Player::Human => self.visible_player_bet = self.game_state.player_bet,
                    Player::Bot => self.visible_bot_bet = self.game_state.bot_bet,
                }
                self.pending_events.push_back(GameEvent::PostBigBlind);
                self.next_event_at =
                    Some(Instant::now() + Duration::from_millis(DELAY_POST_BB_MS));
                return;
            }
            GameEvent::PostBigBlind => {
                match self.game_state.button {
                    Player::Human => self.visible_bot_bet = self.game_state.bot_bet,
                    Player::Bot => self.visible_player_bet = self.game_state.player_bet,
                }
            }
            GameEvent::RevealCards => {
                self.visible_board_len = self.game_state.board.len();
                self.visible_player_bet = 0;
                self.visible_bot_bet = 0;
                self.player_last_action = None;
                self.bot_last_action = None;
                // After revealing, check if bot should act next
                if self.game_state.to_act == Player::Bot {
                    self.pending_events.push_back(GameEvent::BotAction);
                    self.next_event_at =
                        Some(Instant::now() + Duration::from_millis(DELAY_BOT_ACTION_AFTER_REVEAL_MS));
                    self.bot_thinking = true;
                    self.thinking_start_tick = self.tick_count;
                    return;
                }
            }
            GameEvent::RevealShowdown => {
                self.showdown_revealed = true;
                self.player_last_action = None;
                self.bot_last_action = None;
                // Record stats
                if let Some(ref result) = self.game_state.showdown_result {
                    let won = result.winner == Some(Player::Human);
                    stats.record_showdown(won);
                    if won {
                        stats.record_pot_won(result.pot_won);
                    } else if result.winner == Some(Player::Bot) {
                        stats.record_pot_lost(result.pot_won);
                    }
                }
                self.pending_events.push_back(GameEvent::ShowResult);
                self.next_event_at =
                    Some(Instant::now() + Duration::from_millis(DELAY_SHOWDOWN_RESULT_MS));
                return;
            }
            GameEvent::ShowResult => {
                self.showdown_result_shown = true;
                self.next_event_at = None;
                return;
            }
        }

        // Clear timer and check what to do next
        self.next_event_at = None;
        self.enqueue_next_events(stats);
    }

    fn log_blinds(&mut self) {
        let sb_bb = SMALL_BLIND as f64 / BIG_BLIND as f64;
        let (sb_player, bb_player) = if self.game_state.button == Player::Human {
            ("You", "Opp")
        } else {
            ("Opp", "You")
        };
        self.log_action(
            "Pre-Flop",
            format!("{} post SB ({:.1}BB)", sb_player, sb_bb),
        );
        self.log_action("Pre-Flop", format!("{} post BB (1BB)", bb_player));
    }

    pub fn initialize(&mut self, _stats: &mut StatsStore) {
        self.visible_player_bet = 0;
        self.visible_bot_bet = 0;
        self.log_blinds();
        self.pending_events.push_back(GameEvent::PostSmallBlind);
        self.next_event_at = Some(Instant::now() + Duration::from_millis(DELAY_POST_SB_MS));
    }

    /// Compute what a player's bet will be after an action, before apply_action clears it.
    fn projected_bet(&self, player: Player, action: Action) -> u32 {
        let current = match player {
            Player::Human => self.game_state.player_bet,
            Player::Bot => self.game_state.bot_bet,
        };
        let stack = match player {
            Player::Human => self.game_state.player_stack,
            Player::Bot => self.game_state.bot_stack,
        };
        match action {
            Action::Fold | Action::Check => current,
            Action::Call(amount) => current + amount.min(stack),
            Action::Bet(amount) | Action::Raise(amount) => {
                let to_add = amount - current;
                current + to_add.min(stack)
            }
            Action::AllIn(amount) => {
                let to_add = amount - current;
                current + to_add.min(stack)
            }
        }
    }

    pub fn continue_after_showdown(&mut self, _stats: &mut StatsStore) {
        if self.game_state.phase == GamePhase::Showdown && self.showdown_result_shown {
            self.pending_events.clear();
            if self.game_state.player_stack > 0 && self.game_state.bot_stack > 0 {
                self.pending_events.push_back(GameEvent::StartNewHand);
                self.next_event_at = Some(Instant::now()); // immediate — user pressed key
            } else {
                self.game_state.phase = GamePhase::SessionEnd;
                self.next_event_at = None;
            }
        }
    }
}
