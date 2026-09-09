//! Deterministic Sprint 4 protocol and privacy review trajectory.

use serde::Serialize;

use crate::game::actions::Action;
use crate::game::campaign::{run_seeded_campaign, CampaignConfig, CampaignReport};
use crate::game::multiway::{MultiwayHand, MultiwayPhase};
use crate::game::seat::{SeatId, TableSize};
use crate::protocol::{
    CommandEnvelope, HandId, ProjectionAudience, ProjectionKind, ProtocolAuthority,
    ProtocolErrorCode, SnapshotEnvelope, TableId, PROTOCOL_VERSION,
};

pub const REVIEW_SEED: u64 = 13;
pub const REVIEW_TABLE_ID: TableId = TableId(44);
pub const REVIEW_PROTOCOL_HAND_ID: HandId = HandId(1);
pub const BUILD_ID: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "-v",
    env!("CARGO_PKG_VERSION"),
    "-sprint4-review-v1"
);
pub const FIXTURE_ID: &str = "four-handed-versioned-private-projection";
pub const HAND_ID: &str = "fixture-hand-0001";

#[derive(Debug, Clone, Serialize)]
pub struct ProtocolReviewFrame {
    pub sequence: u8,
    pub screenshot_stem: String,
    pub phase: String,
    pub audience: String,
    pub revision: u64,
    pub command_id: String,
    pub outcome: String,
    pub accepted_action_or_event: String,
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
pub struct ProtocolReviewEvidence {
    pub build_id: String,
    pub fixture_id: String,
    pub hand_id: String,
    pub review_seed: u64,
    pub protocol_version: u16,
    pub table_id: u64,
    pub campaign: CampaignReport,
    pub frames: Vec<ProtocolReviewFrame>,
}

#[derive(Debug, Clone)]
pub struct ProtocolReviewCheckpoint {
    pub screenshot_stem: String,
    pub event: String,
    pub command_id: String,
    pub outcome: String,
    pub hand: MultiwayHand,
    pub snapshot: SnapshotEnvelope,
}

#[derive(Debug, Clone)]
pub struct ProtocolReviewRun {
    pub evidence: ProtocolReviewEvidence,
    pub checkpoints: Vec<ProtocolReviewCheckpoint>,
}

pub fn build_protocol_review() -> ProtocolReviewRun {
    let campaign = run_seeded_campaign(CampaignConfig::default())
        .expect("the accepted seeded invariant campaign passes");
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
    let mut authority = ProtocolAuthority::new(REVIEW_TABLE_ID, REVIEW_PROTOCOL_HAND_ID, hand);
    let mut checkpoints = Vec::new();

    checkpoints.push(checkpoint(
        "01-public-blinds",
        format!(
            "Public spectator snapshot; {} seeded cases passed across occupancies 2-9",
            campaign.cases.len()
        ),
        "-",
        "PUBLIC / CAMPAIGN PASS",
        &authority,
        ProjectionAudience::Spectator,
    ));
    checkpoints.push(checkpoint(
        "02-player-private",
        "Player S0 snapshot contains only S0 hole cards; remote hands are absent",
        "-",
        "PRIVATE PLAYER S0",
        &authority,
        ProjectionAudience::Player(seat(0)),
    ));

    authority
        .submit(CommandEnvelope::act(
            "cmd-0001",
            REVIEW_TABLE_ID,
            0,
            seat(3),
            Action::AllIn(200),
        ))
        .expect("first scripted protocol command is accepted");
    checkpoints.push(checkpoint(
        "03-command-accepted",
        "cmd-0001 accepted exactly once; revision advances 0 -> 1 and S0 acts next",
        "cmd-0001",
        "ACCEPTED",
        &authority,
        ProjectionAudience::Player(seat(0)),
    ));

    let before_rejection = authority
        .snapshot(ProjectionAudience::Player(seat(0)))
        .expect("player snapshot is valid");
    let rejection = authority
        .submit(CommandEnvelope::act(
            "cmd-stale",
            REVIEW_TABLE_ID,
            0,
            seat(0),
            Action::AllIn(40),
        ))
        .expect_err("stale expected revision is rejected");
    assert_eq!(rejection.error.code, ProtocolErrorCode::StaleRevision);
    assert_eq!(authority.revision(), 1);
    assert_eq!(
        before_rejection,
        authority
            .snapshot(ProjectionAudience::Player(seat(0)))
            .expect("player snapshot remains valid")
    );
    checkpoints.push(checkpoint(
        "04-stale-rejected",
        "cmd-stale rejected before poker mutation; authoritative revision remains 1",
        "cmd-stale",
        "STALE REJECTED / NO MUTATION",
        &authority,
        ProjectionAudience::Player(seat(0)),
    ));

    for (command_id, expected_revision, actor, target) in
        [("cmd-0002", 1, seat(0), 40), ("cmd-0003", 2, seat(1), 100)]
    {
        authority
            .submit(CommandEnvelope::act(
                command_id,
                REVIEW_TABLE_ID,
                expected_revision,
                actor,
                Action::AllIn(target),
            ))
            .expect("scripted side-cap command is accepted");
    }
    checkpoints.push(checkpoint(
        "05-private-side-caps",
        "cmd-0002 and cmd-0003 accepted; revision 3, S2 acts, remote cards stay absent",
        "cmd-0003",
        "ACCEPTED x2 / PRIVATE",
        &authority,
        ProjectionAudience::Player(seat(0)),
    ));

    authority
        .submit(CommandEnvelope::act(
            "cmd-0004",
            REVIEW_TABLE_ID,
            3,
            seat(2),
            Action::AllIn(200),
        ))
        .expect("terminal scripted command is accepted");
    checkpoints.push(checkpoint(
        "06-public-showdown",
        "cmd-0004 accepted; revision 4, public reveal set and three awards projected",
        "cmd-0004",
        "PUBLIC SHOWDOWN",
        &authority,
        ProjectionAudience::Spectator,
    ));

    let frames = checkpoints
        .iter()
        .enumerate()
        .map(|(index, checkpoint)| frame(index as u8 + 1, checkpoint))
        .collect();
    ProtocolReviewRun {
        evidence: ProtocolReviewEvidence {
            build_id: BUILD_ID.to_string(),
            fixture_id: FIXTURE_ID.to_string(),
            hand_id: HAND_ID.to_string(),
            review_seed: REVIEW_SEED,
            protocol_version: PROTOCOL_VERSION,
            table_id: REVIEW_TABLE_ID.0,
            campaign,
            frames,
        },
        checkpoints,
    }
}

pub fn protocol_action_log(
    checkpoint: &ProtocolReviewCheckpoint,
    campaign: &CampaignReport,
) -> Vec<String> {
    let mut log = vec![
        format!(
            "CAMPAIGN  PASS / {} cases / {} accepted actions / seed {}",
            campaign.cases.len(),
            campaign.accepted_actions,
            campaign.base_seed
        ),
        format!(
            "PROTOCOL  v{} / table {} / revision {} / view {}",
            PROTOCOL_VERSION,
            REVIEW_TABLE_ID.0,
            checkpoint.snapshot.revision,
            audience_label(&checkpoint.snapshot)
        ),
    ];
    log.extend(checkpoint.hand.action_history.iter().map(|record| {
        format!(
            "#{:02} REV {} S{} {} / wager {}",
            record.sequence,
            record.sequence,
            record.seat.as_u8(),
            action_text(record.action),
            record.wager_after
        )
    }));
    if checkpoint.hand.phase == MultiwayPhase::Showdown {
        log.extend(checkpoint.hand.awards.iter().map(|award| {
            format!(
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
            )
        }));
    }
    log.push(format!(
        "BOUNDARY  CMD {} / {}",
        checkpoint.command_id, checkpoint.outcome
    ));
    log.push(format!("CHECKPOINT  {}", checkpoint.event));
    log
}

fn checkpoint(
    stem: &str,
    event: impl Into<String>,
    command_id: &str,
    outcome: &str,
    authority: &ProtocolAuthority,
    audience: ProjectionAudience,
) -> ProtocolReviewCheckpoint {
    ProtocolReviewCheckpoint {
        screenshot_stem: stem.to_string(),
        event: event.into(),
        command_id: command_id.to_string(),
        outcome: outcome.to_string(),
        hand: authority.hand().clone(),
        snapshot: authority
            .snapshot(audience)
            .expect("scripted review audience occupies the table"),
    }
}

fn frame(sequence: u8, checkpoint: &ProtocolReviewCheckpoint) -> ProtocolReviewFrame {
    let snapshot = &checkpoint.snapshot.snapshot;
    ProtocolReviewFrame {
        sequence,
        screenshot_stem: checkpoint.screenshot_stem.clone(),
        phase: snapshot.phase.name().to_string(),
        audience: audience_label(&checkpoint.snapshot),
        revision: checkpoint.snapshot.revision,
        command_id: checkpoint.command_id.clone(),
        outcome: checkpoint.outcome.clone(),
        accepted_action_or_event: checkpoint.event.clone(),
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
        total_chips: checkpoint.hand.total_chips(),
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

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("review seat is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_is_reproducible_and_covers_campaign_privacy_and_revision() {
        let first = build_protocol_review();
        let second = build_protocol_review();
        assert_eq!(
            serde_json::to_string(&first.evidence).unwrap(),
            serde_json::to_string(&second.evidence).unwrap()
        );
        assert_eq!(first.evidence.campaign.cases.len(), 8 * 24);
        assert_eq!(first.evidence.frames.len(), 6);
        assert_eq!(
            first
                .evidence
                .frames
                .iter()
                .map(|frame| frame.revision)
                .collect::<Vec<_>>(),
            [0, 0, 1, 1, 3, 4]
        );
        assert_eq!(first.evidence.frames[0].visible_hands, 0);
        assert_eq!(first.evidence.frames[1].visible_hands, 1);
        assert_eq!(first.evidence.frames[5].visible_hands, 4);
        assert!(first
            .evidence
            .frames
            .iter()
            .all(|frame| frame.total_chips == 540));
        assert_eq!(first.evidence.frames[5].pot_amounts, [160, 180, 200]);
    }

    #[test]
    fn stale_checkpoint_has_identical_hand_values_to_prior_accepted_frame() {
        let review = build_protocol_review();
        let accepted = &review.evidence.frames[2];
        let stale = &review.evidence.frames[3];
        assert_eq!(accepted.revision, stale.revision);
        assert_eq!(accepted.stacks, stale.stacks);
        assert_eq!(accepted.contributions, stale.contributions);
        assert_eq!(accepted.pot_total, stale.pot_total);
        assert_eq!(accepted.current_wager, stale.current_wager);
        assert_eq!(accepted.to_act, stale.to_act);
    }
}
