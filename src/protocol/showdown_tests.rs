use super::*;

fn s(n: u8) -> SeatId {
    SeatId::new(n).unwrap()
}
fn authority() -> ProtocolAuthority {
    ProtocolAuthority::new_paced(
        TableId(17),
        HandId(1),
        MultiwayHand::new_seeded_for_review(
            TableSize::new(3).unwrap(),
            s(0),
            &[(s(0), 100), (s(1), 100), (s(2), 100)],
            31_415,
        )
        .unwrap(),
    )
}

#[test]
fn ordered_public_reveals_never_publish_mucked_cards_or_future_awards() {
    let mut a = authority();
    let mut events = Vec::new();
    while let Some(actor) = a.hand().to_act {
        let legal = a.hand().legal_actions_for(actor).unwrap();
        let command = CommandEnvelope::act_for_hand(
            format!("step-{}", a.revision()),
            TableId(17),
            HandId(1),
            a.revision(),
            actor,
            crate::network_client::passive_action(&legal),
        );
        events.push(a.submit(command).unwrap());
    }
    let first = a.snapshot(ProjectionAudience::Spectator).unwrap();
    assert_eq!(first.snapshot.shown, [s(1)]);
    assert_eq!(
        first
            .snapshot
            .seats
            .iter()
            .filter(|s| s.hole_cards.is_some())
            .count(),
        1
    );
    assert!(first.snapshot.awards.is_empty());
    assert_eq!(
        first
            .snapshot
            .seats
            .iter()
            .map(|s| s.stack + s.hand_contribution)
            .sum::<u32>(),
        300
    );
    let late = CommandEnvelope::act_for_hand(
        "late",
        TableId(17),
        HandId(1),
        a.revision(),
        s(0),
        Action::Check,
    );
    assert!(a.submit(late).is_err());
    assert_eq!(a.snapshot(ProjectionAudience::Spectator).unwrap(), first);
    while let Some(event) = a.advance_showdown() {
        events.push(event);
    }
    let terminal = a.snapshot(ProjectionAudience::Spectator).unwrap();
    assert_eq!(terminal.snapshot.mucked, [s(2), s(0)]);
    assert_eq!(terminal.snapshot.shown, [s(1)]);
    assert_eq!(
        terminal
            .snapshot
            .seats
            .iter()
            .filter(|s| s.hole_cards.is_some())
            .count(),
        1
    );
    let history =
        crate::ring_history::SafeRingHandHistory::from_public_terminal(&terminal, &events).unwrap();
    assert_eq!(history.publicly_revealed.len(), 1);
    assert_eq!(history.publicly_revealed[0].seat, s(1));
    let own = a.snapshot(ProjectionAudience::Player(s(2))).unwrap();
    assert!(own
        .snapshot
        .seats
        .iter()
        .find(|s| s.seat == super::showdown_tests::s(2))
        .unwrap()
        .hole_cards
        .is_some());
    assert_eq!(
        terminal.snapshot.seats.iter().map(|s| s.stack).sum::<u32>(),
        300
    );
}

#[test]
fn preference_is_idempotent_stale_safe_private_and_old_clients_are_rejected() {
    let mut a = authority();
    let pref = CommandEnvelope {
        version: PROTOCOL_VERSION,
        command_id: "show-my-hand".into(),
        table_id: TableId(17),
        hand_id: HandId(1),
        expected_revision: 0,
        payload: CommandPayload::ShowdownPreference {
            seat: s(2),
            always_show: true,
        },
    };
    let receipt = a.submit_with_acknowledgement(pref.clone());
    assert_eq!(
        receipt.acknowledgement.result,
        AcknowledgementResult::Accepted
    );
    assert_eq!(
        a.submit_with_acknowledgement(pref.clone())
            .acknowledgement
            .delivery,
        AcknowledgementDelivery::Replayed
    );
    assert_eq!(a.revision(), 1);
    assert!(
        a.snapshot(ProjectionAudience::Player(s(2)))
            .unwrap()
            .snapshot
            .always_show
    );
    assert!(
        !a.snapshot(ProjectionAudience::Player(s(0)))
            .unwrap()
            .snapshot
            .always_show
    );
    let before = a.snapshot(ProjectionAudience::Spectator).unwrap();
    let mut stale = pref.clone();
    stale.command_id = "stale".into();
    assert!(a.submit(stale).is_err());
    let mut old = pref;
    old.version = PROTOCOL_VERSION - 1;
    old.command_id = "old-client".into();
    old.expected_revision = 1;
    assert_eq!(
        a.submit(old).unwrap_err().error.code,
        ProtocolErrorCode::UnsupportedVersion
    );
    assert_eq!(a.snapshot(ProjectionAudience::Spectator).unwrap(), before);
}

#[test]
fn preference_cannot_impersonate_another_session_or_extend_betting_deadline() {
    use crate::authorized_table::{AuthorizedTableRuntime, GuestSessionId, SessionRole};
    let runtime = AuthorizedTableRuntime::spawn(authority()).unwrap();
    let handle = runtime.handle();
    let session = GuestSessionId::new("pref-owner").unwrap();
    handle
        .bind(
            session.clone(),
            TableId(17),
            HandId(1),
            SessionRole::Player { seat: s(0) },
        )
        .unwrap();
    handle.advance_time(20).unwrap();
    let pref = CommandEnvelope {
        version: PROTOCOL_VERSION,
        command_id: "pref".into(),
        table_id: TableId(17),
        hand_id: HandId(1),
        expected_revision: 0,
        payload: CommandPayload::ShowdownPreference {
            seat: s(2),
            always_show: true,
        },
    };
    assert!(handle.submit(session.clone(), pref.clone()).is_err());
    let mut own = pref;
    own.payload = CommandPayload::ShowdownPreference {
        seat: s(0),
        always_show: true,
    };
    let response = handle.submit(session, own).unwrap();
    assert_eq!(response.deadline.unwrap().due_tick, 60);
    runtime.shutdown().unwrap();
}

#[test]
fn all_in_runout_finishes_even_when_every_player_disconnects() {
    use crate::authorized_table::{AuthorizedTableRuntime, GuestSessionId, SessionRole};
    use std::time::{Duration, Instant};
    let mut a = authority();
    while let Some(actor) = a.hand().to_act {
        let all_in = a.hand().legal_actions_for(actor).unwrap().all_in_to;
        a.submit(CommandEnvelope::act_for_hand(
            format!("all-in-{}", a.revision()),
            TableId(17),
            HandId(1),
            a.revision(),
            actor,
            Action::AllIn(all_in),
        ))
        .unwrap();
    }
    assert!(a.hand().board.is_empty());
    let runtime = AuthorizedTableRuntime::spawn(a).unwrap();
    let handle = runtime.handle();
    for n in 0..3 {
        let guest = GuestSessionId::new(format!("offline-{n}")).unwrap();
        handle
            .bind(
                guest.clone(),
                TableId(17),
                HandId(1),
                SessionRole::Player { seat: s(n) },
            )
            .unwrap();
        handle.disconnect(guest).unwrap();
    }
    let started = Instant::now();
    loop {
        let (snapshot, _) = handle.safe_history_material().unwrap();
        if snapshot.snapshot.phase == MultiwayPhase::Showdown {
            assert_eq!(snapshot.snapshot.board.len(), 5);
            assert_eq!(snapshot.snapshot.shown.len(), 3);
            assert_eq!(
                snapshot.snapshot.seats.iter().map(|s| s.stack).sum::<u32>(),
                300
            );
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(9));
        std::thread::sleep(Duration::from_millis(40));
    }
    runtime.shutdown().unwrap();
}

#[test]
fn auto_muck_settles_after_reveal_without_a_human_decision_or_extra_window() {
    use crate::authorized_table::{AuthorizedTableRuntime, GuestSessionId, SessionRole};
    use std::time::{Duration, Instant};
    let mut a = authority();
    while let Some(actor) = a.hand().to_act {
        let legal = a.hand().legal_actions_for(actor).unwrap();
        a.submit(CommandEnvelope::act_for_hand(
            format!("check-{}", a.revision()),
            TableId(17),
            HandId(1),
            a.revision(),
            actor,
            crate::network_client::passive_action(&legal),
        ))
        .unwrap();
    }
    let runtime = AuthorizedTableRuntime::spawn(a).unwrap();
    let handle = runtime.handle();
    let session = GuestSessionId::new("auto-muck-human").unwrap();
    handle
        .bind(
            session.clone(),
            TableId(17),
            HandId(1),
            SessionRole::Player { seat: s(0) },
        )
        .unwrap();
    let mut client = crate::network_client::ProjectionClient::bootstrap_from_update(
        handle.subscribe(session.clone()).unwrap().recv().unwrap(),
    )
    .unwrap();
    assert!(client
        .prepare_showdown_preference("too-late", true)
        .is_err());
    let initial = handle.snapshot(session.clone()).unwrap();
    let late = CommandEnvelope {
        version: PROTOCOL_VERSION,
        command_id: "late-show".into(),
        table_id: TableId(17),
        hand_id: HandId(1),
        expected_revision: initial.revision,
        payload: CommandPayload::ShowdownPreference {
            seat: s(0),
            always_show: true,
        },
    };
    assert_eq!(
        handle
            .submit(session.clone(), late)
            .unwrap()
            .receipt
            .acknowledgement
            .result,
        AcknowledgementResult::Rejected
    );
    let started = Instant::now();
    loop {
        let (public, _) = handle.safe_history_material().unwrap();
        if public.snapshot.showdown.is_none() {
            assert!(started.elapsed() >= Duration::from_millis(1_300));
            assert!(
                started.elapsed() < Duration::from_secs(4),
                "auto-muck must not open the old five-second window"
            );
            assert_eq!(public.snapshot.mucked, [s(2), s(0)]);
            assert_eq!(public.snapshot.shown, [s(1)]);
            assert!(!public.snapshot.awards.is_empty());
            for seat in public
                .snapshot
                .seats
                .iter()
                .filter(|seat| seat.seat != s(1))
            {
                assert!(seat.hole_cards.is_none());
            }
            break;
        }
        assert!(public.snapshot.awards.is_empty());
        assert!(started.elapsed() < Duration::from_secs(4));
        std::thread::sleep(Duration::from_millis(20));
    }
    runtime.shutdown().unwrap();
}
