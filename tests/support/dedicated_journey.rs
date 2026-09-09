use ratatui::{backend::TestBackend, Terminal};
use std::{
    error::Error,
    io::{BufRead, BufReader},
    net::SocketAddr,
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use terminal_poker::{
    game::actions::Action,
    network_client::ProjectionClient,
    network_session::{LobbySession, NetworkSession},
    protocol::TableId,
    tournament::{TournamentConfig, TournamentLevel, TournamentPayoutPlan, TournamentStatus},
    ui::{
        network_app::NetworkApp,
        platform::{apply_terminal_palette, ColorDepth, ThemeMode},
        render::render_practice_view,
        shell::render_tournament_entry,
    },
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn config(name: &str, code: &str) -> TournamentConfig {
    TournamentConfig {
        name: name.into(),
        entrants: 2,
        starting_stack: 1000,
        levels: vec![TournamentLevel {
            small_blind: 25,
            big_blind: 50,
            ante: 5,
            duration_seconds: 60,
            break_after_seconds: 0,
        }],
        payout: TournamentPayoutPlan {
            pool: 101,
            shares_bps: vec![10000],
        },
        join_code: code.into(),
    }
}
fn join_pair(
    address: SocketAddr,
    id: TableId,
    code: &str,
) -> Result<Vec<(NetworkSession, NetworkApp)>> {
    let (a, _, _, _) = LobbySession::connect(address, "Same Player")?;
    let access = code.to_string();
    let worker =
        thread::spawn(move || a.join_and_wait_with_access(id, None, Some(access)).unwrap());
    let (b, _, _, _) = LobbySession::connect(address, "Same Player")?;
    let (b, update_b, _) = b.join_and_wait_with_access(id, None, Some(code.into()))?;
    let (a, update_a, _) = worker.join().unwrap();
    Ok(vec![
        (
            a,
            NetworkApp::new(
                ProjectionClient::bootstrap_from_update(update_a)?,
                "Same Player",
            ),
        ),
        (
            b,
            NetworkApp::new(
                ProjectionClient::bootstrap_from_update(update_b)?,
                "Same Player",
            ),
        ),
    ])
}
fn capture(out: &Path, name: &str, app: Option<&NetworkApp>) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(80, 30))?;
    terminal.draw(|frame| {
        if let Some(app) = app {
            render_practice_view(frame, &app.view(name));
        } else {
            render_tournament_entry(
                frame,
                "CONNECT TO SERVER",
                "Server address",
                "127.0.0.1:7777",
            );
        }
        let area = frame.area();
        apply_terminal_palette(
            frame.buffer_mut(),
            area,
            ThemeMode::Ash,
            ColorDepth::TrueColor,
        );
    })?;
    let cells:Vec<_>=terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"symbol":c.symbol(),"foreground":format!("{:?}",c.fg),"background":format!("{:?}",c.bg),"modifiers":c.modifier.bits()})).collect();
    let state=app.map(|a| {let v=a.view(name); serde_json::json!({"hand":v.hand_id,"phase":format!("{:?}",v.phase),"board":v.board.iter().map(|c|format!("{c}")).collect::<Vec<_>>(),"pot":v.pot_total,"local_seat":v.local_seat.as_u8(),"seats":v.seats.iter().map(|s|serde_json::json!({"seat":s.seat.as_u8(),"stack":s.stack,"contribution":s.contribution,"awarded":s.awarded})).collect::<Vec<_>>()})});
    std::fs::write(
        out.join(format!("{name}.json")),
        serde_json::to_vec_pretty(
            &serde_json::json!({"checkpoint":name,"width":80,"height":30,"cells":cells,"state":state}),
        )?,
    )?;
    Ok(())
}

pub fn run(binary: &Path, output: Option<&Path>) -> Result<()> {
    let child = Command::new(binary)
        .args(["--multi-table", "--bind", "127.0.0.1:0", "--seed", "15000"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut server = Server(child);
    let stdout = server.0.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(stdout).read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx.recv_timeout(Duration::from_secs(10))?;
    let address: SocketAddr = line
        .split_whitespace()
        .nth(1)
        .ok_or("server did not announce address")?
        .parse()?;
    let (mut creator, _, _, _) = LobbySession::connect(address, "Creator")?;
    assert!(creator.list_tables(Default::default())?.is_empty());
    let first = creator.create_tournament(config("Game One", "sprint18-first-private-code"))?;
    let second = creator.create_tournament(config("Open Game", ""))?;
    let rows = creator.list_tables(Default::default())?;
    assert_eq!(rows.len(), 2);
    assert_eq!(
        first.visibility,
        terminal_poker::lobby::TableVisibility::PasswordProtected
    );
    assert_eq!(
        second.visibility,
        terminal_poker::lobby::TableVisibility::Public
    );
    let public = serde_json::to_string(&rows)?;
    for forbidden in [
        "sprint18-first-private-code",
        "join_code",
        "digest_hex",
        "salt_hex",
    ] {
        assert!(!public.contains(forbidden));
    }
    if let Some(out) = output {
        std::fs::create_dir_all(out)?;
        capture_lobby(out, "00-lobby", rows.clone(), 80, 30)?;
        capture_lobby(out, "00-compact-lobby", rows, 40, 20)?;
        capture_lobby(out, "00-empty-lobby", vec![], 80, 30)?;
    }
    let (cancel, _, _, _) = LobbySession::connect(address, "Cancel Player")?;
    assert!(matches!(
        cancel.join_and_wait_while(second.table_id, None, None, |_| false),
        Err(terminal_poker::network_session::NetworkSessionError::JoinCancelled)
    ));
    assert_eq!(creator.inspect_table(second.table_id)?.occupied, 0);
    disconnect_before_start(address, second.table_id)?;
    let cleanup_started = Instant::now();
    while creator.inspect_table(second.table_id)?.occupied != 0 {
        assert!(
            cleanup_started.elapsed() < Duration::from_secs(3),
            "departed registration retained a seat"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let (wrong, _, _, _) = LobbySession::connect(address, "Wrong Password")?;
    assert!(wrong
        .join_and_wait_with_access(first.table_id, None, Some("incorrect".into()))
        .is_err());
    assert_eq!(creator.list_tables(Default::default())?[0].occupied, 0);
    assert_ne!(first.table_id, second.table_id);
    creator.close()?;
    drop(creator);
    assert!(
        server.0.try_wait()?.is_none(),
        "creator exit stopped server"
    );
    let (mut inspect, _, _, _) = LobbySession::connect(address, "Inspector")?;
    assert!(inspect
        .inspect_table_with_access(first.table_id, "incorrect".into())
        .is_err());
    let mut players = join_pair(address, first.table_id, "sprint18-first-private-code")?;
    // Reconnect one seat without restarting the server or changing table identity.
    let token = players[0]
        .0
        .reconnect_token()
        .ok_or("missing reconnect token")?;
    players[0].0.close()?;
    let reconnect_started = Instant::now();
    let (session, update) = loop {
        match NetworkSession::reconnect(address, "Same Player", token.clone()) {
            Ok(result) => break result,
            Err(terminal_poker::network_session::NetworkSessionError::Rejected {
                code, ..
            }) if code == "duplicate_active_session"
                && reconnect_started.elapsed() < Duration::from_secs(3) =>
            {
                thread::sleep(Duration::from_millis(20))
            }
            Err(error) => return Err(error.into()),
        }
    };
    assert_eq!(update.snapshot.table_id, first.table_id);
    players[0] = (
        session,
        NetworkApp::new(
            ProjectionClient::bootstrap_from_update(update)?,
            "Same Player",
        ),
    );
    if let Some(out) = output {
        std::fs::create_dir_all(out)?;
        capture(out, "00-server-entry", None)?;
        capture(out, "01-deal", Some(&players[0].1))?;
    }
    let started = Instant::now();
    let mut sent = [false; 2];
    let mut captured = false;
    let mut board_captured = false;
    while !players.iter().all(|(_, app)| app.is_terminal()) {
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "hand timed out"
        );
        for (session, app) in &mut players {
            for message in session.poll()? {
                app.apply_message(message)?;
            }
        }
        let view = players[0].1.view("progress");
        if !captured && view.current_wager > 50 && view.board.is_empty() {
            if let Some(out) = output {
                capture(out, "02-shove", Some(&players[0].1))?;
            }
            captured = true;
        }
        if !board_captured && !view.board.is_empty() {
            if let Some(out) = output {
                capture(out, "03-runout", Some(&players[0].1))?;
            }
            board_captured = true;
        }
        // Submit at most one command per iteration, allowing authoritative snapshots between actions.
        for (index, (session, app)) in players.iter_mut().enumerate() {
            if !sent[index] && app.client().controls_enabled() {
                let amount = app
                    .client()
                    .snapshot()
                    .snapshot
                    .legal_actions
                    .as_ref()
                    .unwrap()
                    .all_in_to;
                session.send_command(app.prepare_action(Action::AllIn(amount))?)?;
                sent[index] = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    if let Some(out) = output {
        capture(out, "04-award", Some(&players[0].1))?;
    }
    let chip_total: u32 = players[0]
        .1
        .view("done")
        .seats
        .iter()
        .map(|s| s.stack)
        .sum();
    assert_eq!(chip_total, 2000);
    let started = Instant::now();
    loop {
        let state = inspect
            .inspect_table_with_access(first.table_id, "sprint18-first-private-code".into())?
            .tournament
            .unwrap();
        if state.status == TournamentStatus::Complete {
            assert_eq!(state.remaining, 1);
            assert_eq!(state.standings.iter().map(|s| s.payout).sum::<u32>(), 101);
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(5));
        thread::sleep(Duration::from_millis(10));
    }
    for (session, _) in &mut players {
        session.close()?;
    }
    drop(players);
    let waiting = inspect
        .inspect_table_with_access(second.table_id, "".into())?
        .tournament
        .unwrap();
    assert_eq!(waiting.status, TournamentStatus::Registering);
    assert_eq!(waiting.hands_completed, 0);
    let mut second_players = join_pair(address, second.table_id, "")?;
    assert_eq!(
        second_players[0].1.client().snapshot().table_id,
        second.table_id
    );
    let second_view = second_players[0].1.view("second");
    assert_eq!(
        second_view.seats.iter().map(|s| s.stack).sum::<u32>() + second_view.pot_total,
        2000
    );
    for (session, _) in &mut second_players {
        session.close()?;
    }
    assert!(server.0.try_wait()?.is_none(), "game exit stopped server");
    if let Some(out) = output {
        std::fs::write(
            out.join("evidence.json"),
            serde_json::to_vec_pretty(
                &serde_json::json!({"normal_server_process":true,"creator_disconnect_survived":true,"games":2,"isolation":true,"reconnect":true,"winner":true,"chips":2000,"payout":101,"server_survived_game_exit":true}),
            )?,
        )?;
    }
    Ok(())
}

fn capture_lobby(
    out: &Path,
    name: &str,
    tables: Vec<terminal_poker::lobby::PublicTableSummary>,
    width: u16,
    height: u16,
) -> Result<()> {
    use terminal_poker::ui::{
        game_lobby::{render_game_lobby, GameLobby},
        platform::SemanticTheme,
    };
    let mut lobby = GameLobby::default();
    lobby.refresh(tables);
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|f| {
        render_game_lobby(
            f,
            &lobby,
            "127.0.0.1:7777",
            SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor),
        )
    })?;
    let cells: Vec<_> = terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"symbol":c.symbol(),"foreground":format!("{:?}",c.fg),"background":format!("{:?}",c.bg),"modifiers":c.modifier.bits()})).collect();
    std::fs::write(
        out.join(format!("{name}.json")),
        serde_json::to_vec_pretty(
            &serde_json::json!({"checkpoint":name,"width":width,"height":height,"cells":cells}),
        )?,
    )?;
    Ok(())
}

fn disconnect_before_start(address: SocketAddr, table_id: TableId) -> Result<()> {
    use terminal_poker::{
        lobby::{LobbyEnvelope, LobbyRequest, LobbyResult},
        network_transport::{
            read_available, write_message, ClientWireMessage, FrameDecoder, ServerWireMessage,
            WIRE_VERSION,
        },
    };
    let mut stream = std::net::TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    stream.set_nonblocking(true)?;
    let mut decoder = FrameDecoder::default();
    write_message(
        &mut stream,
        &ClientWireMessage::Connect {
            version: WIRE_VERSION,
            label: "Departing Player".into(),
            reconnect: None,
        },
    )?;
    let mut receive = |stream: &mut std::net::TcpStream| -> Result<ServerWireMessage> {
        let started = Instant::now();
        loop {
            read_available(stream, &mut decoder)?;
            if let Some(message) = decoder.decode_next()? {
                return Ok(message);
            }
            if started.elapsed() > Duration::from_secs(3) {
                return Err("raw lobby response timed out".into());
            }
            thread::sleep(Duration::from_millis(2));
        }
    };
    assert!(matches!(
        receive(&mut stream)?,
        ServerWireMessage::LobbyWelcome { .. }
    ));
    write_message(
        &mut stream,
        &ClientWireMessage::Lobby {
            request: LobbyEnvelope::new(
                "drop-registration",
                LobbyRequest::Join {
                    table_id,
                    seat: None,
                    access_code: None,
                },
            ),
        },
    )?;
    assert!(
        matches!(receive(&mut stream)?, ServerWireMessage::Lobby { response } if matches!(response.result,LobbyResult::Joined { ready: false, .. }))
    );
    stream.shutdown(std::net::Shutdown::Both)?;
    Ok(())
}
