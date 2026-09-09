//! Projection-native baseline policies.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;

use crate::game::deck::{Card, Rank, Suit};
use crate::game::hand::{evaluate_hand, HandEvaluation};
use crate::game::table::HandParticipation;

use super::action::{legal_policy_actions, PolicyActionError, PolicyActionV1};
use super::observation::PolicyObservationV1;

pub trait Policy {
    fn select_action(
        &mut self,
        observation: &PolicyObservationV1,
    ) -> Result<PolicyActionV1, PolicyActionError>;
}

#[derive(Debug, Default)]
pub struct CheckCallPolicy;

impl Policy for CheckCallPolicy {
    fn select_action(
        &mut self,
        observation: &PolicyObservationV1,
    ) -> Result<PolicyActionV1, PolicyActionError> {
        if observation.legal_actions.can_check {
            return Ok(PolicyActionV1::Check);
        }
        if observation.amount_to_call > 0 {
            return Ok(PolicyActionV1::Call);
        }
        Err(PolicyActionError::Masked(PolicyActionV1::Check))
    }
}

#[derive(Debug)]
pub struct RandomLegalPolicy {
    rng: StdRng,
}

impl RandomLegalPolicy {
    pub fn seeded(seed: u64) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
        }
    }
}

impl Policy for RandomLegalPolicy {
    fn select_action(
        &mut self,
        observation: &PolicyObservationV1,
    ) -> Result<PolicyActionV1, PolicyActionError> {
        let legal = legal_policy_actions(observation);
        legal
            .choose(&mut self.rng)
            .copied()
            .ok_or(PolicyActionError::Masked(PolicyActionV1::Check))
    }
}

/// Checks when free and folds to any wager.
///
/// This deliberately exploitable policy is useful for detecting candidates
/// that fail to apply pressure to an over-folding opponent.
#[derive(Debug, Default)]
pub struct FoldCheckPolicy;

impl Policy for FoldCheckPolicy {
    fn select_action(
        &mut self,
        observation: &PolicyObservationV1,
    ) -> Result<PolicyActionV1, PolicyActionError> {
        if observation.legal_actions.can_check {
            Ok(PolicyActionV1::Check)
        } else if observation.legal_actions.can_fold {
            Ok(PolicyActionV1::Fold)
        } else {
            passive_fallback(observation)
        }
    }
}

/// Bets or raises the pot whenever possible, then checks or calls.
///
/// This exposes policies that over-fold or fail to defend against sustained,
/// non-all-in aggression.
#[derive(Debug, Default)]
pub struct PotPressurePolicy;

impl Policy for PotPressurePolicy {
    fn select_action(
        &mut self,
        observation: &PolicyObservationV1,
    ) -> Result<PolicyActionV1, PolicyActionError> {
        prefer_legal(
            observation,
            &[
                PolicyActionV1::BetRaisePot,
                PolicyActionV1::BetRaiseThreeQuarterPot,
                PolicyActionV1::BetRaiseHalfPot,
            ],
        )
        .map_or_else(|| passive_fallback(observation), Ok)
    }
}

/// Moves all-in whenever the abstract all-in action is legal.
#[derive(Debug, Default)]
pub struct JamPolicy;

impl Policy for JamPolicy {
    fn select_action(
        &mut self,
        observation: &PolicyObservationV1,
    ) -> Result<PolicyActionV1, PolicyActionError> {
        prefer_legal(observation, &[PolicyActionV1::AllIn])
            .map_or_else(|| passive_fallback(observation), Ok)
    }
}

/// A seeded uniform-range Monte Carlo equity policy.
///
/// Opponent cards and future board cards are sampled only from cards that are
/// unseen in the authorized observation. The actual deal plan and hidden
/// opponent cards are never consulted. Calls use observed pot odds; large
/// equity edges value-bet or raise using fixed pot fractions.
#[derive(Debug)]
pub struct EquityPotOddsPolicy {
    rng: StdRng,
    samples: u32,
}

impl EquityPotOddsPolicy {
    pub fn seeded(seed: u64, samples: u32) -> Self {
        Self {
            rng: StdRng::seed_from_u64(seed),
            samples: samples.max(1),
        }
    }
}

impl Policy for EquityPotOddsPolicy {
    fn select_action(
        &mut self,
        observation: &PolicyObservationV1,
    ) -> Result<PolicyActionV1, PolicyActionError> {
        let equity = estimate_uniform_equity(observation, self.samples, &mut self.rng);
        let pot_odds = observation
            .pot_odds_millionths
            .map_or(0.0, |value| f64::from(value) / 1_000_000.0);

        if observation.amount_to_call > 0 {
            if equity >= (pot_odds + 0.20).max(0.65) {
                if let Some(action) = prefer_legal(
                    observation,
                    &[
                        PolicyActionV1::BetRaisePot,
                        PolicyActionV1::BetRaiseThreeQuarterPot,
                        PolicyActionV1::BetRaiseHalfPot,
                    ],
                ) {
                    return Ok(action);
                }
            }
            if equity + f64::EPSILON >= pot_odds {
                return prefer_legal(observation, &[PolicyActionV1::Call])
                    .map_or_else(|| passive_fallback(observation), Ok);
            }
            if observation.legal_actions.can_fold {
                return Ok(PolicyActionV1::Fold);
            }
            return passive_fallback(observation);
        }

        let value_sizes: &[PolicyActionV1] = if equity >= 0.75 {
            &[
                PolicyActionV1::BetRaisePot,
                PolicyActionV1::BetRaiseThreeQuarterPot,
                PolicyActionV1::BetRaiseHalfPot,
            ]
        } else if equity >= 0.58 {
            &[
                PolicyActionV1::BetRaiseHalfPot,
                PolicyActionV1::BetRaiseQuarterPot,
            ]
        } else {
            &[]
        };
        prefer_legal(observation, value_sizes).map_or_else(|| passive_fallback(observation), Ok)
    }
}

/// Estimates showdown pot share against uniformly sampled legal holdings.
///
/// Ties contribute the hero's fractional share of the pot. Folded and undealt
/// seats are excluded. This is intentionally a small, auditable baseline rather
/// than an opponent-range model.
pub fn estimate_uniform_equity(
    observation: &PolicyObservationV1,
    samples: u32,
    rng: &mut StdRng,
) -> f64 {
    let opponent_count = observation
        .seats
        .iter()
        .filter(|seat| seat.seat != observation.acting_seat)
        .filter(|seat| {
            matches!(
                seat.participation,
                HandParticipation::Live | HandParticipation::AllIn
            )
        })
        .count();
    if opponent_count == 0 {
        return 1.0;
    }

    let mut unseen = full_deck();
    unseen
        .retain(|card| !observation.hole_cards.contains(card) && !observation.board.contains(card));
    let board_needed = 5usize.saturating_sub(observation.board.len());
    let cards_needed = opponent_count
        .saturating_mul(2)
        .saturating_add(board_needed);
    if samples == 0 || unseen.len() < cards_needed {
        return 0.0;
    }

    let mut total_share = 0.0;
    for _ in 0..samples {
        unseen.shuffle(rng);
        let mut cursor = 0;
        let mut opponent_holes = Vec::with_capacity(opponent_count);
        for _ in 0..opponent_count {
            opponent_holes.push([unseen[cursor], unseen[cursor + 1]]);
            cursor += 2;
        }
        let mut board = observation.board.clone();
        board.extend_from_slice(&unseen[cursor..cursor + board_needed]);

        let hero = evaluate_hand(&observation.hole_cards, &board);
        let opponents = opponent_holes
            .iter()
            .map(|cards| evaluate_hand(cards, &board))
            .collect::<Vec<_>>();
        let hero_beaten = opponents
            .iter()
            .any(|opponent| compare_evaluations(opponent, &hero).is_gt());
        if !hero_beaten {
            let tied_opponents = opponents
                .iter()
                .filter(|opponent| compare_evaluations(opponent, &hero).is_eq())
                .count();
            total_share += 1.0 / (tied_opponents + 1) as f64;
        }
    }
    total_share / f64::from(samples)
}

fn full_deck() -> Vec<Card> {
    let mut cards = Vec::with_capacity(52);
    for suit in [Suit::Spades, Suit::Hearts, Suit::Diamonds, Suit::Clubs] {
        for rank in Rank::ALL {
            cards.push(Card::new(rank, suit));
        }
    }
    cards
}

fn compare_evaluations(left: &HandEvaluation, right: &HandEvaluation) -> std::cmp::Ordering {
    left.rank
        .cmp(&right.rank)
        .then_with(|| left.kickers.cmp(&right.kickers))
}

fn prefer_legal(
    observation: &PolicyObservationV1,
    preferences: &[PolicyActionV1],
) -> Option<PolicyActionV1> {
    let legal = legal_policy_actions(observation);
    preferences
        .iter()
        .find(|action| legal.contains(action))
        .copied()
}

fn passive_fallback(
    observation: &PolicyObservationV1,
) -> Result<PolicyActionV1, PolicyActionError> {
    prefer_legal(
        observation,
        &[
            PolicyActionV1::Check,
            PolicyActionV1::Call,
            PolicyActionV1::AllIn,
            PolicyActionV1::Fold,
        ],
    )
    .ok_or(PolicyActionError::Masked(PolicyActionV1::Check))
}
