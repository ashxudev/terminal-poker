use std::fmt;

use serde::{Deserialize, Serialize};

use super::actions::Action;
use super::seat::SeatId;

/// A controller request expressed entirely in domain terms.
///
/// Offline input, bots, and future network sessions all cross this same
/// boundary before authoritative state can change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeatCommand {
    pub seat: SeatId,
    pub action: Action,
}

impl SeatCommand {
    pub const fn new(seat: SeatId, action: Action) -> Self {
        Self { seat, action }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    FoldNotAllowed,
    CheckNotAllowed,
    CallNotAllowed,
    InvalidCall { expected: u32, actual: u32 },
    BetNotAllowed,
    BetOutOfRange { min: u32, max: u32, actual: u32 },
    RaiseNotAllowed,
    RaiseNotReopened,
    RaiseOutOfRange { min: u32, max: u32, actual: u32 },
    InvalidAllIn { expected: u32, actual: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    HandNotActive,
    SeatNotOccupied(SeatId),
    SeatNotEligible(SeatId),
    OutOfTurn { expected: SeatId, actual: SeatId },
    IllegalAction(ActionError),
}

impl fmt::Display for ActionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FoldNotAllowed => write!(formatter, "fold is only legal when facing a bet"),
            Self::CheckNotAllowed => write!(formatter, "check is not legal when facing a bet"),
            Self::CallNotAllowed => write!(formatter, "call is not legal in this state"),
            Self::InvalidCall { expected, actual } => {
                write!(formatter, "call must add {expected} chips, received {actual}")
            }
            Self::BetNotAllowed => write!(formatter, "bet is not legal when facing a bet"),
            Self::BetOutOfRange { min, max, actual } => write!(
                formatter,
                "bet-to amount must be between {min} and {max}, received {actual}"
            ),
            Self::RaiseNotAllowed => write!(formatter, "raise is not legal without a bet to face"),
            Self::RaiseNotReopened => write!(
                formatter,
                "raising is not reopened for this seat by the latest wager increase"
            ),
            Self::RaiseOutOfRange { min, max, actual } => write!(
                formatter,
                "raise-to amount must be between {min} and {max}, received {actual}"
            ),
            Self::InvalidAllIn { expected, actual } => write!(
                formatter,
                "all-in amount must be the actor's total street commitment {expected}, received {actual}"
            ),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HandNotActive => write!(formatter, "the hand is not accepting actions"),
            Self::SeatNotOccupied(seat) => {
                write!(formatter, "seat {} is not occupied", seat.as_u8())
            }
            Self::SeatNotEligible(seat) => {
                write!(formatter, "seat {} is not eligible to act", seat.as_u8())
            }
            Self::OutOfTurn { expected, actual } => write!(
                formatter,
                "seat {} acted out of turn; seat {} is expected",
                actual.as_u8(),
                expected.as_u8()
            ),
            Self::IllegalAction(error) => write!(formatter, "illegal action: {error}"),
        }
    }
}

impl std::error::Error for ActionError {}
impl std::error::Error for CommandError {}
