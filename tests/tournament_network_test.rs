use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use terminal_poker::game::actions::Action;
use terminal_poker::network_client::ProjectionClient;
use terminal_poker::network_server::{MultiTableNetworkServer, MultiTableNetworkServerConfig};
use terminal_poker::network_session::LobbySession;
use terminal_poker::tournament::{
    TournamentConfig, TournamentLevel, TournamentPayoutPlan, TournamentStatus,
};
use terminal_poker::ui::network_app::NetworkApp;

#[test]
fn private_tournament_registers_and_rolls_to_a_second_tcp_hand() {
    let shutdown = Arc::new(AtomicBool::new(false));
    let server = MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
        shutdown_requested: Arc::clone(&shutdown),
        deterministic_seed_base: Some(15_001),
        ..MultiTableNetworkServerConfig::default()
    })
    .unwrap();
    let address = server.listen_addr();
    let server_worker = thread::spawn(move || server.run().unwrap());

    let invite = "sprint15-private-invite-123456".to_string();
    let config = TournamentConfig {
        name: "Sprint 15 Freezeout".to_string(),
        entrants: 2,
        starting_stack: 1_000,
        levels: vec![TournamentLevel {
            small_blind: 25,
            big_blind: 50,
            ante: 5,
            duration_seconds: 60,
            break_after_seconds: 0,
        }],
        payout: TournamentPayoutPlan {
            pool: 101,
            shares_bps: vec![10_000],
        },
        join_code: invite.clone(),
    };
    let (mut host_lobby, _, _, _) = LobbySession::connect(address, "Host").unwrap();
    let table = host_lobby.create_tournament(config).unwrap();
    assert!(table.tournament.is_some());

    let table_id = table.table_id;
    let host_invite = invite.clone();
    let host_join = thread::spawn(move || {
        host_lobby
            .join_and_wait_with_access(table_id, None, Some(host_invite))
            .unwrap()
    });
    let (guest_lobby, _, _, _) = LobbySession::connect(address, "Guest").unwrap();
    let (mut guest_session, guest_update, _) = guest_lobby
        .join_and_wait_with_access(table_id, None, Some(invite.clone()))
        .unwrap();
    let (mut host_session, host_update, _) = host_join.join().unwrap();

    for update in [&host_update, &guest_update] {
        assert_eq!(update.snapshot.snapshot.small_blind_amount, 25);
        assert_eq!(update.snapshot.snapshot.big_blind_amount, 50);
        assert_eq!(update.snapshot.snapshot.ante_amount, 5);
    }
    let mut host_app = NetworkApp::new(
        ProjectionClient::bootstrap_from_update(host_update).unwrap(),
        "Host",
    );
    let mut guest_app = NetworkApp::new(
        ProjectionClient::bootstrap_from_update(guest_update).unwrap(),
        "Guest",
    );
    let first_hand_id = host_app.client().snapshot().hand_id;
    let first_started = Instant::now();
    let mut fold_sent = false;
    while !(host_app.is_terminal() && guest_app.is_terminal()) {
        assert!(first_started.elapsed() < Duration::from_secs(10));
        for message in host_session.poll().unwrap() {
            host_app.apply_message(message).unwrap();
        }
        for message in guest_session.poll().unwrap() {
            guest_app.apply_message(message).unwrap();
        }
        if !fold_sent && host_app.client().controls_enabled() {
            host_session
                .send_command(host_app.prepare_action(Action::Fold).unwrap())
                .unwrap();
            fold_sent = true;
        } else if !fold_sent && guest_app.client().controls_enabled() {
            guest_session
                .send_command(guest_app.prepare_action(Action::Fold).unwrap())
                .unwrap();
            fold_sent = true;
        }
        thread::sleep(Duration::from_millis(2));
    }
    let host_token = host_session.reconnect_token().unwrap();
    let guest_token = guest_session.reconnect_token().unwrap();
    let _ = host_session.close();
    let _ = guest_session.close();
    thread::sleep(Duration::from_millis(100));
    let (next_host, next_host_update) =
        terminal_poker::network_session::NetworkSession::reconnect(address, "Host", host_token)
            .unwrap();
    let (next_guest, next_guest_update) =
        terminal_poker::network_session::NetworkSession::reconnect(address, "Guest", guest_token)
            .unwrap();
    host_session = next_host;
    guest_session = next_guest;
    host_app = NetworkApp::new(
        ProjectionClient::bootstrap_from_update(next_host_update).unwrap(),
        "Host",
    );
    guest_app = NetworkApp::new(
        ProjectionClient::bootstrap_from_update(next_guest_update).unwrap(),
        "Guest",
    );
    assert!(host_app.client().snapshot().hand_id.0 > first_hand_id.0);
    assert_eq!(
        guest_app.client().snapshot().hand_id,
        host_app.client().snapshot().hand_id
    );
    assert_eq!(
        host_app
            .client()
            .snapshot()
            .snapshot
            .seats
            .iter()
            .map(|seat| seat.stack)
            .sum::<u32>()
            + host_app.client().snapshot().snapshot.pot_total,
        2_000
    );

    let (mut observer, _, _, _) = LobbySession::connect(address, "Observer").unwrap();
    let state = observer
        .inspect_table_with_access(table_id, invite.clone())
        .unwrap()
        .tournament
        .unwrap();
    assert_eq!(state.status, TournamentStatus::Running);
    assert_eq!(state.hands_completed, 1);

    let _ = host_session.close();
    let _ = guest_session.close();
    shutdown.store(true, Ordering::Release);
    let server_summary = server_worker.join().unwrap();
    assert!(server_summary.connections_accepted >= 3);
}

#[test]
fn private_tournament_reaches_one_winner_and_reconciles_payout_over_tcp() {
    let shutdown = Arc::new(AtomicBool::new(false));
    let server = MultiTableNetworkServer::start(MultiTableNetworkServerConfig {
        shutdown_requested: Arc::clone(&shutdown),
        deterministic_seed_base: Some(15_000),
        ..MultiTableNetworkServerConfig::default()
    })
    .unwrap();
    let address = server.listen_addr();
    let server_worker = thread::spawn(move || server.run().unwrap());
    let invite = "sprint15-winner-invite-1234567".to_string();
    let config = TournamentConfig {
        name: "Winner Freezeout".to_string(),
        entrants: 2,
        starting_stack: 1_000,
        levels: vec![TournamentLevel {
            small_blind: 25,
            big_blind: 50,
            ante: 5,
            duration_seconds: 60,
            break_after_seconds: 0,
        }],
        payout: TournamentPayoutPlan {
            pool: 101,
            shares_bps: vec![10_000],
        },
        join_code: invite.clone(),
    };
    let (mut host_lobby, _, _, _) = LobbySession::connect(address, "WinnerHost").unwrap();
    let table = host_lobby.create_tournament(config).unwrap();
    let table_id = table.table_id;
    let host_invite = invite.clone();
    let host_join = thread::spawn(move || {
        host_lobby
            .join_and_wait_with_access(table_id, None, Some(host_invite))
            .unwrap()
    });
    let (guest_lobby, _, _, _) = LobbySession::connect(address, "WinnerGuest").unwrap();
    let (mut guest_session, guest_update, _) = guest_lobby
        .join_and_wait_with_access(table_id, None, Some(invite.clone()))
        .unwrap();
    let (mut host_session, host_update, _) = host_join.join().unwrap();
    let mut host_app = NetworkApp::new(
        ProjectionClient::bootstrap_from_update(host_update).unwrap(),
        "WinnerHost",
    );
    let mut guest_app = NetworkApp::new(
        ProjectionClient::bootstrap_from_update(guest_update).unwrap(),
        "WinnerGuest",
    );
    let started = Instant::now();
    let mut host_sent = false;
    let mut guest_sent = false;
    while !(host_app.is_terminal() && guest_app.is_terminal()) {
        assert!(started.elapsed() < Duration::from_secs(10));
        for message in host_session.poll().unwrap() {
            host_app.apply_message(message).unwrap();
        }
        for message in guest_session.poll().unwrap() {
            guest_app.apply_message(message).unwrap();
        }
        if !host_sent && host_app.client().controls_enabled() {
            let amount = host_app
                .client()
                .snapshot()
                .snapshot
                .legal_actions
                .as_ref()
                .unwrap()
                .all_in_to;
            host_session
                .send_command(host_app.prepare_action(Action::AllIn(amount)).unwrap())
                .unwrap();
            host_sent = true;
        }
        if !guest_sent && guest_app.client().controls_enabled() {
            let amount = guest_app
                .client()
                .snapshot()
                .snapshot
                .legal_actions
                .as_ref()
                .unwrap()
                .all_in_to;
            guest_session
                .send_command(guest_app.prepare_action(Action::AllIn(amount)).unwrap())
                .unwrap();
            guest_sent = true;
        }
        thread::sleep(Duration::from_millis(2));
    }
    let inspect_started = Instant::now();
    let result = loop {
        assert!(inspect_started.elapsed() < Duration::from_secs(5));
        let (mut observer, _, _, _) = LobbySession::connect(address, "WinnerObserver").unwrap();
        let state = observer
            .inspect_table_with_access(table_id, invite.clone())
            .unwrap()
            .tournament
            .unwrap();
        if state.status == TournamentStatus::Complete {
            break state;
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(result.remaining, 1);
    assert_eq!(result.standings.len(), 2);
    assert_eq!(result.standings[0].place, 1);
    assert_eq!(result.standings[0].payout, 101);
    assert_eq!(result.standings[1].place, 2);
    assert_eq!(
        result
            .standings
            .iter()
            .map(|place| place.payout)
            .sum::<u32>(),
        101
    );

    let _ = host_session.close();
    let _ = guest_session.close();
    shutdown.store(true, Ordering::Release);
    let server_summary = server_worker.join().unwrap();
    assert!(server_summary.connections_accepted >= 3);
}
