use super::models::PlayerStats;
use std::fs;
use std::path::PathBuf;

const APP_NAME: &str = "terminal-poker";
const STATS_FILE: &str = "stats.json";

pub struct StatsStore {
    pub stats: PlayerStats,
    path: PathBuf,
}

impl StatsStore {
    pub fn load_or_create() -> Self {
        let path = Self::stats_path();

        let stats = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(contents) => {
                    match serde_json::from_str(&contents) {
                        Ok(stats) => stats,
                        Err(e) => {
                            eprintln!("Warning: Could not parse stats file, starting fresh: {}", e);
                            PlayerStats::default()
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Could not read stats file, starting fresh: {}", e);
                    PlayerStats::default()
                }
            }
        } else {
            PlayerStats::default()
        };

        Self { stats, path }
    }

    pub fn save(&self) {
        if let Some(parent) = self.path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Warning: Could not create stats directory: {}", e);
                return;
            }
        }

        match serde_json::to_string_pretty(&self.stats) {
            Ok(json) => {
                if let Err(e) = fs::write(&self.path, json) {
                    eprintln!("Warning: Could not save stats: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Warning: Could not serialize stats: {}", e);
            }
        }
    }

    fn stats_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME)
            .join(STATS_FILE)
    }

    pub fn record_hand_start(&mut self) {
        self.stats.total_hands += 1;
    }

    pub fn record_vpip(&mut self) {
        self.stats.vpip_hands += 1;
    }

    pub fn record_pfr(&mut self) {
        self.stats.pfr_hands += 1;
    }

    pub fn record_bet(&mut self) {
        self.stats.bets += 1;
    }

    pub fn record_raise(&mut self) {
        self.stats.raises += 1;
    }

    pub fn record_call(&mut self) {
        self.stats.calls += 1;
    }

    pub fn record_saw_flop(&mut self) {
        self.stats.wtsd_opportunities += 1;
    }

    pub fn record_showdown(&mut self, won: bool) {
        self.stats.wtsd_hands += 1;
        if won {
            self.stats.wsd_hands += 1;
        }
    }

    pub fn record_profit(&mut self, amount: i64) {
        self.stats.total_profit_chips += amount;
    }

    pub fn record_pot_won(&mut self, pot: u32) {
        if pot > self.stats.biggest_pot_won {
            self.stats.biggest_pot_won = pot;
        }
    }

    pub fn record_pot_lost(&mut self, pot: u32) {
        if pot > self.stats.biggest_pot_lost {
            self.stats.biggest_pot_lost = pot;
        }
    }

    pub fn record_session_end(&mut self) {
        self.stats.total_sessions += 1;
    }
}
