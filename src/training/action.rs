//! Versioned discrete policy actions and their authoritative legal mapper.

use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::game::actions::Action;

use super::observation::PolicyObservationV1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyActionV1 {
    Fold,
    Check,
    Call,
    BetRaiseQuarterPot,
    BetRaiseHalfPot,
    BetRaiseThreeQuarterPot,
    BetRaisePot,
    BetRaiseOneAndHalfPot,
    AllIn,
}

impl PolicyActionV1 {
    pub const ALL: [Self; 9] = [
        Self::Fold,
        Self::Check,
        Self::Call,
        Self::BetRaiseQuarterPot,
        Self::BetRaiseHalfPot,
        Self::BetRaiseThreeQuarterPot,
        Self::BetRaisePot,
        Self::BetRaiseOneAndHalfPot,
        Self::AllIn,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyActionError {
    Masked(PolicyActionV1),
}

impl Display for PolicyActionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Masked(action) => write!(formatter, "policy action {action:?} is masked"),
        }
    }
}

impl Error for PolicyActionError {}

pub fn legal_policy_actions(observation: &PolicyObservationV1) -> Vec<PolicyActionV1> {
    PolicyActionV1::ALL
        .into_iter()
        .filter(|action| map_policy_action(observation, *action).is_ok())
        .collect()
}

/// Maps an abstract action to one exact domain action.
///
/// Pot fractions use integer ceiling. Bet targets are the actor's current
/// contribution plus the fraction. Raise targets are the current wager plus
/// the fraction of the pot after calling. Targets clamp upward to the minimum;
/// reaching the maximum stack maps explicitly to `AllIn`.
pub fn map_policy_action(
    observation: &PolicyObservationV1,
    policy_action: PolicyActionV1,
) -> Result<Action, PolicyActionError> {
    let legal = &observation.legal_actions;
    let contribution = observation.acting_seat_state().street_contribution;
    match policy_action {
        PolicyActionV1::Fold if legal.can_fold => Ok(Action::Fold),
        PolicyActionV1::Check if legal.can_check => Ok(Action::Check),
        PolicyActionV1::Call => {
            if let Some(amount) = legal.call_amount {
                Ok(Action::Call(amount))
            } else if observation.amount_to_call > 0 && legal.all_in_to > contribution {
                Ok(Action::AllIn(legal.all_in_to))
            } else {
                Err(PolicyActionError::Masked(policy_action))
            }
        }
        PolicyActionV1::BetRaiseQuarterPot => map_fraction(observation, policy_action, 1, 4),
        PolicyActionV1::BetRaiseHalfPot => map_fraction(observation, policy_action, 1, 2),
        PolicyActionV1::BetRaiseThreeQuarterPot => map_fraction(observation, policy_action, 3, 4),
        PolicyActionV1::BetRaisePot => map_fraction(observation, policy_action, 1, 1),
        PolicyActionV1::BetRaiseOneAndHalfPot => map_fraction(observation, policy_action, 3, 2),
        PolicyActionV1::AllIn if legal.all_in_to > contribution => {
            Ok(Action::AllIn(legal.all_in_to))
        }
        _ => Err(PolicyActionError::Masked(policy_action)),
    }
}

fn map_fraction(
    observation: &PolicyObservationV1,
    policy_action: PolicyActionV1,
    numerator: u32,
    denominator: u32,
) -> Result<Action, PolicyActionError> {
    let legal = &observation.legal_actions;
    let contribution = observation.acting_seat_state().street_contribution;
    let (minimum, desired, is_bet) = if let Some(minimum) = legal.min_bet_to {
        let increment = ceil_fraction(observation.pot_total, numerator, denominator);
        (minimum, contribution.saturating_add(increment), true)
    } else if let Some(minimum) = legal.min_raise_to {
        let pot_after_call = observation
            .pot_total
            .saturating_add(observation.amount_to_call);
        let increment = ceil_fraction(pot_after_call, numerator, denominator);
        (
            minimum,
            observation.current_wager.saturating_add(increment),
            false,
        )
    } else if legal.all_in_to > contribution {
        return Ok(Action::AllIn(legal.all_in_to));
    } else {
        return Err(PolicyActionError::Masked(policy_action));
    };

    let target = desired.max(minimum);
    if target >= legal.all_in_to {
        return Ok(Action::AllIn(legal.all_in_to));
    }
    let maximum_non_all_in = legal.all_in_to.saturating_sub(1);
    let target = target.min(maximum_non_all_in);
    if target < minimum {
        return Ok(Action::AllIn(legal.all_in_to));
    }
    if is_bet {
        Ok(Action::Bet(target))
    } else {
        Ok(Action::Raise(target))
    }
}

fn ceil_fraction(value: u32, numerator: u32, denominator: u32) -> u32 {
    let product = u64::from(value) * u64::from(numerator);
    let rounded = product.div_ceil(u64::from(denominator));
    u32::try_from(rounded).unwrap_or(u32::MAX)
}
