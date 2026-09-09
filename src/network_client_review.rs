//! Production authorized-client trajectory reused for Sprint 12 beta evidence.

use serde::Serialize;

use crate::authorized_table::{
    AuthorizedTableHandle, AuthorizedTableMetrics, AuthorizedTableRuntime, GuestSessionId,
    SessionRole,
};
use crate::game::actions::Action;
use crate::game::multiway::{MultiwayHand, MultiwayPhase};
use crate::game::seat::{SeatId, TableSize};
use crate::network_client::{all_in_action, passive_action, ProjectionClient, UpdateDisposition};
use crate::protocol::{HandId, ProtocolAuthority, TableId, PROTOCOL_VERSION};

pub const REVIEW_SEED: u64 = 17;
pub const REVIEW_TABLE_ID: TableId = TableId(55);
pub const REVIEW_HAND_ID: HandId = HandId(1);
pub const BUILD_ID: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "-v",
    env!("CARGO_PKG_VERSION"),
    "-sprint12-private-beta-v1"
);
pub const FIXTURE_ID: &str = "nine-handed-projection-client-all-in-ladder";
pub const HAND_LABEL: &str = "fixture-hand-0001";
pub const REVIEW_WIDTH: u16 = 160;
pub const REVIEW_HEIGHT: u16 = 50;
pub const COMPACT_WIDTH: u16 = 120;
pub const COMPACT_HEIGHT: u16 = 40;

#[derive(Debug, Clone, Serialize)]
pub struct OccupancyCampaignResult {
    pub occupancy: u8,
    pub accepted_actions: u64,
    pub final_revision: u64,
    pub final_stream_sequence: u64,
    pub terminal_phase: String,
    pub initial_chips: u32,
    pub final_chips: u32,
    pub privacy_checks: u64,
    pub authorization_rejections: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientReviewFrame {
    pub sequence: u8,
    pub screenshot_stem: String,
    pub event: String,
    pub command_id: String,
    pub outcome: String,
    pub phase: String,
    pub revision: u64,
    pub stream_sequence: u64,
    pub connection: String,
    pub pending_command: Option<String>,
    pub to_act: Option<u8>,
    pub visible_hands: usize,
    pub hidden_hands: usize,
    pub pot_total: u32,
    pub stacks: Vec<u32>,
    pub contributions: Vec<u32>,
    pub pot_amounts: Vec<u32>,
    pub total_chips: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClientReviewEvidence {
    pub build_id: String,
    pub fixture_id: String,
    pub hand_id: String,
    pub review_seed: u64,
    pub protocol_version: u16,
    pub table_id: u64,
    pub protocol_hand_id: u64,
    pub review_viewport: [u16; 2],
    pub compact_viewport: [u16; 2],
    pub campaign: Vec<OccupancyCampaignResult>,
    pub frames: Vec<ClientReviewFrame>,
}

#[derive(Debug, Clone)]
pub struct ClientReviewCheckpoint {
    pub screenshot_stem: String,
    pub event: String,
    pub command_id: String,
    pub outcome: String,
    pub client: ProjectionClient,
    pub metrics: AuthorizedTableMetrics,
    pub action_log: Vec<String>,
    pub viewport: [u16; 2],
    pub trajectory: bool,
}

#[derive(Debug, Clone)]
pub struct ClientReviewRun {
    pub evidence: ClientReviewEvidence,
    pub checkpoints: Vec<ClientReviewCheckpoint>,
}

pub fn run_authorized_occupancy_campaign() -> Vec<OccupancyCampaignResult> {
    (2..=9).map(run_occupancy).collect()
}

fn run_occupancy(occupancy: u8) -> OccupancyCampaignResult {
    let table_id = TableId(1_000 + u64::from(occupancy));
    let hand_id = HandId(1);
    let stacks = (0..occupancy)
        .map(|index| (seat(index), 100))
        .collect::<Vec<_>>();
    let initial_chips = stacks.iter().map(|(_, stack)| stack).sum();
    let hand = MultiwayHand::new_seeded_for_review(
        TableSize::new(occupancy).expect("campaign occupancy is valid"),
        seat(0),
        &stacks,
        100 + u64::from(occupancy),
    )
    .expect("campaign hand is valid");
    let runtime = AuthorizedTableRuntime::spawn(ProtocolAuthority::new(table_id, hand_id, hand))
        .expect("campaign runtime starts");
    let handle = runtime.handle();
    for index in 0..occupancy {
        handle
            .bind(
                player_session(occupancy, index),
                table_id,
                hand_id,
                SessionRole::Player { seat: seat(index) },
            )
            .expect("campaign player binds");
    }
    handle
        .bind(
            spectator_session(occupancy),
            table_id,
            hand_id,
            SessionRole::Spectator,
        )
        .expect("campaign spectator binds");
    let spectator = handle
        .subscribe(spectator_session(occupancy))
        .expect("campaign spectator subscribes");
    let initial = spectator.recv().expect("initial campaign update");
    assert_eq!(visible_hands(&initial.snapshot), 0);
    let mut last_stream_sequence = initial.stream_sequence;

    let first_actor = initial
        .snapshot
        .snapshot
        .to_act
        .expect("active campaign has an actor");
    let wrong_seat = seat((first_actor.as_u8() + 1) % occupancy);
    let before_denied = handle
        .snapshot(spectator_session(occupancy))
        .expect("campaign public snapshot");
    let denied = crate::protocol::CommandEnvelope::act_for_hand(
        format!("campaign-{occupancy}-denied"),
        table_id,
        hand_id,
        before_denied.revision,
        wrong_seat,
        Action::Fold,
    );
    handle
        .submit(player_session(occupancy, first_actor.as_u8()), denied)
        .expect_err("cross-seat campaign command fails before authority");
    assert_eq!(
        handle
            .snapshot(spectator_session(occupancy))
            .expect("public snapshot after denial"),
        before_denied
    );

    let mut accepted_actions = 0u64;
    let mut privacy_checks = 0u64;
    loop {
        let public = handle
            .snapshot(spectator_session(occupancy))
            .expect("campaign public state");
        if matches!(
            public.snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            break;
        }
        let actor = public.snapshot.to_act.expect("non-terminal hand has actor");
        let private = handle
            .snapshot(player_session(occupancy, actor.as_u8()))
            .expect("actor receives private projection");
        assert_eq!(visible_hands(&private), 1, "occupancy {occupancy}");
        privacy_checks += 1;
        let mut client = ProjectionClient::bootstrap(private, last_stream_sequence)
            .expect("campaign client bootstraps");
        let action = passive_action(
            client
                .snapshot()
                .snapshot
                .legal_actions
                .as_ref()
                .expect("actor projection has legal actions"),
        );
        let command = client
            .prepare_action(format!("campaign-{occupancy}-{accepted_actions}"), action)
            .expect("campaign client prepares legal command");
        let response = handle
            .submit(player_session(occupancy, actor.as_u8()), command)
            .expect("campaign command reaches authority");
        client
            .apply_response(response)
            .expect("campaign client reconciles response");
        let update = spectator.recv().expect("campaign action is broadcast");
        assert!(update.stream_sequence > last_stream_sequence);
        assert_eq!(update.snapshot.revision, accepted_actions + 1);
        if !matches!(
            update.snapshot.snapshot.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            assert_eq!(visible_hands(&update.snapshot), 0);
        }
        last_stream_sequence = update.stream_sequence;
        accepted_actions += 1;
        assert!(accepted_actions < 100, "campaign hand must terminate");
    }

    let final_snapshot = handle
        .snapshot(spectator_session(occupancy))
        .expect("campaign terminal snapshot");
    let final_chips = final_snapshot
        .snapshot
        .seats
        .iter()
        .map(|projected| projected.stack)
        .sum();
    assert_eq!(final_chips, initial_chips, "occupancy {occupancy}");
    let metrics = handle.metrics().expect("campaign metrics");
    let result = OccupancyCampaignResult {
        occupancy,
        accepted_actions,
        final_revision: final_snapshot.revision,
        final_stream_sequence: metrics.stream_sequence,
        terminal_phase: final_snapshot.snapshot.phase.name().to_string(),
        initial_chips,
        final_chips,
        privacy_checks,
        authorization_rejections: metrics.authorization_rejections,
    };
    runtime.shutdown().expect("campaign runtime stops");
    result
}

pub fn build_network_client_review() -> ClientReviewRun {
    let campaign = run_authorized_occupancy_campaign();
    let stacks = [40, 70, 100, 130, 160, 190, 220, 250, 280]
        .into_iter()
        .enumerate()
        .map(|(index, stack)| (seat(index as u8), stack))
        .collect::<Vec<_>>();
    let initial_chips = stacks.iter().map(|(_, stack)| stack).sum::<u32>();
    let hand = MultiwayHand::new_seeded_for_review(
        TableSize::new(9).expect("nine-handed review size is valid"),
        seat(0),
        &stacks,
        REVIEW_SEED,
    )
    .expect("nine-handed review configuration is valid");
    let runtime = AuthorizedTableRuntime::spawn(ProtocolAuthority::new(
        REVIEW_TABLE_ID,
        REVIEW_HAND_ID,
        hand,
    ))
    .expect("Sprint 7 review runtime starts");
    let handle = runtime.handle();
    for index in 0..9 {
        handle
            .bind(
                review_session(index),
                REVIEW_TABLE_ID,
                REVIEW_HAND_ID,
                SessionRole::Player { seat: seat(index) },
            )
            .expect("review player binds");
    }
    let public = bind_review_spectator(&handle);
    let hero = public
        .snapshot
        .to_act
        .expect("review hand starts with an actor");
    let subscription = handle
        .subscribe(review_session(hero.as_u8()))
        .expect("hero subscribes");
    let initial = subscription.recv().expect("hero initial projection");
    let mut hero_client = ProjectionClient::bootstrap(initial.snapshot, initial.stream_sequence)
        .expect("hero client bootstraps");
    let mut log = vec![
        "CLIENT  projection-only authority / no internal hand or optimistic mutation".to_string(),
        "LAYOUT  responsive two-row nine-seat geometry / 160 x 50 review viewport".to_string(),
    ];
    let mut checkpoints = vec![checkpoint(
        "01-nine-seat-connected",
        "Nine-seat client connects from one authorized projection",
        "-",
        "CONNECTED / CONTROLS ENABLED",
        &hero_client,
        &handle,
        &log,
        [REVIEW_WIDTH, REVIEW_HEIGHT],
        true,
    )];

    let hero_action = all_in_action(
        hero_client
            .snapshot()
            .snapshot
            .legal_actions
            .as_ref()
            .expect("hero has legal actions"),
    );
    let hero_command = hero_client
        .prepare_action("client-0001", hero_action)
        .expect("hero intention is legal");
    checkpoints.push(checkpoint(
        "02-command-pending",
        "Local intention is pending while authoritative poker state remains unchanged",
        "client-0001",
        "PENDING / CONTROLS DISABLED",
        &hero_client,
        &handle,
        &log,
        [REVIEW_WIDTH, REVIEW_HEIGHT],
        true,
    ));
    handle
        .submit(review_session(hero.as_u8()), hero_command)
        .expect("hero command is accepted");
    let hero_update = subscription.recv().expect("hero acceptance update");
    assert_eq!(
        hero_client
            .apply_update(hero_update)
            .expect("hero reconciles"),
        UpdateDisposition::Applied
    );
    log.push(format!(
        "AUTHORITY  client-0001 accepted / hero S{} / revision 1",
        hero.as_u8()
    ));
    checkpoints.push(checkpoint(
        "03-authoritative-acceptance",
        "Authoritative output clears pending state and advances the projection",
        "client-0001",
        "ACCEPTED / REVISION 1",
        &hero_client,
        &handle,
        &log,
        [REVIEW_WIDTH, REVIEW_HEIGHT],
        true,
    ));

    let next_actor = current_actor(&handle);
    let next_snapshot = handle
        .snapshot(review_session(next_actor.as_u8()))
        .expect("next actor snapshot");
    let mut stale_client = ProjectionClient::bootstrap(
        next_snapshot,
        handle.metrics().expect("metrics").stream_sequence,
    )
    .expect("next client bootstraps");
    let stale_action = all_in_action(
        stale_client
            .snapshot()
            .snapshot
            .legal_actions
            .as_ref()
            .expect("next actor legal actions"),
    );
    let mut stale_command = stale_client
        .prepare_action("client-stale", stale_action)
        .expect("stale command starts valid");
    stale_command.expected_revision = 0;
    let stale_response = handle
        .submit(review_session(next_actor.as_u8()), stale_command)
        .expect("protocol returns a stable rejection response");
    stale_client
        .apply_response(stale_response)
        .expect("rejected response clears pending state");
    assert_eq!(stale_client.snapshot().revision, 1);
    log.push(format!(
        "REJECT  client-stale / S{} / stale revision / no mutation / no broadcast",
        next_actor.as_u8()
    ));
    checkpoints.push(checkpoint(
        "04-stale-rejected",
        "A stale client command is rejected without changing the hero projection",
        "client-stale",
        "REJECTED / NO MUTATION",
        &hero_client,
        &handle,
        &log,
        [REVIEW_WIDTH, REVIEW_HEIGHT],
        true,
    ));

    submit_current_all_in(&handle, 2);
    let lost = subscription
        .recv()
        .expect("one update is intentionally lost");
    log.push(format!(
        "TRANSPORT  simulated loss of stream {} / client still at revision {}",
        lost.stream_sequence,
        hero_client.snapshot().revision
    ));
    submit_current_all_in(&handle, 3);
    let after_gap = subscription.recv().expect("later update arrives");
    assert!(matches!(
        hero_client
            .apply_update(after_gap)
            .expect("gap handling is deterministic"),
        UpdateDisposition::ResynchronizationRequired { .. }
    ));
    checkpoints.push(checkpoint(
        "05-stream-gap",
        "A missed update disables controls and preserves the last good projection",
        "client-0003",
        "GAP / AWAITING RESYNC",
        &hero_client,
        &handle,
        &log,
        [REVIEW_WIDTH, REVIEW_HEIGHT],
        true,
    ));

    hero_client
        .resynchronize(
            handle
                .snapshot(review_session(hero.as_u8()))
                .expect("fresh hero snapshot"),
            handle.metrics().expect("metrics").stream_sequence,
        )
        .expect("fresh snapshot restores the client");
    log.push(
        "SYNC  snapshot restored current revision / controls derive from authority".to_string(),
    );
    checkpoints.push(checkpoint(
        "06-resynchronized",
        "A fresh authorized snapshot closes the gap without replaying local poker state",
        "-",
        "RESYNCHRONIZED",
        &hero_client,
        &handle,
        &log,
        [REVIEW_WIDTH, REVIEW_HEIGHT],
        true,
    ));

    let mut command_number = 4u64;
    while !terminal(&handle) {
        submit_current_all_in(&handle, command_number);
        let update = subscription
            .recv()
            .expect("hero receives ordered all-in update");
        assert_eq!(
            hero_client
                .apply_update(update)
                .expect("ordered update applies"),
            UpdateDisposition::Applied
        );
        command_number += 1;
        assert!(
            command_number < 20,
            "nine-handed all-in trajectory terminates"
        );
        if hero_client.snapshot().revision == 6 {
            log.push("POTS  contribution ladder established / side-pot caps pending".to_string());
            checkpoints.push(checkpoint(
                "07-all-in-ladder",
                "Six authoritative all-ins establish distinct contribution caps",
                "client-0006",
                "ORDERED / SIDE CAPS",
                &hero_client,
                &handle,
                &log,
                [REVIEW_WIDTH, REVIEW_HEIGHT],
                true,
            ));
        }
    }
    assert_eq!(chip_total(hero_client.snapshot()), initial_chips);
    assert!(hero_client.snapshot().snapshot.pots.len() >= 2);
    log.push(format!(
        "SHOWDOWN  {} pots awarded / {} chips conserved / all hands public",
        hero_client.snapshot().snapshot.pots.len(),
        initial_chips
    ));
    checkpoints.push(checkpoint(
        "08-public-showdown",
        "Nine-player all-in runout awards independent pots and reconciles every chip",
        &format!("client-{:04}", command_number - 1),
        "TERMINAL / PUBLIC AWARDS",
        &hero_client,
        &handle,
        &log,
        [REVIEW_WIDTH, REVIEW_HEIGHT],
        true,
    ));

    checkpoints.extend(layout_checkpoints());
    let frames = checkpoints
        .iter()
        .filter(|checkpoint| checkpoint.trajectory)
        .enumerate()
        .map(|(index, checkpoint)| frame((index + 1) as u8, checkpoint))
        .collect();
    let evidence = ClientReviewEvidence {
        build_id: BUILD_ID.to_string(),
        fixture_id: FIXTURE_ID.to_string(),
        hand_id: HAND_LABEL.to_string(),
        review_seed: REVIEW_SEED,
        protocol_version: PROTOCOL_VERSION,
        table_id: REVIEW_TABLE_ID.0,
        protocol_hand_id: REVIEW_HAND_ID.0,
        review_viewport: [REVIEW_WIDTH, REVIEW_HEIGHT],
        compact_viewport: [COMPACT_WIDTH, COMPACT_HEIGHT],
        campaign,
        frames,
    };
    runtime.shutdown().expect("Sprint 7 review runtime stops");
    ClientReviewRun {
        evidence,
        checkpoints,
    }
}

fn layout_checkpoints() -> Vec<ClientReviewCheckpoint> {
    [2u8, 4, 6, 9]
        .into_iter()
        .map(|occupancy| {
            let table_id = TableId(2_000 + u64::from(occupancy));
            let stacks = (0..occupancy)
                .map(|index| (seat(index), 100 + u32::from(index) * 10))
                .collect::<Vec<_>>();
            let hand = MultiwayHand::new_seeded_for_review(
                TableSize::new(occupancy).expect("layout occupancy is valid"),
                seat(0),
                &stacks,
                REVIEW_SEED + u64::from(occupancy),
            )
            .expect("layout hand is valid");
            let actor = hand.to_act.expect("layout has actor");
            let runtime = AuthorizedTableRuntime::spawn(ProtocolAuthority::new(
                table_id,
                REVIEW_HAND_ID,
                hand,
            ))
            .expect("layout runtime starts");
            let handle = runtime.handle();
            let session = GuestSessionId::new(format!("layout-{occupancy}-hero")).unwrap();
            handle
                .bind(
                    session.clone(),
                    table_id,
                    REVIEW_HAND_ID,
                    SessionRole::Player { seat: actor },
                )
                .expect("layout actor binds");
            let client = ProjectionClient::bootstrap(
                handle.snapshot(session).expect("layout projection"),
                0,
            )
            .expect("layout client bootstraps");
            let checkpoint = ClientReviewCheckpoint {
                screenshot_stem: format!("layout-{occupancy:02}-seats"),
                event: format!("Responsive compact layout with {occupancy} occupied seats"),
                command_id: "-".to_string(),
                outcome: "LAYOUT / AUTHORIZED PROJECTION".to_string(),
                client,
                metrics: handle.metrics().expect("layout metrics"),
                action_log: vec![format!(
                    "LAYOUT  {occupancy} seats / {} x {} / projection-fed",
                    COMPACT_WIDTH, COMPACT_HEIGHT
                )],
                viewport: [COMPACT_WIDTH, COMPACT_HEIGHT],
                trajectory: false,
            };
            runtime.shutdown().expect("layout runtime stops");
            checkpoint
        })
        .collect()
}

fn bind_review_spectator(handle: &AuthorizedTableHandle) -> crate::protocol::SnapshotEnvelope {
    let session = GuestSessionId::new("sprint7-review-spectator").unwrap();
    handle
        .bind(
            session.clone(),
            REVIEW_TABLE_ID,
            REVIEW_HAND_ID,
            SessionRole::Spectator,
        )
        .expect("review spectator binds");
    handle.snapshot(session).expect("review public snapshot")
}

fn current_actor(handle: &AuthorizedTableHandle) -> SeatId {
    let spectator = GuestSessionId::new("sprint7-review-spectator").unwrap();
    handle
        .snapshot(spectator)
        .expect("public review snapshot")
        .snapshot
        .to_act
        .expect("active review hand has an actor")
}

fn submit_current_all_in(handle: &AuthorizedTableHandle, number: u64) {
    let actor = current_actor(handle);
    let session = review_session(actor.as_u8());
    let snapshot = handle.snapshot(session.clone()).expect("actor snapshot");
    let mut client =
        ProjectionClient::bootstrap(snapshot, handle.metrics().expect("metrics").stream_sequence)
            .expect("actor client bootstraps");
    let action = all_in_action(
        client
            .snapshot()
            .snapshot
            .legal_actions
            .as_ref()
            .expect("actor has legal actions"),
    );
    let command = client
        .prepare_action(format!("client-{number:04}"), action)
        .expect("all-in intention is legal");
    let response = handle.submit(session, command).expect("all-in is accepted");
    client
        .apply_response(response)
        .expect("actor reconciles response");
}

fn terminal(handle: &AuthorizedTableHandle) -> bool {
    let spectator = GuestSessionId::new("sprint7-review-spectator").unwrap();
    matches!(
        handle
            .snapshot(spectator)
            .expect("review public state")
            .snapshot
            .phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    )
}

#[allow(clippy::too_many_arguments)]
fn checkpoint(
    stem: &str,
    event: &str,
    command_id: &str,
    outcome: &str,
    client: &ProjectionClient,
    handle: &AuthorizedTableHandle,
    log: &[String],
    viewport: [u16; 2],
    trajectory: bool,
) -> ClientReviewCheckpoint {
    ClientReviewCheckpoint {
        screenshot_stem: stem.to_string(),
        event: event.to_string(),
        command_id: command_id.to_string(),
        outcome: outcome.to_string(),
        client: client.clone(),
        metrics: handle.metrics().expect("review metrics"),
        action_log: log.to_vec(),
        viewport,
        trajectory,
    }
}

fn frame(sequence: u8, checkpoint: &ClientReviewCheckpoint) -> ClientReviewFrame {
    let snapshot = checkpoint.client.snapshot();
    ClientReviewFrame {
        sequence,
        screenshot_stem: checkpoint.screenshot_stem.clone(),
        event: checkpoint.event.clone(),
        command_id: checkpoint.command_id.clone(),
        outcome: checkpoint.outcome.clone(),
        phase: snapshot.snapshot.phase.name().to_string(),
        revision: snapshot.revision,
        stream_sequence: checkpoint.client.last_stream_sequence(),
        connection: checkpoint.client.connection().label().to_string(),
        pending_command: checkpoint
            .client
            .pending()
            .map(|pending| pending.command_id.clone()),
        to_act: snapshot.snapshot.to_act.map(SeatId::as_u8),
        visible_hands: visible_hands(snapshot),
        hidden_hands: snapshot
            .snapshot
            .seats
            .iter()
            .filter(|projected| projected.hole_cards.is_none())
            .count(),
        pot_total: snapshot.snapshot.pot_total,
        stacks: snapshot
            .snapshot
            .seats
            .iter()
            .map(|projected| projected.stack)
            .collect(),
        contributions: snapshot
            .snapshot
            .seats
            .iter()
            .map(|projected| projected.hand_contribution)
            .collect(),
        pot_amounts: snapshot
            .snapshot
            .pots
            .iter()
            .map(|pot| pot.amount)
            .collect(),
        total_chips: chip_total(snapshot),
    }
}

fn chip_total(snapshot: &crate::protocol::SnapshotEnvelope) -> u32 {
    let stacks = snapshot
        .snapshot
        .seats
        .iter()
        .map(|projected| projected.stack)
        .sum::<u32>();
    if matches!(
        snapshot.snapshot.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        stacks
    } else {
        stacks + snapshot.snapshot.pot_total
    }
}

fn visible_hands(snapshot: &crate::protocol::SnapshotEnvelope) -> usize {
    snapshot
        .snapshot
        .seats
        .iter()
        .filter(|projected| projected.hole_cards.is_some())
        .count()
}

fn seat(index: u8) -> SeatId {
    SeatId::new(index).expect("review seat is valid")
}

fn review_session(index: u8) -> GuestSessionId {
    GuestSessionId::new(format!("sprint7-player-{index}")).expect("review session is valid")
}

fn player_session(occupancy: u8, index: u8) -> GuestSessionId {
    GuestSessionId::new(format!("campaign-{occupancy}-player-{index}"))
        .expect("campaign session is valid")
}

fn spectator_session(occupancy: u8) -> GuestSessionId {
    GuestSessionId::new(format!("campaign-{occupancy}-spectator"))
        .expect("campaign spectator is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::network_client::ClientConnectionState;
    use crate::ui::multiway_review::MultiwayReviewView;
    use crate::ui::render;

    fn render_checkpoint(checkpoint: &ClientReviewCheckpoint) -> String {
        let view = MultiwayReviewView::from_network_client(
            &checkpoint.client,
            BUILD_ID,
            HAND_LABEL,
            REVIEW_SEED,
            &checkpoint.screenshot_stem,
            &checkpoint.command_id,
            &checkpoint.outcome,
            checkpoint.action_log.clone(),
        );
        let backend = TestBackend::new(checkpoint.viewport[0], checkpoint.viewport[1]);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render::render_network_view(frame, &view))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    #[test]
    fn authorized_campaign_completes_every_occupancy_with_privacy_and_conservation() {
        let campaign = run_authorized_occupancy_campaign();
        assert_eq!(campaign.len(), 8);
        for result in campaign {
            assert_eq!(result.initial_chips, result.final_chips);
            assert_eq!(result.final_revision, result.accepted_actions);
            assert!(result.privacy_checks > 0);
            assert_eq!(result.authorization_rejections, 1);
            assert_eq!(result.terminal_phase, "Showdown");
        }
    }

    #[test]
    fn review_proves_pending_rejection_gap_resync_side_pots_and_showdown() {
        let review = build_network_client_review();
        assert_eq!(review.evidence.campaign.len(), 8);
        assert_eq!(review.evidence.frames.len(), 8);
        assert_eq!(review.evidence.frames[1].revision, 0);
        assert_eq!(
            review.evidence.frames[1].pending_command.as_deref(),
            Some("client-0001")
        );
        assert_eq!(review.evidence.frames[3].revision, 1);
        assert_eq!(
            review.evidence.frames[4].connection,
            ClientConnectionState::AwaitingResynchronization.label()
        );
        let final_frame = review.evidence.frames.last().unwrap();
        assert_eq!(final_frame.phase, "Showdown");
        assert!(final_frame.pot_amounts.len() >= 2);
        assert_eq!(final_frame.visible_hands, 9);
        assert_eq!(final_frame.total_chips, 1_440);
    }

    #[test]
    fn production_renderer_supports_two_through_nine_seat_views() {
        let review = build_network_client_review();
        for checkpoint in review.checkpoints.iter().filter(|item| !item.trajectory) {
            let seat_count = checkpoint.client.snapshot().snapshot.table_size.get();
            let rendered = render_checkpoint(checkpoint);
            assert!(rendered.contains("SNEAKY BLINDERS"));
            assert!(rendered.contains(&format!("S{}", seat_count - 1)));
            assert!(!rendered.contains("NETWORK CLIENT"));
            assert!(!rendered.contains(BUILD_ID));
            assert!(!rendered.contains("seed"));
        }
    }

    #[test]
    fn production_renderer_makes_pending_and_resync_safety_visible() {
        let review = build_network_client_review();
        let pending = review
            .checkpoints
            .iter()
            .find(|item| item.screenshot_stem == "02-command-pending")
            .unwrap();
        let gap = review
            .checkpoints
            .iter()
            .find(|item| item.screenshot_stem == "05-stream-gap")
            .unwrap();

        assert!(render_checkpoint(pending).contains("AWAITING AUTHORITY"));
        let gap_render = render_checkpoint(gap);
        assert!(gap_render.contains("AWAITING RESYNC"));
        assert!(gap_render.contains("ACTIONS DISABLED"));
    }

    #[test]
    fn production_renderer_uses_only_projection_safe_identity_and_cards() {
        let review = build_network_client_review();
        let initial = review
            .checkpoints
            .iter()
            .find(|item| item.screenshot_stem == "01-nine-seat-connected")
            .unwrap();
        let view = MultiwayReviewView::from_network_client(
            &initial.client,
            BUILD_ID,
            HAND_LABEL,
            REVIEW_SEED,
            &initial.screenshot_stem,
            &initial.command_id,
            &initial.outcome,
            initial.action_log.clone(),
        );
        assert_eq!(
            view.seats.iter().filter(|seat| seat.cards_visible).count(),
            1
        );
        assert_eq!(
            view.seats.iter().filter(|seat| !seat.cards_visible).count(),
            8
        );

        let rendered = render_checkpoint(initial);
        assert!(rendered.contains("TABLE 55"));
        assert!(!rendered.contains("fixture-hand-0001"));
        assert!(!rendered.contains("client-"));
        assert!(!rendered.contains("STREAM"));
        assert!(!rendered.contains("REVISION"));
    }
}
