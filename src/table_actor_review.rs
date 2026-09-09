//! Deterministic Sprint 5 retry-safe serialized-authority review trajectory.

use std::thread;

use serde::Serialize;

use crate::game::actions::Action;
use crate::game::multiway::{MultiwayHand, MultiwayPhase};
use crate::game::seat::{SeatId, TableSize};
use crate::protocol::{
    AcknowledgementDelivery, AcknowledgementResult, CommandEnvelope, CommandOutcome, HandId,
    ProjectionAudience, ProjectionKind, SnapshotEnvelope, TableEvent, TableId,
    MAX_COMMAND_ENVELOPE_BYTES, MAX_RECORDED_COMMANDS_PER_HAND, PROTOCOL_VERSION,
};
use crate::table_actor::{
    TableActor, TableActorMetrics, TableActorResponse, TABLE_MAILBOX_CAPACITY,
};

pub const REVIEW_SEED: u64 = 13;
pub const REVIEW_TABLE_ID: TableId = TableId(44);
pub const REVIEW_PROTOCOL_HAND_ID: HandId = HandId(1);
pub const RETRY_BATCH_SIZE: usize = 8;
pub const BUILD_ID: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "-v",
    env!("CARGO_PKG_VERSION"),
    "-sprint5-review-v1"
);
pub const FIXTURE_ID: &str = "four-handed-retry-safe-table-actor";
pub const HAND_ID: &str = "fixture-hand-0001";

#[derive(Debug, Clone, Serialize)]
pub struct TableActorReviewFrame {
    pub sequence: u8,
    pub screenshot_stem: String,
    pub phase: String,
    pub audience: String,
    pub revision: u64,
    pub command_id: String,
    pub acknowledgement_result: String,
    pub acknowledgement_delivery: String,
    pub outcome: String,
    pub accepted_action_or_event: String,
    pub actor_metrics: TableActorMetrics,
    pub board_cards: usize,
    pub visible_hands: usize,
    pub hidden_hands: usize,
    pub pot_total: u32,
    pub current_wager: u32,
    pub to_act: Option<u8>,
    pub stacks: Vec<u32>,
    pub contributions: Vec<u32>,
    pub pot_amounts: Vec<u32>,
    pub total_chips: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableActorReviewEvidence {
    pub build_id: String,
    pub fixture_id: String,
    pub hand_id: String,
    pub review_seed: u64,
    pub protocol_version: u16,
    pub table_id: u64,
    pub mailbox_capacity: usize,
    pub command_ledger_capacity: usize,
    pub max_command_bytes: usize,
    pub retry_batch_size: usize,
    pub frames: Vec<TableActorReviewFrame>,
}

#[derive(Debug, Clone)]
pub struct TableActorReviewCheckpoint {
    pub screenshot_stem: String,
    pub event: String,
    pub command_id: String,
    pub acknowledgement_result: String,
    pub acknowledgement_delivery: String,
    pub outcome: String,
    pub snapshot: SnapshotEnvelope,
    pub metrics: TableActorMetrics,
    pub action_log: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct TableActorReviewRun {
    pub evidence: TableActorReviewEvidence,
    pub checkpoints: Vec<TableActorReviewCheckpoint>,
}

pub fn build_table_actor_review() -> TableActorReviewRun {
    let hand = MultiwayHand::new_seeded_for_review(
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
    let actor = TableActor::spawn(crate::protocol::ProtocolAuthority::new(
        REVIEW_TABLE_ID,
        REVIEW_PROTOCOL_HAND_ID,
        hand,
    ))
    .expect("review table actor starts");
    let handle = actor.handle();
    let mut checkpoints = Vec::new();
    let mut log = vec![format!(
        "ACTOR  ONLINE / mailbox {} / ledger {} / max JSON {} bytes",
        TABLE_MAILBOX_CAPACITY, MAX_RECORDED_COMMANDS_PER_HAND, MAX_COMMAND_ENVELOPE_BYTES
    )];

    let public = handle
        .snapshot(ProjectionAudience::Spectator)
        .expect("public review snapshot is valid");
    checkpoints.push(checkpoint(
        "01-actor-public",
        "One bounded worker owns table 44; spectator receives public state only",
        "-",
        "-",
        "-",
        "ACTOR ONLINE / PUBLIC",
        public,
        handle.metrics().expect("actor metrics are available"),
        log.clone(),
    ));

    let private = handle
        .snapshot(ProjectionAudience::Player(seat(0)))
        .expect("player review snapshot is valid");
    log.push("PROJECTION  PLAYER S0 / own cards only / remote cards absent".to_string());
    checkpoints.push(checkpoint(
        "02-player-private",
        "Player S0 snapshot is constructed by the actor with only S0 private cards",
        "-",
        "-",
        "-",
        "PRIVATE PLAYER S0",
        private,
        handle.metrics().expect("actor metrics are available"),
        log.clone(),
    ));

    let command_one =
        CommandEnvelope::act("cmd-0001", REVIEW_TABLE_ID, 0, seat(3), Action::AllIn(200));
    let first = handle
        .submit(command_one.clone(), ProjectionAudience::Player(seat(0)))
        .expect("original command is accepted");
    log.push(event_log_line(&first));
    log.push("ACK  cmd-0001 / ACCEPTED / PROCESSED / revision 1".to_string());
    checkpoints.push(checkpoint_from_response(
        "03-original-accepted",
        "cmd-0001 processed once; S3 all-in 200 advances revision 0 -> 1",
        "ACCEPTED / PROCESSED",
        first,
        handle.metrics().expect("actor metrics are available"),
        log.clone(),
    ));

    let retry_workers = (0..RETRY_BATCH_SIZE)
        .map(|_| {
            let retry_handle = handle.clone();
            let retry_command = command_one.clone();
            thread::spawn(move || {
                retry_handle
                    .submit(retry_command, ProjectionAudience::Player(seat(0)))
                    .expect("concurrent exact retry receives a response")
            })
        })
        .collect::<Vec<_>>();
    let retry_responses = retry_workers
        .into_iter()
        .map(|worker| worker.join().expect("retry worker does not panic"))
        .collect::<Vec<_>>();
    assert!(retry_responses.iter().all(|response| {
        response.receipt.acknowledgement.delivery == AcknowledgementDelivery::Replayed
            && response.receipt.acknowledgement.result == AcknowledgementResult::Accepted
            && response.snapshot.revision == 1
    }));
    let after_retries = retry_responses
        .first()
        .expect("retry batch is non-empty")
        .snapshot
        .clone();
    log.push(format!(
        "RETRY  cmd-0001 x{} concurrent / original event replayed / no mutation",
        RETRY_BATCH_SIZE
    ));
    checkpoints.push(checkpoint(
        "04-concurrent-replays",
        format!(
            "{RETRY_BATCH_SIZE} concurrent exact retries replay cmd-0001; revision and state remain 1"
        ),
        "cmd-0001",
        "ACCEPTED",
        "REPLAYED",
        "REPLAYED x8 / NO MUTATION",
        after_retries,
        handle.metrics().expect("actor metrics are available"),
        log.clone(),
    ));

    let decode_error = handle
        .submit_json(b"{", ProjectionAudience::Player(seat(0)))
        .expect_err("malformed JSON is rejected before authority submission");
    let after_malformed = handle
        .snapshot(ProjectionAudience::Player(seat(0)))
        .expect("player snapshot remains valid after decode rejection");
    log.push(format!(
        "INGRESS  malformed JSON / {} / revision remains 1",
        decode_error
    ));
    checkpoints.push(checkpoint(
        "05-malformed-rejected",
        "Malformed JSON is bounded and rejected before command retention or poker mutation",
        "-",
        "REJECTED",
        "PROCESSED",
        "MALFORMED REJECTED / NO MUTATION",
        after_malformed,
        handle.metrics().expect("actor metrics are available"),
        log.clone(),
    ));

    for (command_id, expected_revision, actor_seat, target) in
        [("cmd-0002", 1, seat(0), 40), ("cmd-0003", 2, seat(1), 100)]
    {
        let response = handle
            .submit(
                CommandEnvelope::act(
                    command_id,
                    REVIEW_TABLE_ID,
                    expected_revision,
                    actor_seat,
                    Action::AllIn(target),
                ),
                ProjectionAudience::Player(seat(0)),
            )
            .expect("side-cap command receives an actor response");
        log.push(event_log_line(&response));
    }
    let private_caps = handle
        .snapshot(ProjectionAudience::Player(seat(0)))
        .expect("private side-cap snapshot is valid");
    log.push("ACK  cmd-0002 + cmd-0003 / serialized / revision 3".to_string());
    checkpoints.push(checkpoint(
        "06-serialized-side-caps",
        "cmd-0002 and cmd-0003 serialize through one worker; revision 3 and S2 acts",
        "cmd-0003",
        "ACCEPTED",
        "PROCESSED",
        "ACCEPTED x2 / SERIALIZED",
        private_caps,
        handle.metrics().expect("actor metrics are available"),
        log.clone(),
    ));

    let terminal = handle
        .submit(
            CommandEnvelope::act("cmd-0004", REVIEW_TABLE_ID, 3, seat(2), Action::AllIn(200)),
            ProjectionAudience::Spectator,
        )
        .expect("terminal command receives an actor response");
    log.push(event_log_line(&terminal));
    for award in &terminal.snapshot.snapshot.awards {
        log.push(format!(
            "AWARD  {} {} -> {}",
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
        ));
    }
    checkpoints.push(checkpoint_from_response(
        "07-public-showdown",
        "cmd-0004 completes revision 4; actor publishes four authorized hands and three awards",
        "PUBLIC SHOWDOWN",
        terminal,
        handle.metrics().expect("actor metrics are available"),
        log,
    ));

    actor.shutdown().expect("review actor shuts down cleanly");
    let frames = checkpoints
        .iter()
        .enumerate()
        .map(|(index, checkpoint)| frame(index as u8 + 1, checkpoint))
        .collect();
    TableActorReviewRun {
        evidence: TableActorReviewEvidence {
            build_id: BUILD_ID.to_string(),
            fixture_id: FIXTURE_ID.to_string(),
            hand_id: HAND_ID.to_string(),
            review_seed: REVIEW_SEED,
            protocol_version: PROTOCOL_VERSION,
            table_id: REVIEW_TABLE_ID.0,
            mailbox_capacity: TABLE_MAILBOX_CAPACITY,
            command_ledger_capacity: MAX_RECORDED_COMMANDS_PER_HAND,
            max_command_bytes: MAX_COMMAND_ENVELOPE_BYTES,
            retry_batch_size: RETRY_BATCH_SIZE,
            frames,
        },
        checkpoints,
    }
}

fn checkpoint_from_response(
    stem: &str,
    event: impl Into<String>,
    outcome: &str,
    response: TableActorResponse,
    metrics: TableActorMetrics,
    action_log: Vec<String>,
) -> TableActorReviewCheckpoint {
    checkpoint(
        stem,
        event,
        response
            .receipt
            .acknowledgement
            .command_id
            .as_deref()
            .unwrap_or("-"),
        acknowledgement_result(response.receipt.acknowledgement.result),
        acknowledgement_delivery(response.receipt.acknowledgement.delivery),
        outcome,
        response.snapshot,
        metrics,
        action_log,
    )
}

#[allow(clippy::too_many_arguments)]
fn checkpoint(
    stem: &str,
    event: impl Into<String>,
    command_id: &str,
    ack_result: &str,
    ack_delivery: &str,
    outcome: &str,
    snapshot: SnapshotEnvelope,
    metrics: TableActorMetrics,
    action_log: Vec<String>,
) -> TableActorReviewCheckpoint {
    TableActorReviewCheckpoint {
        screenshot_stem: stem.to_string(),
        event: event.into(),
        command_id: command_id.to_string(),
        acknowledgement_result: ack_result.to_string(),
        acknowledgement_delivery: ack_delivery.to_string(),
        outcome: outcome.to_string(),
        snapshot,
        metrics,
        action_log,
    }
}

fn frame(sequence: u8, checkpoint: &TableActorReviewCheckpoint) -> TableActorReviewFrame {
    let snapshot = &checkpoint.snapshot.snapshot;
    let total_chips = if matches!(
        snapshot.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        snapshot.seats.iter().map(|seat| seat.stack).sum()
    } else {
        snapshot.seats.iter().map(|seat| seat.stack).sum::<u32>() + snapshot.pot_total
    };
    TableActorReviewFrame {
        sequence,
        screenshot_stem: checkpoint.screenshot_stem.clone(),
        phase: snapshot.phase.name().to_string(),
        audience: audience_label(&checkpoint.snapshot),
        revision: checkpoint.snapshot.revision,
        command_id: checkpoint.command_id.clone(),
        acknowledgement_result: checkpoint.acknowledgement_result.clone(),
        acknowledgement_delivery: checkpoint.acknowledgement_delivery.clone(),
        outcome: checkpoint.outcome.clone(),
        accepted_action_or_event: checkpoint.event.clone(),
        actor_metrics: checkpoint.metrics,
        board_cards: snapshot.board.len(),
        visible_hands: snapshot
            .seats
            .iter()
            .filter(|seat| seat.hole_cards.is_some())
            .count(),
        hidden_hands: snapshot
            .seats
            .iter()
            .filter(|seat| seat.hole_cards.is_none())
            .count(),
        pot_total: snapshot.pot_total,
        current_wager: snapshot.current_wager,
        to_act: snapshot.to_act.map(SeatId::as_u8),
        stacks: snapshot.seats.iter().map(|seat| seat.stack).collect(),
        contributions: snapshot
            .seats
            .iter()
            .map(|seat| seat.hand_contribution)
            .collect(),
        pot_amounts: snapshot.pots.iter().map(|pot| pot.amount).collect(),
        total_chips,
    }
}

fn event_log_line(response: &TableActorResponse) -> String {
    match &response.receipt.outcome {
        CommandOutcome::Accepted { event } => match event.event {
            TableEvent::ShowdownAdvanced | TableEvent::ShowdownPreferenceAccepted { .. } => {
                format!("SHOWDOWN REV {}", event.revision)
            }
            TableEvent::ActionAccepted {
                seat,
                action,
                pot_total,
                ..
            } => format!(
                "EVENT  REV {} S{} {} / pot {}",
                event.revision,
                seat.as_u8(),
                action_text(action),
                pot_total
            ),
        },
        CommandOutcome::Rejected { error } => format!(
            "ERROR  REV {} / {}",
            error.revision,
            error.error.code.name()
        ),
    }
}

fn audience_label(snapshot: &SnapshotEnvelope) -> String {
    match snapshot.snapshot.audience {
        ProjectionKind::Player { seat } => format!("PLAYER S{}", seat.as_u8()),
        ProjectionKind::Spectator => "SPECTATOR".to_string(),
    }
}

fn acknowledgement_result(result: AcknowledgementResult) -> &'static str {
    match result {
        AcknowledgementResult::Accepted => "ACCEPTED",
        AcknowledgementResult::Rejected => "REJECTED",
    }
}

fn acknowledgement_delivery(delivery: AcknowledgementDelivery) -> &'static str {
    match delivery {
        AcknowledgementDelivery::Processed => "PROCESSED",
        AcknowledgementDelivery::Replayed => "REPLAYED",
    }
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

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("review seat is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_is_reproducible_and_covers_retry_decode_privacy_and_actor_metrics() {
        let first = build_table_actor_review();
        let second = build_table_actor_review();
        assert_eq!(
            serde_json::to_string(&first.evidence).unwrap(),
            serde_json::to_string(&second.evidence).unwrap()
        );
        assert_eq!(first.evidence.frames.len(), 7);
        assert_eq!(
            first
                .evidence
                .frames
                .iter()
                .map(|frame| frame.revision)
                .collect::<Vec<_>>(),
            [0, 0, 1, 1, 1, 3, 4]
        );
        assert_eq!(first.evidence.frames[0].visible_hands, 0);
        assert_eq!(first.evidence.frames[1].visible_hands, 1);
        assert_eq!(first.evidence.frames[6].visible_hands, 4);
        assert_eq!(first.evidence.frames[3].actor_metrics.accepted_commands, 1);
        assert_eq!(
            first.evidence.frames[3].actor_metrics.replayed_commands,
            RETRY_BATCH_SIZE as u64
        );
        assert_eq!(first.evidence.frames[4].actor_metrics.decode_rejections, 1);
        assert!(first
            .evidence
            .frames
            .iter()
            .all(|frame| frame.total_chips == 540));
        assert_eq!(first.evidence.frames[6].pot_amounts, [160, 180, 200]);
    }

    #[test]
    fn replay_and_malformed_checkpoints_preserve_the_original_accepted_state() {
        let review = build_table_actor_review();
        let accepted = &review.evidence.frames[2];
        for unchanged in &review.evidence.frames[3..=4] {
            assert_eq!(accepted.revision, unchanged.revision);
            assert_eq!(accepted.stacks, unchanged.stacks);
            assert_eq!(accepted.contributions, unchanged.contributions);
            assert_eq!(accepted.pot_total, unchanged.pot_total);
            assert_eq!(accepted.current_wager, unchanged.current_wager);
            assert_eq!(accepted.to_act, unchanged.to_act);
        }
    }
}
