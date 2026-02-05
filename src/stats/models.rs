use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    // Lifetime stats
    pub total_hands: u64,
    pub total_sessions: u64,

    // Preflop stats
    pub vpip_hands: u64,      // Voluntarily put $ in pot
    pub pfr_hands: u64,       // Preflop raise
    pub three_bet_opportunities: u64,
    pub three_bet_hands: u64,

    // Postflop stats
    pub cbet_opportunities: u64,
    pub cbet_hands: u64,
    pub fold_to_cbet_opportunities: u64,
    pub fold_to_cbet_hands: u64,

    // Showdown stats
    pub wtsd_opportunities: u64, // Went to showdown
    pub wtsd_hands: u64,
    pub wsd_hands: u64, // Won at showdown

    // Aggression tracking
    pub bets: u64,
    pub raises: u64,
    pub calls: u64,

    // Results
    pub total_profit_chips: i64,
    pub biggest_pot_won: u32,
    pub biggest_pot_lost: u32,
}

#[allow(dead_code)]
impl PlayerStats {
    pub fn vpip(&self) -> f64 {
        if self.total_hands == 0 {
            0.0
        } else {
            self.vpip_hands as f64 / self.total_hands as f64 * 100.0
        }
    }

    pub fn pfr(&self) -> f64 {
        if self.total_hands == 0 {
            0.0
        } else {
            self.pfr_hands as f64 / self.total_hands as f64 * 100.0
        }
    }

    pub fn three_bet(&self) -> f64 {
        if self.three_bet_opportunities == 0 {
            0.0
        } else {
            self.three_bet_hands as f64 / self.three_bet_opportunities as f64 * 100.0
        }
    }

    pub fn cbet(&self) -> f64 {
        if self.cbet_opportunities == 0 {
            0.0
        } else {
            self.cbet_hands as f64 / self.cbet_opportunities as f64 * 100.0
        }
    }

    pub fn fold_to_cbet(&self) -> f64 {
        if self.fold_to_cbet_opportunities == 0 {
            0.0
        } else {
            self.fold_to_cbet_hands as f64 / self.fold_to_cbet_opportunities as f64 * 100.0
        }
    }

    pub fn wtsd(&self) -> f64 {
        if self.wtsd_opportunities == 0 {
            0.0
        } else {
            self.wtsd_hands as f64 / self.wtsd_opportunities as f64 * 100.0
        }
    }

    pub fn wsd(&self) -> f64 {
        if self.wtsd_hands == 0 {
            0.0
        } else {
            self.wsd_hands as f64 / self.wtsd_hands as f64 * 100.0
        }
    }

    pub fn aggression_factor(&self) -> f64 {
        if self.calls == 0 {
            if self.bets + self.raises > 0 {
                99.9 // Cap instead of INFINITY for display purposes
            } else {
                0.0
            }
        } else {
            (self.bets + self.raises) as f64 / self.calls as f64
        }
    }

    pub fn win_rate_bb_per_100(&self) -> f64 {
        if self.total_hands == 0 {
            0.0
        } else {
            self.total_profit_chips as f64 / 2.0 / self.total_hands as f64 * 100.0
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct StatDefinition {
    pub abbrev: &'static str,
    pub name: &'static str,
    pub explanation: &'static str,
}

pub const STAT_DEFINITIONS: &[StatDefinition] = &[
    StatDefinition {
        abbrev: "VPIP",
        name: "Voluntarily Put $ In Pot",
        explanation: "% of hands where you voluntarily put money in preflop (calls or raises, not blinds)",
    },
    StatDefinition {
        abbrev: "PFR",
        name: "Pre-Flop Raise",
        explanation: "% of hands where you raised preflop. Should be close to VPIP for tight-aggressive play",
    },
    StatDefinition {
        abbrev: "3Bet",
        name: "3-Bet Frequency",
        explanation: "% of times you re-raised when facing a raise. 7-10% is typical",
    },
    StatDefinition {
        abbrev: "Cbet",
        name: "Continuation Bet",
        explanation: "% of times you bet the flop after raising preflop. 60-70% is standard",
    },
    StatDefinition {
        abbrev: "FCbet",
        name: "Fold to C-bet",
        explanation: "% of times you folded to a continuation bet. >50% is exploitable",
    },
    StatDefinition {
        abbrev: "WTSD",
        name: "Went to Showdown",
        explanation: "% of hands that went to showdown when you saw the flop. 25-32% is healthy",
    },
    StatDefinition {
        abbrev: "W$SD",
        name: "Won $ at Showdown",
        explanation: "% of showdowns you won. >50% means you're showing down strong hands",
    },
    StatDefinition {
        abbrev: "AF",
        name: "Aggression Factor",
        explanation: "Ratio of (bets + raises) / calls. Higher = more aggressive. 2-3 is typical",
    },
];
