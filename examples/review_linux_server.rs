//! Real existing-server acceptance over the production verified TLS transport.
use ratatui::{backend::TestBackend, Terminal};
use std::{
    error::Error,
    net::SocketAddr,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use terminal_poker::{
    game::actions::Action,
    network_client::ProjectionClient,
    network_session::{LobbySession, NetworkSession, NetworkSessionError},
    tournament::{TournamentConfig, TournamentLevel, TournamentPayoutPlan},
    ui::{
        game_lobby::{render_game_lobby, GameLobby},
        network_app::NetworkApp,
        platform::{apply_terminal_palette, ColorDepth, SemanticTheme, ThemeMode},
        render::render_practice_view,
    },
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn capture(out: &Path, name: &str, app: &NetworkApp) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(80, 30))?;
    terminal.draw(|f| {
        render_practice_view(f, &app.view(name));
        let area = f.area();
        apply_terminal_palette(f.buffer_mut(), area, ThemeMode::Ash, ColorDepth::TrueColor);
    })?;
    let v = app.view(name);
    let state = serde_json::json!({"hand":v.hand_id,"table":app.client().snapshot().table_id,
        "phase":format!("{:?}",v.phase),"board":v.board.iter().map(|c|format!("{c}")).collect::<Vec<_>>(),
        "pot":v.pot_total,"local_seat":v.local_seat.as_u8(),
        "seats":v.seats.iter().map(|s|serde_json::json!({"seat":s.seat.as_u8(),"stack":s.stack,"contribution":s.contribution,"awarded":s.awarded})).collect::<Vec<_>>()});
    write_capture(out, name, &terminal, Some(state))
}
fn write_capture(
    out: &Path,
    name: &str,
    terminal: &Terminal<TestBackend>,
    state: Option<serde_json::Value>,
) -> Result<()> {
    let cells:Vec<_>=terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"symbol":c.symbol(),"foreground":format!("{:?}",c.fg),"background":format!("{:?}",c.bg),"modifiers":c.modifier.bits()})).collect();
    std::fs::write(
        out.join(format!("{name}.json")),
        serde_json::to_vec_pretty(
            &serde_json::json!({"backend":"ratatui::backend::TestBackend","checkpoint":name,"width":80,"height":30,"cells":cells,"state":state}),
        )?,
    )?;
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let address: SocketAddr = args.get(1).ok_or("server address required")?.parse()?;
    let out = Path::new(args.get(2).ok_or("output directory required")?);
    std::fs::create_dir_all(out)?;
    let (mut owner, _, _, _) = LobbySession::connect(address, "Linux review owner")?;
    let make_config = |name: &str, code: &str| TournamentConfig {
        name: name.into(),
        entrants: 2,
        starting_stack: 1000,
        levels: vec![TournamentLevel {
            small_blind: 25,
            big_blind: 50,
            ante: 5,
            duration_seconds: 600,
            break_after_seconds: 0,
        }],
        payout: TournamentPayoutPlan {
            pool: 1000,
            shares_bps: vec![10000],
        },
        join_code: code.into(),
    };
    let first =
        owner.create_tournament(make_config("Linux Sprint Review", "review-only-password"))?;
    let second = owner.create_tournament(make_config("Independent Open Game", ""))?;
    let rows = owner.list_tables(Default::default())?;
    assert!(rows.iter().any(|r| r.table_id == first.table_id));
    assert!(rows.iter().any(|r| r.table_id == second.table_id));
    assert!(!serde_json::to_string(&rows)?.contains("review-only-password"));
    let mut lobby = GameLobby::default();
    lobby.refresh(rows);
    let mut terminal = Terminal::new(TestBackend::new(80, 30))?;
    terminal.draw(|f| {
        render_game_lobby(
            f,
            &lobby,
            &format!("{address} | Linux authority"),
            SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor),
        )
    })?;
    write_capture(out, "00-linux-lobby", &terminal, None)?;
    let (wrong, _, _, _) = LobbySession::connect(address, "Rejected review")?;
    assert!(wrong
        .join_and_wait_with_access(first.table_id, None, Some("incorrect".into()))
        .is_err());
    assert_eq!(
        owner
            .inspect_table_with_access(first.table_id, "review-only-password".into())?
            .occupied,
        0
    );
    let (cancel, _, _, _) = LobbySession::connect(address, "Cancelled review")?;
    assert!(matches!(
        cancel.join_and_wait_while(second.table_id, None, None, |_| false),
        Err(terminal_poker::network_session::NetworkSessionError::JoinCancelled)
    ));
    assert_eq!(owner.inspect_table(second.table_id)?.occupied, 0);
    owner.close()?;
    let (a, _, _, _) = LobbySession::connect(address, "Linux review S0")?;
    let table = first.table_id;
    let worker = thread::spawn(move || {
        a.join_and_wait_with_access(table, None, Some("review-only-password".into()))
            .unwrap()
    });
    // Wait for the first registration so observer seat identity is stable.
    let (mut inspector, _, _, _) = LobbySession::connect(address, "Linux inspector")?;
    let started = Instant::now();
    while inspector
        .inspect_table_with_access(table, "review-only-password".into())?
        .occupied
        != 1
    {
        assert!(started.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(20));
    }
    // Reproduce real human setup time: the host must survive more than one
    // admission window before a second terminal joins.
    thread::sleep(Duration::from_secs(12));
    let (b, _, _, _) = LobbySession::connect(address, "Linux review S1")?;
    let (b, ub, _) =
        b.join_and_wait_with_access(table, None, Some("review-only-password".into()))?;
    let (a, ua, _) = worker.join().unwrap();
    let mut players = vec![
        (
            a,
            NetworkApp::new(
                ProjectionClient::bootstrap_from_update(ua)?,
                "Linux review S0",
            ),
        ),
        (
            b,
            NetworkApp::new(
                ProjectionClient::bootstrap_from_update(ub)?,
                "Linux review S1",
            ),
        ),
    ];
    let token = players[0]
        .0
        .reconnect_token()
        .ok_or("missing reconnect token")?;
    let hand = players[0].1.client().snapshot().snapshot.hand_id;
    players[0].0.close()?;
    let reconnect_started = Instant::now();
    let (session, update) = loop {
        match NetworkSession::reconnect(address, "Linux review S0", token.clone()) {
            Ok(result) => break result,
            Err(NetworkSessionError::Rejected { code, .. })
                if code == "duplicate_active_session"
                    && reconnect_started.elapsed() < Duration::from_secs(3) =>
            {
                thread::sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error.into()),
        }
    };
    assert_eq!(update.snapshot.table_id, table);
    assert_eq!(update.snapshot.hand_id, hand);
    players[0] = (
        session,
        NetworkApp::new(
            ProjectionClient::bootstrap_from_update(update)?,
            "Linux review S0",
        ),
    );
    capture(out, "01-deal", &players[0].1)?;
    let mut sent = [false; 2];
    let mut shove = false;
    let mut runout = false;
    let started = Instant::now();
    while !players.iter().all(|(_, a)| a.is_terminal()) {
        assert!(
            started.elapsed() < Duration::from_secs(40),
            "remote hand timed out"
        );
        for (s, a) in &mut players {
            for msg in s.poll()? {
                a.apply_message(msg)?;
            }
        }
        let view = players[0].1.view("review");
        if !shove && view.current_wager > 50 && view.board.is_empty() {
            capture(out, "02-shove", &players[0].1)?;
            shove = true;
        }
        if !runout && !view.board.is_empty() {
            capture(out, "03-runout", &players[0].1)?;
            runout = true;
        }
        for (i, (s, a)) in players.iter_mut().enumerate() {
            if !sent[i] && a.client().controls_enabled() {
                let amount = a
                    .client()
                    .snapshot()
                    .snapshot
                    .legal_actions
                    .as_ref()
                    .unwrap()
                    .all_in_to;
                s.send_command(a.prepare_action(Action::AllIn(amount))?)?;
                sent[i] = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    capture(out, "04-award", &players[0].1)?;
    let total: u32 = players[0].1.view("end").seats.iter().map(|s| s.stack).sum();
    assert_eq!(total, 2000);
    for (s, _) in &mut players {
        s.close()?;
    }
    let independent = inspector
        .inspect_table(second.table_id)?
        .tournament
        .unwrap();
    assert_eq!(independent.hands_completed, 0);
    assert_eq!(inspector.inspect_table(second.table_id)?.occupied, 0);
    let completed = inspector
        .inspect_table_with_access(table, "review-only-password".into())?
        .tournament
        .unwrap();
    assert_eq!(completed.hands_completed, 1);
    inspector.close()?;
    std::fs::write(
        out.join("evidence.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"endpoint":address,"table":table,"hand":hand,"chips":total,"games":2,"tls_reconnect":true,"creator_disconnect_survived":true,"password_rejection":true,"cancelled_registration":true,"independent_game_unchanged":true,"server_survived_game_exit":true,"tournament_status":format!("{:?}",completed.status)}),
        )?,
    )?;
    println!("REMOTE_LINUX_JOURNEY_PASS chips={total} tables=2");
    Ok(())
}
