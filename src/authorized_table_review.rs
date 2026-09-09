//! Deterministic Sprint 6 authorization, deadline, and subscription trajectory.

use std::sync::mpsc::TryRecvError;

use serde::Serialize;

use crate::authorized_table::{
    ActionDeadline, AuthorizedTableMetrics, AuthorizedTableRuntime, GuestSessionId, SessionRole,
    SubscriptionReason, ACTION_TIMEOUT_TICKS, ACTION_WARNING_TICKS,
    AUTHORIZED_RUNTIME_MAILBOX_CAPACITY, SUBSCRIPTION_BUFFER_CAPACITY,
};
use crate::game::actions::Action;
use crate::game::multiway::{MultiwayHand, MultiwayPhase};
use crate::game::seat::{SeatId, TableSize};
use crate::protocol::{
    CommandEnvelope, CommandOutcome, HandId, ProjectionKind, SnapshotEnvelope, TableEvent, TableId,
    PROTOCOL_VERSION,
};

pub const REVIEW_SEED: u64 = 13;
pub const REVIEW_TABLE_ID: TableId = TableId(44);
pub const REVIEW_PROTOCOL_HAND_ID: HandId = HandId(1);
pub const BUILD_ID: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "-v",
    env!("CARGO_PKG_VERSION"),
    "-sprint6-review-v1"
);
pub const FIXTURE_ID: &str = "four-handed-authorized-timeout-subscriptions";
pub const HAND_ID: &str = "fixture-hand-0001";

#[derive(Debug, Clone, Serialize)]
pub struct AuthorizedReviewFrame {
    pub sequence: u8,
    pub screenshot_stem: String,
    pub phase: String,
    pub audience: String,
    pub revision: u64,
    pub stream_sequence: u64,
    pub now_tick: u64,
    pub deadline_seat: Option<u8>,
    pub deadline_due_tick: Option<u64>,
    pub command_id: String,
    pub outcome: String,
    pub actor_metrics: crate::table_actor::TableActorMetrics,
    pub authorization_rejections: u64,
    pub disconnects: u64,
    pub deadline_warnings: u64,
    pub timeout_actions: u64,
    pub subscription_deliveries: u64,
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
pub struct AuthorizedReviewEvidence {
    pub build_id: String,
    pub fixture_id: String,
    pub hand_id: String,
    pub review_seed: u64,
    pub protocol_version: u16,
    pub table_id: u64,
    pub protocol_hand_id: u64,
    pub runtime_mailbox_capacity: usize,
    pub subscription_buffer_capacity: usize,
    pub action_timeout_ticks: u64,
    pub action_warning_ticks: u64,
    pub frames: Vec<AuthorizedReviewFrame>,
}

#[derive(Debug, Clone)]
pub struct AuthorizedReviewCheckpoint {
    pub screenshot_stem: String,
    pub event: String,
    pub command_id: String,
    pub outcome: String,
    pub snapshot: SnapshotEnvelope,
    pub metrics: AuthorizedTableMetrics,
    pub deadline: Option<ActionDeadline>,
    pub action_log: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthorizedReviewRun {
    pub evidence: AuthorizedReviewEvidence,
    pub checkpoints: Vec<AuthorizedReviewCheckpoint>,
}

pub fn build_authorized_table_review() -> AuthorizedReviewRun {
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
    let runtime = AuthorizedTableRuntime::spawn(crate::protocol::ProtocolAuthority::new(
        REVIEW_TABLE_ID,
        REVIEW_PROTOCOL_HAND_ID,
        hand,
    ))
    .expect("authorized review runtime starts");
    let handle = runtime.handle();
    for index in 0..4 {
        handle
            .bind(
                player_session(index),
                REVIEW_TABLE_ID,
                REVIEW_PROTOCOL_HAND_ID,
                SessionRole::Player { seat: seat(index) },
            )
            .expect("review player binding is valid");
    }
    handle
        .bind(
            spectator_session(),
            REVIEW_TABLE_ID,
            REVIEW_PROTOCOL_HAND_ID,
            SessionRole::Spectator,
        )
        .expect("review spectator binding is valid");

    let spectator = handle
        .subscribe(spectator_session())
        .expect("spectator subscription starts");
    let spectator_initial = spectator.recv().expect("spectator initial update");
    let player = handle
        .subscribe(player_session(0))
        .expect("player subscription starts");
    let player_initial = player.recv().expect("player initial update");
    let mut log = vec![format!(
        "RUNTIME  AUTHORIZED / mailbox {} / subscription buffer {}",
        AUTHORIZED_RUNTIME_MAILBOX_CAPACITY, SUBSCRIPTION_BUFFER_CAPACITY
    )];
    log.push(format!(
        "CLOCK  authoritative {} ticks / warning with {} remaining",
        ACTION_TIMEOUT_TICKS, ACTION_WARNING_TICKS
    ));
    let mut checkpoints = vec![checkpoint(
        "01-spectator-subscribed",
        "Spectator subscription receives public revision 0 without private cards",
        "-",
        "SUBSCRIBED / PUBLIC",
        spectator_initial.snapshot,
        handle.metrics().expect("runtime metrics are available"),
        spectator_initial.deadline,
        log.clone(),
    )];

    log.push("BINDING  PLAYER S0 / table 44 / hand 1 / private audience derived".to_string());
    checkpoints.push(checkpoint(
        "02-player-bound-private",
        "Bound player S0 receives exactly one authorized private hand",
        "-",
        "BOUND / PRIVATE S0",
        player_initial.snapshot,
        handle.metrics().expect("runtime metrics are available"),
        player_initial.deadline,
        log.clone(),
    ));

    let denied = handle
        .submit(
            player_session(0),
            command("attack-cross-seat", 0, seat(3), Action::AllIn(200)),
        )
        .expect_err("S0 cannot act for S3");
    assert_eq!(denied.code.name(), "unauthorized_seat");
    assert!(matches!(spectator.try_recv(), Err(TryRecvError::Empty)));
    assert!(matches!(player.try_recv(), Err(TryRecvError::Empty)));
    log.push(
        "DENY  cross-seat intent / unauthorized_seat / no mutation / no broadcast".to_string(),
    );
    let after_denied = handle
        .snapshot(spectator_session())
        .expect("public state remains available");
    checkpoints.push(checkpoint(
        "03-cross-seat-denied",
        "S0 cross-seat command is denied before authority, retention, or fan-out",
        "attack-cross-seat",
        "UNAUTHORIZED / NO MUTATION",
        after_denied,
        handle.metrics().expect("runtime metrics are available"),
        player_initial.deadline,
        log.clone(),
    ));

    let accepted = handle
        .submit(
            player_session(3),
            command("cmd-0001", 0, seat(3), Action::AllIn(200)),
        )
        .expect("bound S3 command is accepted");
    let spectator_action = spectator.recv().expect("spectator action update");
    let player_action = player.recv().expect("player action update");
    assert_eq!(
        spectator_action.stream_sequence,
        player_action.stream_sequence
    );
    log.push(event_log_line(&accepted.receipt));
    log.push(format!(
        "STREAM  seq {} / S0 private + spectator public / ordered",
        player_action.stream_sequence
    ));
    checkpoints.push(checkpoint(
        "04-authorized-action",
        "Bound S3 action advances revision 1 and reaches two audience-correct streams",
        "cmd-0001",
        "AUTHORIZED / ACCEPTED",
        player_action.snapshot,
        handle.metrics().expect("runtime metrics are available"),
        player_action.deadline,
        log.clone(),
    ));

    handle
        .disconnect(player_session(0))
        .expect("S0 disconnect is recorded");
    let disconnected = spectator.recv().expect("spectator disconnect update");
    assert!(matches!(
        disconnected.reason,
        SubscriptionReason::ConnectionStateChanged {
            seat: Some(disconnected_seat),
            connected: false
        } if disconnected_seat == seat(0)
    ));
    assert!(matches!(player.try_recv(), Err(TryRecvError::Disconnected)));
    log.push(format!(
        "DISCONNECT  S0 / private stream closed / deadline still due tick {}",
        disconnected.deadline.expect("deadline remains").due_tick
    ));
    checkpoints.push(checkpoint(
        "05-disconnected-clock-live",
        "S0 disconnect closes its private stream while the authority clock continues",
        "-",
        "DISCONNECTED / CLOCK LIVE",
        disconnected.snapshot,
        handle.metrics().expect("runtime metrics are available"),
        disconnected.deadline,
        log.clone(),
    ));

    let warning_result = handle.advance_time(50).expect("warning tick is accepted");
    assert!(warning_result.warning_emitted);
    let warning = spectator.recv().expect("spectator warning update");
    log.push("DEADLINE  S0 / warning / 10 ticks remaining / revision 1".to_string());
    checkpoints.push(checkpoint(
        "06-deadline-warning",
        "Server tick 50 emits one S0 warning without poker mutation",
        "-",
        "WARNING / 10 TICKS",
        warning.snapshot,
        handle.metrics().expect("runtime metrics are available"),
        warning.deadline,
        log.clone(),
    ));

    let timeout_result = handle.advance_time(60).expect("timeout tick is accepted");
    assert_eq!(timeout_result.timeout_action, Some(Action::Fold));
    let timeout = spectator.recv().expect("spectator timeout update");
    log.push("TIMEOUT  S0 / fold selected because check is illegal / revision 2".to_string());
    checkpoints.push(checkpoint(
        "07-timeout-fold",
        "Server tick 60 folds S0 exactly once and schedules S1",
        timeout
            .event
            .as_ref()
            .map_or("-", |event| event.command_id.as_str()),
        "SERVER TIMEOUT / FOLD",
        timeout.snapshot,
        handle.metrics().expect("runtime metrics are available"),
        timeout.deadline,
        log.clone(),
    ));

    let first_cap = handle
        .submit(
            player_session(1),
            command("cmd-0002", 2, seat(1), Action::AllIn(100)),
        )
        .expect("S1 cap command is accepted");
    let first_cap_update = spectator.recv().expect("first cap update");
    assert_eq!(first_cap_update.snapshot.revision, 3);
    log.push(event_log_line(&first_cap.receipt));
    let terminal = handle
        .submit(
            player_session(2),
            command("cmd-0003", 3, seat(2), Action::AllIn(200)),
        )
        .expect("S2 terminal command is accepted");
    let terminal_update = spectator.recv().expect("terminal public update");
    log.push(event_log_line(&terminal.receipt));
    for award in &terminal_update.snapshot.snapshot.awards {
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
    checkpoints.push(checkpoint(
        "08-public-showdown",
        "Authorized S1/S2 caps complete revision 4; folded S0 stays hidden and awards reconcile",
        "cmd-0003",
        "PUBLIC SHOWDOWN / ORDERED",
        terminal_update.snapshot,
        handle.metrics().expect("runtime metrics are available"),
        terminal_update.deadline,
        log,
    ));

    runtime
        .shutdown()
        .expect("authorized review runtime shuts down cleanly");
    let frames = checkpoints
        .iter()
        .enumerate()
        .map(|(index, checkpoint)| frame(index as u8 + 1, checkpoint))
        .collect();
    AuthorizedReviewRun {
        evidence: AuthorizedReviewEvidence {
            build_id: BUILD_ID.to_string(),
            fixture_id: FIXTURE_ID.to_string(),
            hand_id: HAND_ID.to_string(),
            review_seed: REVIEW_SEED,
            protocol_version: PROTOCOL_VERSION,
            table_id: REVIEW_TABLE_ID.0,
            protocol_hand_id: REVIEW_PROTOCOL_HAND_ID.0,
            runtime_mailbox_capacity: AUTHORIZED_RUNTIME_MAILBOX_CAPACITY,
            subscription_buffer_capacity: SUBSCRIPTION_BUFFER_CAPACITY,
            action_timeout_ticks: ACTION_TIMEOUT_TICKS,
            action_warning_ticks: ACTION_WARNING_TICKS,
            frames,
        },
        checkpoints,
    }
}

#[allow(clippy::too_many_arguments)]
fn checkpoint(
    stem: &str,
    event: impl Into<String>,
    command_id: &str,
    outcome: &str,
    snapshot: SnapshotEnvelope,
    metrics: AuthorizedTableMetrics,
    deadline: Option<ActionDeadline>,
    action_log: Vec<String>,
) -> AuthorizedReviewCheckpoint {
    AuthorizedReviewCheckpoint {
        screenshot_stem: stem.to_string(),
        event: event.into(),
        command_id: command_id.to_string(),
        outcome: outcome.to_string(),
        snapshot,
        metrics,
        deadline,
        action_log,
    }
}

fn frame(sequence: u8, checkpoint: &AuthorizedReviewCheckpoint) -> AuthorizedReviewFrame {
    let snapshot = &checkpoint.snapshot.snapshot;
    let total_chips = if matches!(
        snapshot.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        snapshot.seats.iter().map(|seat| seat.stack).sum()
    } else {
        snapshot.seats.iter().map(|seat| seat.stack).sum::<u32>() + snapshot.pot_total
    };
    AuthorizedReviewFrame {
        sequence,
        screenshot_stem: checkpoint.screenshot_stem.clone(),
        phase: snapshot.phase.name().to_string(),
        audience: audience_label(&checkpoint.snapshot),
        revision: checkpoint.snapshot.revision,
        stream_sequence: checkpoint.metrics.stream_sequence,
        now_tick: checkpoint.metrics.now_tick,
        deadline_seat: checkpoint.deadline.map(|deadline| deadline.seat.as_u8()),
        deadline_due_tick: checkpoint.deadline.map(|deadline| deadline.due_tick),
        command_id: checkpoint.command_id.clone(),
        outcome: checkpoint.outcome.clone(),
        actor_metrics: checkpoint.metrics.actor,
        authorization_rejections: checkpoint.metrics.authorization_rejections,
        disconnects: checkpoint.metrics.disconnects,
        deadline_warnings: checkpoint.metrics.deadline_warnings,
        timeout_actions: checkpoint.metrics.timeout_actions,
        subscription_deliveries: checkpoint.metrics.subscription_deliveries,
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

fn event_log_line(receipt: &crate::protocol::SubmissionReceipt) -> String {
    match &receipt.outcome {
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

fn command(id: &str, revision: u64, player_seat: SeatId, action: Action) -> CommandEnvelope {
    CommandEnvelope::act_for_hand(
        id,
        REVIEW_TABLE_ID,
        REVIEW_PROTOCOL_HAND_ID,
        revision,
        player_seat,
        action,
    )
}

fn player_session(index: u8) -> GuestSessionId {
    GuestSessionId::new(format!("review-player-{index}")).expect("review session ID is valid")
}

fn spectator_session() -> GuestSessionId {
    GuestSessionId::new("review-spectator").expect("review session ID is valid")
}

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("review seat is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_is_reproducible_private_ordered_and_conservative() {
        let first = build_authorized_table_review();
        let second = build_authorized_table_review();
        assert_eq!(
            serde_json::to_string(&first.evidence).unwrap(),
            serde_json::to_string(&second.evidence).unwrap()
        );
        assert_eq!(first.evidence.frames.len(), 8);
        assert_eq!(
            first
                .evidence
                .frames
                .iter()
                .map(|frame| frame.revision)
                .collect::<Vec<_>>(),
            [0, 0, 0, 1, 1, 1, 2, 4]
        );
        assert_eq!(first.evidence.frames[0].visible_hands, 0);
        assert_eq!(first.evidence.frames[1].visible_hands, 1);
        assert_eq!(first.evidence.frames[7].visible_hands, 3);
        assert_eq!(first.evidence.frames[7].hidden_hands, 1);
        assert!(first
            .evidence
            .frames
            .iter()
            .all(|frame| frame.total_chips == 540));
        let final_frame = &first.evidence.frames[7];
        assert_eq!(final_frame.actor_metrics.accepted_commands, 4);
        assert_eq!(final_frame.authorization_rejections, 1);
        assert_eq!(final_frame.disconnects, 1);
        assert_eq!(final_frame.deadline_warnings, 1);
        assert_eq!(final_frame.timeout_actions, 1);
        assert_eq!(final_frame.subscription_deliveries, 9);
        assert_eq!(final_frame.stream_sequence, 6);
        assert_eq!(final_frame.pot_amounts, [300, 200]);
        assert_eq!(final_frame.stacks, [40, 300, 200, 0]);
    }

    #[test]
    fn denied_warning_and_disconnect_frames_do_not_mutate_the_hand() {
        let review = build_authorized_table_review();
        let baseline = &review.evidence.frames[0];
        let denied = &review.evidence.frames[2];
        assert_eq!(baseline.revision, denied.revision);
        assert_eq!(baseline.stacks, denied.stacks);
        assert_eq!(baseline.contributions, denied.contributions);
        let accepted = &review.evidence.frames[3];
        for unchanged in &review.evidence.frames[4..=5] {
            assert_eq!(accepted.revision, unchanged.revision);
            assert_eq!(accepted.stacks, unchanged.stacks);
            assert_eq!(accepted.contributions, unchanged.contributions);
            assert_eq!(accepted.pot_total, unchanged.pot_total);
        }
    }
}
