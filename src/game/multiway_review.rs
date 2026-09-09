//! Deterministic four-handed Sprint 3 review trajectory.

use serde::Serialize;

use super::actions::Action;
use super::command::SeatCommand;
use super::deck::{Card, Suit};
use super::multiway::{Contribution, MultiwayHand, MultiwayPhase, PotAward};
use super::seat::{SeatId, TableSize};

pub const REVIEW_SEED: u64 = 13;
pub const BUILD_ID: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "-v",
    env!("CARGO_PKG_VERSION"),
    "-sprint3-review-v1"
);
pub const FIXTURE_ID: &str = "four-handed-three-pot-all-in";
pub const HAND_ID: &str = "fixture-hand-0001";

#[derive(Debug, Clone, Serialize)]
pub struct MultiwayReviewFrame {
    pub sequence: u8,
    pub screenshot_stem: String,
    pub phase: String,
    pub accepted_action_or_event: String,
    pub board: String,
    pub pot_total: u32,
    pub current_wager: u32,
    pub to_act: Option<u8>,
    pub stacks: Vec<u32>,
    pub contributions: Vec<u32>,
    pub pot_amounts: Vec<u32>,
    pub awards: Vec<PotAward>,
    pub total_chips: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiwayReviewEvidence {
    pub build_id: String,
    pub fixture_id: String,
    pub hand_id: String,
    pub seed: u64,
    pub local_seat: u8,
    pub frames: Vec<MultiwayReviewFrame>,
}

#[derive(Debug, Clone)]
pub struct MultiwayReviewCheckpoint {
    pub screenshot_stem: String,
    pub event: String,
    pub hand: MultiwayHand,
}

pub fn build_review_checkpoints() -> Vec<MultiwayReviewCheckpoint> {
    let mut hand = MultiwayHand::new_seeded_for_review(
        TableSize::new(4).expect("four-handed review size is valid"),
        seat(0),
        &[
            (seat(0), 40),
            (seat(1), 100),
            (seat(2), 200),
            (seat(3), 200),
        ],
        REVIEW_SEED,
    )
    .expect("review fixture configuration is valid");
    let mut checkpoints = vec![checkpoint(
        "01-blinds",
        "Blinds posted: S1 1, S2 2; S3 acts first",
        &hand,
    )];

    for (stem, target, event) in [
        (
            "02-full-raise",
            200,
            "S3 all-in to 200: full raise; every remaining seat owes a response",
        ),
        (
            "03-main-cap",
            40,
            "S0 all-in to 40: four-way main-pot contribution cap established",
        ),
        (
            "04-side-cap",
            100,
            "S1 all-in to 100: three-way first side-pot cap established",
        ),
        (
            "05-showdown",
            200,
            "S2 all-in to 200: board runs out and three pot layers resolve",
        ),
    ] {
        let actor = hand.to_act.expect("scripted review action has an actor");
        hand.apply_command(SeatCommand::new(actor, Action::AllIn(target)))
            .expect("scripted review all-in is legal");
        checkpoints.push(checkpoint(stem, event, &hand));
    }
    checkpoints
}

pub fn run_multiway_review_hand() -> MultiwayReviewEvidence {
    let frames = build_review_checkpoints()
        .iter()
        .enumerate()
        .map(|(index, checkpoint)| frame(index as u8 + 1, checkpoint))
        .collect();
    MultiwayReviewEvidence {
        build_id: BUILD_ID.to_string(),
        fixture_id: FIXTURE_ID.to_string(),
        hand_id: HAND_ID.to_string(),
        seed: REVIEW_SEED,
        local_seat: 0,
        frames,
    }
}

pub fn action_log_for_review(hand: &MultiwayHand, checkpoint_event: &str) -> Vec<String> {
    let mut log = vec!["BLINDS  S1 posts 1  |  S2 posts 2".to_string()];
    log.extend(hand.action_history.iter().map(|record| {
        format!(
            "#{:02} {:<7} S{} {}  | wager {}",
            record.sequence,
            record.phase.name(),
            record.seat.as_u8(),
            action_text(record.action),
            record.wager_after
        )
    }));
    if hand.phase == MultiwayPhase::Showdown {
        log.extend(hand.awards.iter().map(|award| {
            format!(
                "AWARD   {} {} -> {}",
                if award.pot_index == 0 {
                    "MAIN".to_string()
                } else {
                    format!("SIDE {}", award.pot_index)
                },
                award.amount,
                award
                    .payouts
                    .iter()
                    .map(|payout| format!("S{} +{}", payout.seat.as_u8(), payout.amount))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }));
    }
    log.push(format!("CHECKPOINT  {checkpoint_event}"));
    log
}

fn checkpoint(stem: &str, event: &str, hand: &MultiwayHand) -> MultiwayReviewCheckpoint {
    MultiwayReviewCheckpoint {
        screenshot_stem: stem.to_string(),
        event: event.to_string(),
        hand: hand.clone(),
    }
}

fn frame(sequence: u8, checkpoint: &MultiwayReviewCheckpoint) -> MultiwayReviewFrame {
    let hand = &checkpoint.hand;
    let settled = matches!(
        hand.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    );
    let contributions = if settled {
        amounts_from_contributions(&hand.settled_contributions)
    } else {
        (0..4)
            .map(|index| hand.seat(seat(index)).hand_contribution)
            .collect()
    };
    let pot_total = if settled {
        hand.pots.iter().map(|pot| pot.amount).sum()
    } else {
        contributions.iter().sum()
    };
    MultiwayReviewFrame {
        sequence,
        screenshot_stem: checkpoint.screenshot_stem.clone(),
        phase: hand.phase.name().to_string(),
        accepted_action_or_event: checkpoint.event.clone(),
        board: cards_text(&hand.board),
        pot_total,
        current_wager: hand.current_wager,
        to_act: hand.to_act.map(SeatId::as_u8),
        stacks: (0..4).map(|index| hand.seat(seat(index)).stack).collect(),
        contributions,
        pot_amounts: hand.pots.iter().map(|pot| pot.amount).collect(),
        awards: hand.awards.clone(),
        total_chips: hand.total_chips(),
    }
}

fn amounts_from_contributions(contributions: &[Contribution]) -> Vec<u32> {
    (0..4)
        .map(|index| {
            contributions
                .iter()
                .find(|entry| entry.seat == seat(index))
                .map_or(0, |entry| entry.amount)
        })
        .collect()
}

fn action_text(action: Action) -> String {
    match action {
        Action::Fold => "folds".to_string(),
        Action::Check => "checks".to_string(),
        Action::Call(amount) => format!("calls {amount}"),
        Action::Bet(amount) => format!("bets to {amount}"),
        Action::Raise(amount) => format!("raises to {amount}"),
        Action::AllIn(amount) => format!("all-in to {amount}"),
    }
}

fn cards_text(cards: &[Card]) -> String {
    if cards.is_empty() {
        return "-".to_string();
    }
    cards
        .iter()
        .map(|card| {
            let suit = match card.suit {
                Suit::Spades => 'S',
                Suit::Hearts => 'H',
                Suit::Diamonds => 'D',
                Suit::Clubs => 'C',
            };
            format!("{}{suit}", card.rank.symbol())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("review seat is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_trajectory_is_reproducible_and_reconciles() {
        let first = run_multiway_review_hand();
        let second = run_multiway_review_hand();
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
        assert_eq!(first.frames.len(), 5);
        assert!(first.frames.iter().all(|frame| frame.total_chips == 540));
        let final_frame = first.frames.last().unwrap();
        assert_eq!(final_frame.phase, "Showdown");
        assert_eq!(final_frame.pot_amounts, [160, 180, 200]);
        assert_eq!(final_frame.contributions, [40, 100, 200, 200]);
        assert_eq!(final_frame.awards[0].winners, [seat(1)]);
        assert_eq!(final_frame.awards[2].winners, [seat(2)]);
    }

    #[test]
    fn pre_showdown_log_does_not_expose_cards() {
        for checkpoint in &build_review_checkpoints()[..4] {
            let log = action_log_for_review(&checkpoint.hand, &checkpoint.event);
            assert!(log.iter().all(|line| !line.contains("Card")));
        }
    }
}
