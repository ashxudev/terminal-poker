use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use terminal_poker::{
    game::actions::Action,
    network_client::{passive_action, ProjectionClient},
    network_server::{MultiTableNetworkServer, MultiTableNetworkServerConfig},
    network_session::{LobbySession, NetworkSession},
    tournament::{TournamentConfig, TournamentLevel, TournamentPayoutPlan},
    ui::network_app::NetworkApp,
};

#[test]
fn tcp_showdown_reconnect_preserves_private_mucks_and_all_in_runout_order() {
    for all_in in [false, true] {
        let shutdown = Arc::new(AtomicBool::new(false));
        let server = MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
            shutdown_requested: shutdown.clone(),
            deterministic_seed_base: Some(31_415),
            ..MultiTableNetworkServerConfig::default()
        })
        .unwrap();
        let address = server.listen_addr();
        let worker = thread::spawn(move || server.run().unwrap());
        let (mut lobby, _, _, _) = LobbySession::connect(address, "Host").unwrap();
        let invite = "sprint17-showdown-private-123456".to_string();
        let table = lobby
            .create_tournament(TournamentConfig {
                name: "Showdown verification".to_string(),
                entrants: 2,
                starting_stack: 100,
                levels: vec![TournamentLevel {
                    small_blind: 1,
                    big_blind: 2,
                    ante: 0,
                    duration_seconds: 60,
                    break_after_seconds: 0,
                }],
                payout: TournamentPayoutPlan {
                    pool: 100,
                    shares_bps: vec![10_000],
                },
                join_code: invite.clone(),
            })
            .unwrap();
        let id = table.table_id;
        let host_invite = invite.clone();
        let host = thread::spawn(move || {
            lobby
                .join_and_wait_with_access(id, None, Some(host_invite))
                .unwrap()
        });
        let (guest, _, _, _) = LobbySession::connect(address, "Guest").unwrap();
        let (guest_session, guest_update, _) = guest
            .join_and_wait_with_access(id, None, Some(invite))
            .unwrap();
        let (host_session, host_update, _) = host.join().unwrap();
        let mut sessions = vec![host_session, guest_session];
        let mut apps = vec![
            NetworkApp::new(
                ProjectionClient::bootstrap_from_update(host_update).unwrap(),
                "Host",
            ),
            NetworkApp::new(
                ProjectionClient::bootstrap_from_update(guest_update).unwrap(),
                "Guest",
            ),
        ];
        let started = Instant::now();
        let mut reveal_started = None;
        let mut board_lengths = Vec::new();
        let mut reconnected = false;
        while !apps.iter().all(NetworkApp::is_terminal) {
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "showdown must finish without another betting command"
            );
            for (session, app) in sessions.iter_mut().zip(&mut apps) {
                for message in session.poll().unwrap() {
                    app.apply_message(message).unwrap();
                }
                let snapshot = &app.client().snapshot().snapshot;
                if let Some(progress) = &snapshot.showdown {
                    reveal_started.get_or_insert_with(Instant::now);
                    assert!(snapshot.awards.is_empty());
                    assert!(snapshot.legal_actions.is_none());
                    assert!(snapshot.to_act.is_none());
                    if all_in {
                        assert!(progress.all_in);
                        assert_eq!(snapshot.shown.len(), 2);
                        assert!(snapshot
                            .seats
                            .iter()
                            .all(|s| s.hole_cards.as_ref().is_some_and(|cards| cards.len() == 2)));
                        if board_lengths
                            .last()
                            .is_none_or(|last| *last < snapshot.board.len())
                        {
                            board_lengths.push(snapshot.board.len());
                        }
                    } else {
                        assert!(!progress.all_in);
                        assert_eq!(progress.order.len(), 2);
                        for s in &snapshot.seats {
                            if snapshot.mucked.contains(&s.seat)
                                && !matches!(snapshot.audience, terminal_poker::protocol::ProjectionKind::Player {seat} if seat==s.seat)
                            {
                                assert!(
                                    s.hole_cards.is_none(),
                                    "mucked opponents stay private on the wire"
                                );
                            }
                        }
                    }
                }
                if app.client().controls_enabled() {
                    let legal = app
                        .client()
                        .snapshot()
                        .snapshot
                        .legal_actions
                        .as_ref()
                        .unwrap();
                    let action = if all_in {
                        Action::AllIn(legal.all_in_to)
                    } else {
                        passive_action(legal)
                    };
                    session
                        .send_command(app.prepare_action(action).unwrap())
                        .unwrap();
                }
            }
            if reveal_started.is_some() && !reconnected {
                let token = sessions[1].reconnect_token().unwrap();
                sessions[1].close().unwrap();
                thread::sleep(Duration::from_millis(40));
                let (session, update) = NetworkSession::reconnect(address, "Guest", token).unwrap();
                assert!(update.snapshot.snapshot.showdown.is_some());
                assert!(update.snapshot.snapshot.awards.is_empty());
                apps[1] = NetworkApp::new(
                    ProjectionClient::bootstrap_from_update(update).unwrap(),
                    "Guest-reconnected",
                );
                sessions[1] = session;
                reconnected = true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(reconnected);
        assert!(reveal_started.unwrap().elapsed() >= Duration::from_millis(1_400));
        if all_in {
            assert_eq!(board_lengths, [0, 3, 4, 5]);
        } else {
            assert!(
                reveal_started.unwrap().elapsed() < Duration::from_secs(5),
                "heads-up must settle automatically without the former five-second choice window"
            );
        }
        for app in &apps {
            let snapshot = &app.client().snapshot().snapshot;
            assert_eq!(snapshot.seats.iter().map(|s| s.stack).sum::<u32>(), 200);
            assert!(!snapshot.awards.is_empty());
        }
        for session in &mut sessions {
            let _ = session.close();
        }
        shutdown.store(true, Ordering::Release);
        let summary = worker.join().unwrap();
        assert_eq!(summary.safe_histories, 1);
        assert_eq!(summary.completed_hands, 1);
    }
}
