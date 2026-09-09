//! Seeded, replayable randomized invariant campaign for the multiway authority.

use std::collections::HashSet;
use std::fmt;

use rand::seq::SliceRandom;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

use super::actions::Action;
use super::command::SeatCommand;
use super::deck::Card;
use super::multiway::{build_pots, MultiwayHand, MultiwayLegalActions, MultiwayPhase};
use super::seat::{SeatId, TableSize};

pub const DEFAULT_CAMPAIGN_SEED: u64 = 0x5350_5249_4E54_3401;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignConfig {
    pub base_seed: u64,
    pub cases_per_occupancy: u16,
    pub max_actions_per_hand: u16,
}

impl Default for CampaignConfig {
    fn default() -> Self {
        Self {
            base_seed: DEFAULT_CAMPAIGN_SEED,
            cases_per_occupancy: 24,
            max_actions_per_hand: 256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignCaseReport {
    pub occupancy: u8,
    pub seed: u64,
    pub accepted_actions: u16,
    pub terminal_phase: MultiwayPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignReport {
    pub base_seed: u64,
    pub cases_per_occupancy: u16,
    pub cases: Vec<CampaignCaseReport>,
    pub accepted_actions: u32,
    pub showdowns: u32,
    pub folds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignFailure {
    pub occupancy: u8,
    pub seed: u64,
    pub invariant: String,
    pub accepted_commands: Vec<SeatCommand>,
}

impl fmt::Display for CampaignFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "campaign invariant '{}' failed at occupancy {} seed {} after {} accepted commands",
            self.invariant,
            self.occupancy,
            self.seed,
            self.accepted_commands.len()
        )
    }
}

impl std::error::Error for CampaignFailure {}

pub fn run_seeded_campaign(config: CampaignConfig) -> Result<CampaignReport, CampaignFailure> {
    let mut cases = Vec::new();
    let mut accepted_actions = 0u32;
    let mut showdowns = 0u32;
    let mut folds = 0u32;

    for occupancy in 2..=9 {
        for case_index in 0..config.cases_per_occupancy {
            let seed = case_seed(config.base_seed, occupancy, case_index);
            let case = run_case(occupancy, seed, config.max_actions_per_hand)?;
            accepted_actions += u32::from(case.accepted_actions);
            match case.terminal_phase {
                MultiwayPhase::Showdown => showdowns += 1,
                MultiwayPhase::HandComplete => folds += 1,
                _ => unreachable!("a successful campaign case is terminal"),
            }
            cases.push(case);
        }
    }

    Ok(CampaignReport {
        base_seed: config.base_seed,
        cases_per_occupancy: config.cases_per_occupancy,
        cases,
        accepted_actions,
        showdowns,
        folds,
    })
}

fn run_case(
    occupancy: u8,
    seed: u64,
    max_actions: u16,
) -> Result<CampaignCaseReport, CampaignFailure> {
    let mut rng = StdRng::seed_from_u64(seed);
    let table_size = TableSize::new(9).expect("nine-seat campaign table is valid");
    let mut physical: Vec<u8> = (0..9).collect();
    physical.shuffle(&mut rng);
    physical.truncate(usize::from(occupancy));
    physical.sort_unstable();
    let occupied: Vec<SeatId> = physical.into_iter().map(seat).collect();
    let button = *occupied
        .choose(&mut rng)
        .expect("campaign occupancy is at least two");
    let stacks: Vec<(SeatId, u32)> = occupied
        .iter()
        .copied()
        .map(|seat| (seat, rng.gen_range(20..=250)))
        .collect();
    let mut hand = MultiwayHand::new_seeded_for_review(table_size, button, &stacks, seed)
        .expect("campaign configuration is generated as valid");
    let mut commands = Vec::new();

    assert_invariants(&hand, occupancy, seed, &commands)?;
    while accepts_actions(hand.phase) {
        if commands.len() >= usize::from(max_actions) {
            return Err(failure(occupancy, seed, "bounded termination", &commands));
        }
        let actor = hand.to_act.ok_or_else(|| {
            failure(
                occupancy,
                seed,
                "active phase has authoritative actor",
                &commands,
            )
        })?;
        let legal = hand.legal_actions_for(actor).ok_or_else(|| {
            failure(
                occupancy,
                seed,
                "authoritative actor owns legal action metadata",
                &commands,
            )
        })?;

        assert_rejection_is_immutable(&mut hand, actor, &legal, occupancy, seed, &commands)?;
        let action = choose_action(&legal, &mut rng);
        let command = SeatCommand::new(actor, action);
        hand.apply_command(command).map_err(|_| {
            failure(
                occupancy,
                seed,
                "generated legal command is accepted",
                &commands,
            )
        })?;
        commands.push(command);
        assert_invariants(&hand, occupancy, seed, &commands)?;
    }

    if !matches!(
        hand.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        return Err(failure(
            occupancy,
            seed,
            "campaign reaches a terminal phase",
            &commands,
        ));
    }

    Ok(CampaignCaseReport {
        occupancy,
        seed,
        accepted_actions: commands.len() as u16,
        terminal_phase: hand.phase,
    })
}

fn assert_rejection_is_immutable(
    hand: &mut MultiwayHand,
    actor: SeatId,
    legal: &MultiwayLegalActions,
    occupancy: u8,
    seed: u64,
    commands: &[SeatCommand],
) -> Result<(), CampaignFailure> {
    let before = signature(hand);
    let invalid_target = legal
        .all_in_to
        .checked_add(1)
        .expect("campaign stacks cannot reach u32::MAX");
    if hand
        .apply_command(SeatCommand::new(actor, Action::AllIn(invalid_target)))
        .is_ok()
    {
        return Err(failure(
            occupancy,
            seed,
            "invalid all-in is rejected",
            commands,
        ));
    }
    if signature(hand) != before {
        return Err(failure(
            occupancy,
            seed,
            "rejected command leaves authoritative signature unchanged",
            commands,
        ));
    }
    Ok(())
}

fn assert_invariants(
    hand: &MultiwayHand,
    occupancy: u8,
    seed: u64,
    commands: &[SeatCommand],
) -> Result<(), CampaignFailure> {
    if hand.total_chips() != hand.initial_total() {
        return Err(failure(occupancy, seed, "chip conservation", commands));
    }

    let cards: Vec<Card> = hand
        .occupied_seats()
        .flat_map(|seat| hand.seat(seat).hole_cards.iter().copied())
        .chain(hand.board.iter().copied())
        .collect();
    if cards.iter().copied().collect::<HashSet<_>>().len() != cards.len() {
        return Err(failure(
            occupancy,
            seed,
            "physical card uniqueness",
            commands,
        ));
    }

    if accepts_actions(hand.phase) {
        let actor = hand.to_act.ok_or_else(|| {
            failure(
                occupancy,
                seed,
                "active phase has authoritative actor",
                commands,
            )
        })?;
        if !hand.seat(actor).can_act() || hand.legal_actions_for(actor).is_none() {
            return Err(failure(
                occupancy,
                seed,
                "actor is occupied live and actionable",
                commands,
            ));
        }
    }

    if hand.phase == MultiwayPhase::Showdown {
        let rebuilt = build_pots(&hand.settled_contributions);
        if rebuilt.pots != hand.pots || rebuilt.returned != hand.returned_excess {
            return Err(failure(
                occupancy,
                seed,
                "terminal pots equal contribution-layer reconstruction",
                commands,
            ));
        }
        if hand.awards.iter().any(|award| {
            award
                .winners
                .iter()
                .any(|winner| !award.eligible.contains(winner))
        }) {
            return Err(failure(
                occupancy,
                seed,
                "every winner is eligible for its pot",
                commands,
            ));
        }
    }
    Ok(())
}

fn choose_action(legal: &MultiwayLegalActions, rng: &mut StdRng) -> Action {
    let mut actions = Vec::with_capacity(6);
    if legal.can_fold {
        actions.push(Action::Fold);
    }
    if legal.can_check {
        actions.push(Action::Check);
    }
    if let Some(amount) = legal.call_amount {
        actions.push(Action::Call(amount));
    }
    if let Some(target) = legal.min_bet_to {
        actions.push(Action::Bet(target));
    }
    if let Some(target) = legal.min_raise_to {
        actions.push(Action::Raise(target));
    }
    actions.push(Action::AllIn(legal.all_in_to));
    *actions
        .choose(rng)
        .expect("an acting seat always has at least one legal action")
}

fn accepts_actions(phase: MultiwayPhase) -> bool {
    matches!(
        phase,
        MultiwayPhase::Preflop | MultiwayPhase::Flop | MultiwayPhase::Turn | MultiwayPhase::River
    )
}

fn signature(hand: &MultiwayHand) -> String {
    serde_json::to_string(&(
        (
            hand.phase,
            hand.table_size,
            &hand.seats,
            &hand.board,
            hand.button,
            hand.small_blind,
            hand.big_blind,
            hand.to_act,
            hand.current_wager,
        ),
        (
            hand.last_full_raise_size,
            &hand.action_history,
            &hand.pots,
            &hand.returned_excess,
            &hand.awards,
            &hand.revealed_hands,
            &hand.settled_contributions,
            hand.initial_total(),
        ),
    ))
    .expect("authoritative signature fields serialize")
}

fn case_seed(base_seed: u64, occupancy: u8, case_index: u16) -> u64 {
    base_seed
        ^ (u64::from(occupancy) << 56)
        ^ u64::from(case_index).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

fn failure(occupancy: u8, seed: u64, invariant: &str, commands: &[SeatCommand]) -> CampaignFailure {
    CampaignFailure {
        occupancy,
        seed,
        invariant: invariant.to_string(),
        accepted_commands: commands.to_vec(),
    }
}

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("campaign physical seat is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_campaign_covers_every_occupancy_and_is_reproducible() {
        let config = CampaignConfig {
            cases_per_occupancy: 16,
            ..CampaignConfig::default()
        };
        let first = run_seeded_campaign(config).unwrap();
        let second = run_seeded_campaign(config).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.cases.len(), 8 * 16);
        for occupancy in 2..=9 {
            assert_eq!(
                first
                    .cases
                    .iter()
                    .filter(|case| case.occupancy == occupancy)
                    .count(),
                16
            );
        }
        assert_eq!(first.showdowns + first.folds, first.cases.len() as u32);
        assert!(first.accepted_actions > first.cases.len() as u32);
    }

    #[test]
    fn failure_contains_exact_replay_identity_and_command_prefix() {
        let config = CampaignConfig {
            base_seed: 77,
            cases_per_occupancy: 1,
            max_actions_per_hand: 0,
        };
        let failure = run_seeded_campaign(config).unwrap_err();
        assert_eq!(failure.occupancy, 2);
        assert_eq!(failure.seed, case_seed(77, 2, 0));
        assert_eq!(failure.invariant, "bounded termination");
        assert!(failure.accepted_commands.is_empty());
        assert!(failure.to_string().contains(&failure.seed.to_string()));
    }
}
