use super::state::BIG_BLIND;
use serde::{Deserialize, Serialize};

fn format_bb(chips: u32) -> String {
    let bb = chips as f64 / BIG_BLIND as f64;
    if bb == bb.floor() {
        format!("{}BB", bb as u32)
    } else {
        format!("{:.1}BB", bb)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Fold,
    Check,
    Call(u32),
    Bet(u32),
    Raise(u32),
    AllIn(u32),
}

#[allow(dead_code)]
impl Action {
    pub fn amount(&self) -> u32 {
        match self {
            Action::Fold | Action::Check => 0,
            Action::Call(amt) | Action::Bet(amt) | Action::Raise(amt) | Action::AllIn(amt) => *amt,
        }
    }

    pub fn is_aggressive(&self) -> bool {
        matches!(self, Action::Bet(_) | Action::Raise(_) | Action::AllIn(_))
    }

    pub fn description(&self) -> String {
        match self {
            Action::Fold => "folds".to_string(),
            Action::Check => "checks".to_string(),
            Action::Call(amt) => format!("calls {}", amt),
            Action::Bet(amt) => format!("bets {}", amt),
            Action::Raise(amt) => format!("raises to {}", amt),
            Action::AllIn(amt) => format!("all-in for {}", amt),
        }
    }

    /// Actor-aware description with BB-formatted amounts.
    /// "You" gets base-form verbs ("call"), "Bot" gets third-person ("calls").
    pub fn description_for(&self, actor: &str) -> String {
        let is_you = actor == "You";
        match self {
            Action::Fold => if is_you { "fold".into() } else { "folds".into() },
            Action::Check => if is_you { "check".into() } else { "checks".into() },
            Action::Call(amt) => format!("{} {}", if is_you { "call" } else { "calls" }, format_bb(*amt)),
            Action::Bet(amt) => format!("{} {}", if is_you { "bet" } else { "bets" }, format_bb(*amt)),
            Action::Raise(amt) => format!("{} to {}", if is_you { "raise" } else { "raises" }, format_bb(*amt)),
            Action::AllIn(amt) => format!("{} for {}", if is_you { "all-in" } else { "all-in" }, format_bb(*amt)),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AvailableActions {
    pub can_fold: bool,
    pub can_check: bool,
    pub can_call: Option<u32>,
    pub min_bet: Option<u32>,
    pub min_raise: Option<u32>,
    pub max_raise: u32,
}

impl AvailableActions {
    pub fn new(
        to_call: u32,
        min_raise_to: u32,
        player_stack: u32,
        big_blind: u32,
    ) -> Self {
        let can_check = to_call == 0;
        let can_call = if to_call > 0 && to_call < player_stack {
            Some(to_call)
        } else {
            None
        };

        let min_bet = if can_check && player_stack > 0 {
            Some(big_blind.min(player_stack))
        } else {
            None
        };

        let min_raise = if to_call > 0 && min_raise_to < player_stack {
            Some(min_raise_to)
        } else {
            None
        };

        Self {
            can_fold: to_call > 0,
            can_check,
            can_call,
            min_bet,
            min_raise,
            max_raise: player_stack,
        }
    }
}
