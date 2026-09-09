//! Server-authoritative single-table freezeout structure and progression.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::game::multiway::BlindValues;
use crate::game::seat::{PlayerId, SeatId};

pub const MAX_LEVELS: usize = 64;
pub const PAYOUT_BASIS_POINTS: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentLevel {
    pub small_blind: u32,
    pub big_blind: u32,
    pub ante: u32,
    pub duration_seconds: u32,
    pub break_after_seconds: u32,
}

impl TournamentLevel {
    pub const fn blinds(self) -> BlindValues {
        BlindValues {
            small_blind: self.small_blind,
            big_blind: self.big_blind,
            ante: self.ante,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentPayoutPlan {
    pub pool: u32,
    pub shares_bps: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentConfig {
    pub name: String,
    pub entrants: u8,
    pub starting_stack: u32,
    pub levels: Vec<TournamentLevel>,
    pub payout: TournamentPayoutPlan,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub join_code: String,
}

impl TournamentConfig {
    pub fn recommended(entrants: u8, join_code: impl Into<String>) -> Self {
        Self {
            name: "Sneaky Freezeout".to_string(),
            entrants,
            starting_stack: 3_000,
            levels: vec![
                TournamentLevel {
                    small_blind: 25,
                    big_blind: 50,
                    ante: 0,
                    duration_seconds: 600,
                    break_after_seconds: 0,
                },
                TournamentLevel {
                    small_blind: 50,
                    big_blind: 100,
                    ante: 10,
                    duration_seconds: 600,
                    break_after_seconds: 300,
                },
                TournamentLevel {
                    small_blind: 100,
                    big_blind: 200,
                    ante: 25,
                    duration_seconds: 600,
                    break_after_seconds: 0,
                },
            ],
            payout: TournamentPayoutPlan {
                pool: 1_000,
                shares_bps: vec![PAYOUT_BASIS_POINTS],
            },
            join_code: join_code.into(),
        }
    }

    pub fn validate(&self) -> Result<(), TournamentError> {
        if !(2..=9).contains(&self.entrants) {
            return Err(TournamentError::InvalidEntrants);
        }
        if self.name.trim().is_empty() || self.name.len() > 32 {
            return Err(TournamentError::InvalidName);
        }
        if !self.join_code.is_empty()
            && (!(4..=96).contains(&self.join_code.len())
                || !self
                    .join_code
                    .bytes()
                    .all(|byte| byte.is_ascii_graphic() || byte == b' '))
        {
            return Err(TournamentError::InvalidJoinCode);
        }
        if self.levels.is_empty() || self.levels.len() > MAX_LEVELS {
            return Err(TournamentError::InvalidLevels);
        }
        let mut prior = (0, 0, 0);
        for level in &self.levels {
            if BlindValues::new(level.small_blind, level.big_blind, level.ante).is_none()
                || !(60..=7_200).contains(&level.duration_seconds)
                || level.break_after_seconds > 1_800
                || (level.small_blind, level.big_blind, level.ante) < prior
            {
                return Err(TournamentError::InvalidLevels);
            }
            prior = (level.small_blind, level.big_blind, level.ante);
        }
        if !(100..=1_000_000).contains(&self.starting_stack)
            || self.starting_stack < self.levels[0].big_blind.saturating_mul(20)
        {
            return Err(TournamentError::InvalidStartingStack);
        }
        if !(1..=1_000_000).contains(&self.payout.pool)
            || self.payout.shares_bps.is_empty()
            || self.payout.shares_bps.len() > 3
            || self.payout.shares_bps.len() > usize::from(self.entrants)
            || self.payout.shares_bps.contains(&0)
            || self.payout.shares_bps.iter().sum::<u32>() != PAYOUT_BASIS_POINTS
        {
            return Err(TournamentError::InvalidPayout);
        }
        Ok(())
    }

    pub fn public_copy(&self) -> Self {
        let mut public = self.clone();
        public.join_code.clear();
        public
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TournamentStatus {
    Registering,
    Running,
    Break,
    Complete,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentEntrant {
    pub player: PlayerId,
    pub seat: SeatId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentStanding {
    pub player: PlayerId,
    pub seat: SeatId,
    pub place: u8,
    pub payout: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentPublicState {
    pub status: TournamentStatus,
    pub entrants: u8,
    pub registered: u8,
    pub remaining: u8,
    pub level_number: u8,
    pub level: TournamentLevel,
    pub level_remaining_millis: u64,
    pub break_remaining_millis: u64,
    pub hands_completed: u64,
    pub standings: Vec<TournamentStanding>,
    pub payout_pool: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TournamentController {
    config: TournamentConfig,
    status: TournamentStatus,
    entrants: BTreeMap<PlayerId, SeatId>,
    remaining: BTreeSet<PlayerId>,
    level_index: usize,
    level_elapsed_millis: u64,
    break_remaining_millis: u64,
    hands_completed: u64,
    standings: Vec<TournamentStanding>,
}

impl TournamentController {
    pub fn new(mut config: TournamentConfig) -> Result<Self, TournamentError> {
        config.validate()?;
        // Access material belongs to the registry verifier boundary, never the
        // durable/public tournament controller.
        config.join_code.clear();
        Ok(Self {
            config,
            status: TournamentStatus::Registering,
            entrants: BTreeMap::new(),
            remaining: BTreeSet::new(),
            level_index: 0,
            level_elapsed_millis: 0,
            break_remaining_millis: 0,
            hands_completed: 0,
            standings: Vec::new(),
        })
    }

    pub const fn status(&self) -> TournamentStatus {
        self.status
    }

    pub fn config(&self) -> &TournamentConfig {
        &self.config
    }

    pub fn register(&mut self, entrant: TournamentEntrant) -> Result<(), TournamentError> {
        if self.status != TournamentStatus::Registering {
            return Err(TournamentError::ConfigurationLocked);
        }
        if self.entrants.len() >= usize::from(self.config.entrants) {
            return Err(TournamentError::RegistrationFull);
        }
        if self.entrants.contains_key(&entrant.player)
            || self.entrants.values().any(|seat| *seat == entrant.seat)
        {
            return Err(TournamentError::DuplicateRegistration);
        }
        if entrant.seat.as_u8() >= self.config.entrants {
            return Err(TournamentError::InvalidSeat);
        }
        self.entrants.insert(entrant.player, entrant.seat);
        self.remaining.insert(entrant.player);
        Ok(())
    }

    pub fn unregister(&mut self, player: PlayerId) -> Result<(), TournamentError> {
        if self.status != TournamentStatus::Registering {
            return Err(TournamentError::ConfigurationLocked);
        }
        self.entrants.remove(&player);
        self.remaining.remove(&player);
        Ok(())
    }

    pub fn start(&mut self) -> Result<(), TournamentError> {
        if self.status != TournamentStatus::Registering {
            return Err(TournamentError::ConfigurationLocked);
        }
        if self.entrants.len() != usize::from(self.config.entrants) {
            return Err(TournamentError::RegistrationIncomplete);
        }
        self.status = TournamentStatus::Running;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), TournamentError> {
        if self.status != TournamentStatus::Registering {
            return Err(TournamentError::ConfigurationLocked);
        }
        self.status = TournamentStatus::Cancelled;
        Ok(())
    }

    pub fn current_blinds(&self) -> BlindValues {
        self.config.levels[self.level_index].blinds()
    }

    pub fn tick_between_hands(&mut self, elapsed_millis: u64) {
        if !matches!(
            self.status,
            TournamentStatus::Running | TournamentStatus::Break
        ) {
            return;
        }
        let mut remaining_elapsed = elapsed_millis;
        while remaining_elapsed > 0 {
            if self.status == TournamentStatus::Break {
                let consumed = remaining_elapsed.min(self.break_remaining_millis);
                self.break_remaining_millis -= consumed;
                remaining_elapsed -= consumed;
                if self.break_remaining_millis == 0 {
                    self.status = TournamentStatus::Running;
                }
                continue;
            }
            let duration = u64::from(self.config.levels[self.level_index].duration_seconds)
                .saturating_mul(1_000);
            let to_boundary = duration.saturating_sub(self.level_elapsed_millis);
            let consumed = remaining_elapsed.min(to_boundary);
            self.level_elapsed_millis += consumed;
            remaining_elapsed -= consumed;
            if self.level_elapsed_millis >= duration {
                self.level_elapsed_millis = 0;
                let completed = self.config.levels[self.level_index];
                self.level_index = (self.level_index + 1).min(self.config.levels.len() - 1);
                if completed.break_after_seconds > 0 {
                    self.status = TournamentStatus::Break;
                    self.break_remaining_millis =
                        u64::from(completed.break_after_seconds).saturating_mul(1_000);
                }
            }
        }
    }

    pub fn complete_hand(
        &mut self,
        starting_stacks: &[(SeatId, u32)],
        final_stacks: &[(SeatId, u32)],
    ) -> Result<(), TournamentError> {
        if self.status == TournamentStatus::Complete {
            return Err(TournamentError::AlreadyComplete);
        }
        if !matches!(
            self.status,
            TournamentStatus::Running | TournamentStatus::Break
        ) {
            return Err(TournamentError::NotRunning);
        }
        let before = starting_stacks.iter().copied().collect::<BTreeMap<_, _>>();
        let after = final_stacks.iter().copied().collect::<BTreeMap<_, _>>();
        if before.len() != after.len() || before.keys().any(|seat| !after.contains_key(seat)) {
            return Err(TournamentError::InvalidHandResult);
        }
        let mut busted = before
            .iter()
            .filter_map(|(&seat, &stack)| {
                (stack > 0 && after.get(&seat) == Some(&0)).then_some((seat, stack))
            })
            .collect::<Vec<_>>();
        busted.sort_by_key(|(seat, stack)| (*stack, seat.as_u8()));
        let remaining_before = self.remaining.len();
        for (index, (seat, _)) in busted.into_iter().enumerate() {
            let player = self
                .entrants
                .iter()
                .find_map(|(&player, &registered)| (registered == seat).then_some(player))
                .ok_or(TournamentError::InvalidHandResult)?;
            if !self.remaining.remove(&player) {
                return Err(TournamentError::DuplicateElimination);
            }
            self.standings.push(TournamentStanding {
                player,
                seat,
                place: u8::try_from(remaining_before.saturating_sub(index))
                    .map_err(|_| TournamentError::InvalidHandResult)?,
                payout: 0,
            });
        }
        self.hands_completed = self.hands_completed.saturating_add(1);
        if self.remaining.len() == 1 {
            let winner = *self.remaining.iter().next().expect("one winner remains");
            let seat = self.entrants[&winner];
            self.standings.push(TournamentStanding {
                player: winner,
                seat,
                place: 1,
                payout: 0,
            });
            self.apply_payouts();
            self.status = TournamentStatus::Complete;
        }
        Ok(())
    }

    fn apply_payouts(&mut self) {
        let plan = &self.config.payout;
        let mut awards = plan
            .shares_bps
            .iter()
            .enumerate()
            .map(|(index, &share)| {
                let numerator = u64::from(plan.pool) * u64::from(share);
                (
                    u8::try_from(index + 1).expect("at most three paid places"),
                    u32::try_from(numerator / u64::from(PAYOUT_BASIS_POINTS))
                        .expect("bounded pool fits u32"),
                    numerator % u64::from(PAYOUT_BASIS_POINTS),
                )
            })
            .collect::<Vec<_>>();
        let assigned = awards.iter().map(|(_, amount, _)| *amount).sum::<u32>();
        let mut remainder = plan.pool - assigned;
        awards.sort_by_key(|(place, _, fraction)| (std::cmp::Reverse(*fraction), *place));
        for (_, amount, _) in &mut awards {
            if remainder == 0 {
                break;
            }
            *amount += 1;
            remainder -= 1;
        }
        for standing in &mut self.standings {
            standing.payout = awards
                .iter()
                .find_map(|(place, amount, _)| (*place == standing.place).then_some(*amount))
                .unwrap_or(0);
        }
    }

    pub fn public_state(&self) -> TournamentPublicState {
        let level = self.config.levels[self.level_index];
        let duration = u64::from(level.duration_seconds).saturating_mul(1_000);
        let mut standings = self.standings.clone();
        standings.sort_by_key(|standing| standing.place);
        TournamentPublicState {
            status: self.status,
            entrants: self.config.entrants,
            registered: u8::try_from(self.entrants.len()).expect("entrants are bounded to nine"),
            remaining: u8::try_from(self.remaining.len()).expect("entrants are bounded to nine"),
            level_number: u8::try_from(self.level_index + 1).expect("levels are bounded to 64"),
            level,
            level_remaining_millis: duration.saturating_sub(self.level_elapsed_millis),
            break_remaining_millis: self.break_remaining_millis,
            hands_completed: self.hands_completed,
            standings,
            payout_pool: self.config.payout.pool,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TournamentError {
    InvalidName,
    InvalidEntrants,
    InvalidJoinCode,
    InvalidStartingStack,
    InvalidLevels,
    InvalidPayout,
    ConfigurationLocked,
    RegistrationFull,
    RegistrationIncomplete,
    DuplicateRegistration,
    InvalidSeat,
    NotRunning,
    AlreadyComplete,
    InvalidHandResult,
    DuplicateElimination,
}

impl fmt::Display for TournamentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidName => "tournament name must contain 1-32 bytes",
            Self::InvalidEntrants => "single-table tournament requires 2-9 entrants",
            Self::InvalidJoinCode => "private invite verifier must contain 24-96 visible bytes",
            Self::InvalidStartingStack => "starting stack is outside bounds or below 20 big blinds",
            Self::InvalidLevels => "blind/ante level schedule is invalid",
            Self::InvalidPayout => "play-money payout plan is invalid",
            Self::ConfigurationLocked => "tournament configuration and registration are locked",
            Self::RegistrationFull => "tournament registration is full",
            Self::RegistrationIncomplete => "all configured entrants must register before start",
            Self::DuplicateRegistration => "player or seat is already registered",
            Self::InvalidSeat => "seat is outside tournament capacity",
            Self::NotRunning => "tournament is not running",
            Self::AlreadyComplete => "tournament is already complete",
            Self::InvalidHandResult => "hand result does not match registered tournament seats",
            Self::DuplicateElimination => "entrant was eliminated more than once",
        })
    }
}

impl std::error::Error for TournamentError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn seat(index: u8) -> SeatId {
        SeatId::new(index).unwrap()
    }

    fn config(entrants: u8) -> TournamentConfig {
        TournamentConfig::recommended(entrants, "ABCDEFGHIJKLMNOPQRSTUVWX")
    }

    #[test]
    fn recommended_structure_is_bounded_and_public_copy_hides_invite() {
        let config = config(9);
        config.validate().unwrap();
        let json = serde_json::to_string(&config.public_copy()).unwrap();
        assert!(!json.contains("ABCDEFGHIJKLMNOPQRSTUVWX"));
        assert!(json.contains("3000"));
    }

    #[test]
    fn registration_is_idempotent_by_identity_and_seat_and_locks_at_start() {
        let mut tournament = TournamentController::new(config(2)).unwrap();
        tournament
            .register(TournamentEntrant {
                player: PlayerId::new(1),
                seat: seat(0),
            })
            .unwrap();
        assert_eq!(
            tournament.register(TournamentEntrant {
                player: PlayerId::new(1),
                seat: seat(1),
            }),
            Err(TournamentError::DuplicateRegistration)
        );
        tournament
            .register(TournamentEntrant {
                player: PlayerId::new(2),
                seat: seat(1),
            })
            .unwrap();
        tournament.start().unwrap();
        assert_eq!(
            tournament.register(TournamentEntrant {
                player: PlayerId::new(3),
                seat: seat(2),
            }),
            Err(TournamentError::ConfigurationLocked)
        );
    }

    #[test]
    fn levels_and_breaks_advance_only_through_authoritative_tick() {
        let mut tournament = TournamentController::new(config(2)).unwrap();
        for index in 0..2 {
            tournament
                .register(TournamentEntrant {
                    player: PlayerId::new(index + 1),
                    seat: seat(index as u8),
                })
                .unwrap();
        }
        tournament.start().unwrap();
        tournament.tick_between_hands(1_200_000);
        let state = tournament.public_state();
        assert_eq!(state.status, TournamentStatus::Break);
        assert_eq!(state.level_number, 3);
        assert_eq!(state.break_remaining_millis, 300_000);
        tournament.tick_between_hands(300_000);
        assert_eq!(tournament.status(), TournamentStatus::Running);
    }

    #[test]
    fn same_hand_busts_are_ordered_and_integer_payouts_reconcile() {
        let mut config = config(3);
        config.payout = TournamentPayoutPlan {
            pool: 101,
            shares_bps: vec![5_000, 3_000, 2_000],
        };
        let mut tournament = TournamentController::new(config).unwrap();
        for index in 0..3 {
            tournament
                .register(TournamentEntrant {
                    player: PlayerId::new(index + 1),
                    seat: seat(index as u8),
                })
                .unwrap();
        }
        tournament.start().unwrap();
        tournament
            .complete_hand(
                &[(seat(0), 100), (seat(1), 200), (seat(2), 300)],
                &[(seat(0), 0), (seat(1), 0), (seat(2), 600)],
            )
            .unwrap();
        let state = tournament.public_state();
        assert_eq!(state.status, TournamentStatus::Complete);
        assert_eq!(
            state.standings.iter().map(|s| s.place).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(state.standings.iter().map(|s| s.payout).sum::<u32>(), 101);
        assert_eq!(state.standings[0].player, PlayerId::new(3));
        assert_eq!(state.standings[2].player, PlayerId::new(1));
    }

    #[test]
    fn controller_checkpoint_round_trip_preserves_exact_state() {
        let mut tournament = TournamentController::new(config(2)).unwrap();
        tournament
            .register(TournamentEntrant {
                player: PlayerId::new(1),
                seat: seat(0),
            })
            .unwrap();
        let encoded = serde_json::to_vec(&tournament).unwrap();
        let restored: TournamentController = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored, tournament);
    }
}
