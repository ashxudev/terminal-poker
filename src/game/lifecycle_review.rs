//! Deterministic Sprint 9 lifecycle trajectory around one real multiway hand.

use serde::Serialize;

use super::actions::Action;
use super::command::SeatCommand;
use super::lifecycle::{final_stacks, TableLifecycle};
use super::multiway::{MultiwayHand, MultiwayPhase};
use super::seat::{PlayerId, SeatId, TableSize};

pub const REVIEW_SEED: u64 = 9031;
pub const BUILD_ID: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "-v",
    env!("CARGO_PKG_VERSION"),
    "-sprint9-lifecycle-v1"
);
pub const HAND_ID: &str = "lifecycle-hand-0001";

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleSeatEvidence {
    pub seat: u8,
    pub player: u64,
    pub stack: u32,
    pub connection: String,
    pub table_participation: String,
    pub hand_participation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleReviewFrame {
    pub sequence: u8,
    pub screenshot_stem: String,
    pub event: String,
    pub phase: String,
    pub table_state: String,
    pub hand_active: bool,
    pub occupied: usize,
    pub eligible: usize,
    pub reservations: usize,
    pub pending: usize,
    pub active_hand_seats: Vec<u8>,
    pub lifecycle_seats: Vec<LifecycleSeatEvidence>,
    pub total_chips: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct LifecycleReviewEvidence {
    pub build_id: String,
    pub hand_id: String,
    pub seed: u64,
    pub trajectory_rule: String,
    pub frames: Vec<LifecycleReviewFrame>,
}

#[derive(Debug, Clone)]
pub struct LifecycleReviewCheckpoint {
    pub screenshot_stem: String,
    pub event: String,
    pub boundary: String,
    pub hand: MultiwayHand,
    pub lifecycle: TableLifecycle,
    pub action_log: Vec<String>,
}

pub fn build_lifecycle_review() -> (LifecycleReviewEvidence, Vec<LifecycleReviewCheckpoint>) {
    let mut lifecycle = TableLifecycle::new(TableSize::new(6).unwrap());
    for index in 0..3 {
        lifecycle
            .join(player(u64::from(index) + 1), seat(index), 100)
            .unwrap();
    }
    let start = lifecycle.begin_hand().unwrap();
    let mut hand = start
        .into_hand(lifecycle.table_size(), Some(REVIEW_SEED))
        .unwrap();
    let mut log = vec![
        "JOIN    P1->S0, P2->S1, P3->S2 / reservations consumed".to_string(),
        "START   3 eligible / immutable hand roster S0,S1,S2".to_string(),
    ];
    let mut checkpoints = vec![checkpoint(
        "01-running",
        "Table starts only after three eligible occupants are snapshotted",
        "WAITING -> RUNNING / hand 1 active",
        &hand,
        &lifecycle,
        &log,
    )];

    play_passive_action(&mut hand, &mut log);
    lifecycle.request_sit_out(player(2)).unwrap();
    lifecycle.request_leave(player(3)).unwrap();
    lifecycle.reserve(player(4), seat(3)).unwrap();
    log.push("QUEUE   P2 sit out + P3 leave / apply at boundary".to_string());
    log.push("RESERVE P4->S3 / claim only, not a hand participant".to_string());
    checkpoints.push(checkpoint(
        "02-deferred",
        "Mid-hand sit-out and leave are queued; reservation cannot enter the hand",
        "ACTIVE ROSTER UNCHANGED / 2 pending",
        &hand,
        &lifecycle,
        &log,
    ));

    play_until_phase(&mut hand, MultiwayPhase::Flop, &mut log);
    checkpoints.push(checkpoint(
        "03-flop",
        "The same three-seat hand continues despite pending lifecycle changes",
        "HAND CONTINUES / deferred requests isolated",
        &hand,
        &lifecycle,
        &log,
    ));
    play_until_phase(&mut hand, MultiwayPhase::River, &mut log);
    checkpoints.push(checkpoint(
        "04-river",
        "The active actor, cards, pot, and chip total remain hand-owned",
        "HAND CONTINUES / no roster mutation",
        &hand,
        &lifecycle,
        &log,
    ));
    play_to_terminal(&mut hand, &mut log);
    log.push("SHOWDOWN hand settles before roster transitions".to_string());
    checkpoints.push(checkpoint(
        "05-showdown",
        "Hand one reaches authoritative showdown with all original seats",
        "TERMINAL HAND / boundary not yet applied",
        &hand,
        &lifecycle,
        &log,
    ));

    lifecycle.complete_hand(&final_stacks(&hand)).unwrap();
    log.push("BOUNDARY P2=SITTING_OUT; P3 removed; pending cleared".to_string());
    log.push("PAUSE   only P1 remains eligible".to_string());
    checkpoints.push(checkpoint(
        "06-paused",
        "Pending transitions apply exactly once and table pauses below two eligible players",
        "RUNNING -> PAUSED / boundary applied once",
        &hand,
        &lifecycle,
        &log,
    ));

    lifecycle.occupy(player(4), seat(3), 100).unwrap();
    lifecycle.request_return(player(2)).unwrap();
    log.push("OCCUPY  P4 claims reserved S3 between hands".to_string());
    log.push("RETURN  P2 becomes active between hands".to_string());
    log.push("RESUME  3 eligible occupants; next hand may start".to_string());
    checkpoints.push(checkpoint(
        "07-resumed",
        "A reserved join plus return restores eligibility and resumes the table",
        "PAUSED -> RUNNING / between-hand roster",
        &hand,
        &lifecycle,
        &log,
    ));

    lifecycle.close().unwrap();
    log.push("CLOSE   explicit command accepted with no active hand".to_string());
    checkpoints.push(checkpoint(
        "08-closed",
        "Explicit close succeeds only after the hand boundary",
        "RUNNING -> CLOSED / terminal lifecycle",
        &hand,
        &lifecycle,
        &log,
    ));

    let frames = checkpoints
        .iter()
        .enumerate()
        .map(|(index, checkpoint)| evidence_frame(index as u8 + 1, checkpoint))
        .collect();
    (
        LifecycleReviewEvidence {
            build_id: BUILD_ID.to_string(),
            hand_id: HAND_ID.to_string(),
            seed: REVIEW_SEED,
            trajectory_rule: "Every frame follows lifecycle-hand-0001; post-hand frames retain its settled authority while the roster boundary advances".to_string(),
            frames,
        },
        checkpoints,
    )
}

fn play_until_phase(hand: &mut MultiwayHand, target: MultiwayPhase, log: &mut Vec<String>) {
    while hand.phase != target && !is_terminal(hand.phase) {
        play_passive_action(hand, log);
    }
}

fn play_to_terminal(hand: &mut MultiwayHand, log: &mut Vec<String>) {
    while !is_terminal(hand.phase) {
        play_passive_action(hand, log);
    }
}

fn play_passive_action(hand: &mut MultiwayHand, log: &mut Vec<String>) {
    let actor = hand.to_act.expect("active review hand has an actor");
    let legal = hand
        .legal_actions_for(actor)
        .expect("review actor has legal actions");
    let action = if legal.can_check {
        Action::Check
    } else if let Some(amount) = legal.call_amount {
        Action::Call(amount)
    } else {
        Action::AllIn(legal.all_in_to)
    };
    let phase = hand.phase;
    hand.apply_command(SeatCommand::new(actor, action)).unwrap();
    log.push(format!(
        "#{:02} {:<7} S{} {}",
        hand.action_history.len(),
        phase.name(),
        actor.as_u8(),
        action_text(action)
    ));
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

fn is_terminal(phase: MultiwayPhase) -> bool {
    matches!(phase, MultiwayPhase::Showdown | MultiwayPhase::HandComplete)
}

fn checkpoint(
    stem: &str,
    event: &str,
    boundary: &str,
    hand: &MultiwayHand,
    lifecycle: &TableLifecycle,
    log: &[String],
) -> LifecycleReviewCheckpoint {
    LifecycleReviewCheckpoint {
        screenshot_stem: stem.to_string(),
        event: event.to_string(),
        boundary: boundary.to_string(),
        hand: hand.clone(),
        lifecycle: lifecycle.clone(),
        action_log: log.to_vec(),
    }
}

fn evidence_frame(sequence: u8, checkpoint: &LifecycleReviewCheckpoint) -> LifecycleReviewFrame {
    let lifecycle = &checkpoint.lifecycle;
    LifecycleReviewFrame {
        sequence,
        screenshot_stem: checkpoint.screenshot_stem.clone(),
        event: checkpoint.event.clone(),
        phase: checkpoint.hand.phase.name().to_string(),
        table_state: format!("{:?}", lifecycle.state()),
        hand_active: lifecycle.hand_active(),
        occupied: lifecycle.seats().occupied_count(),
        eligible: lifecycle.eligible_count(),
        reservations: lifecycle.reservations().count(),
        pending: lifecycle.pending().count(),
        active_hand_seats: checkpoint
            .hand
            .occupied_seats()
            .map(SeatId::as_u8)
            .collect(),
        lifecycle_seats: lifecycle
            .seats()
            .occupied()
            .map(|(seat, state)| LifecycleSeatEvidence {
                seat: seat.as_u8(),
                player: state.player_id().value(),
                stack: state.stack,
                connection: format!("{:?}", state.connection),
                table_participation: format!("{:?}", state.table_participation),
                hand_participation: format!("{:?}", state.hand_participation),
            })
            .collect(),
        total_chips: checkpoint.hand.total_chips(),
    }
}

fn seat(index: u8) -> SeatId {
    SeatId::new(index).unwrap()
}

fn player(index: u64) -> PlayerId {
    PlayerId::new(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::table::TableParticipation;

    #[test]
    fn review_is_one_reproducible_hand_with_an_immutable_active_roster() {
        let (first, _) = build_lifecycle_review();
        let (second, _) = build_lifecycle_review();
        assert_eq!(
            serde_json::to_string(&first).unwrap(),
            serde_json::to_string(&second).unwrap()
        );
        assert_eq!(first.frames.len(), 8);
        assert!(first.frames.iter().all(|frame| frame.total_chips == 300));
        assert!(first.frames[..5]
            .iter()
            .all(|frame| frame.active_hand_seats == [0, 1, 2]));
        assert_eq!(first.frames[1].pending, 2);
        assert_eq!(first.frames[1].occupied, 3);
        assert_eq!(first.frames[5].table_state, "Paused");
        assert_eq!(first.frames[5].pending, 0);
        assert_eq!(first.frames[5].eligible, 1);
        assert_eq!(first.frames[6].table_state, "Running");
        assert_eq!(first.frames[6].eligible, 3);
        assert_eq!(first.frames[7].table_state, "Closed");
    }

    #[test]
    fn queued_sit_out_is_visible_as_table_state_only_after_boundary() {
        let (_, checkpoints) = build_lifecycle_review();
        let deferred = &checkpoints[1].lifecycle;
        assert_eq!(
            deferred.seats().seat(seat(1)).unwrap().table_participation,
            TableParticipation::Active
        );
        let paused = &checkpoints[5].lifecycle;
        assert_eq!(
            paused.seats().seat(seat(1)).unwrap().table_participation,
            TableParticipation::SittingOut
        );
    }
}
