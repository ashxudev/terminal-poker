use std::error::Error;
use std::fs;
use std::io;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::{CrosstermBackend, TestBackend};
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::credentials::BearerToken;
use terminal_poker::game::actions::Action;
use terminal_poker::game::multiway::MultiwayPhase;
use terminal_poker::game::seat::{SeatId, TableSize};
use terminal_poker::lobby::PublicTableSummary;
use terminal_poker::lobby::{PublicTableConfig, PublicTableFilter, TableVisibility};
use terminal_poker::network_client::{passive_action, ProjectionClient};
use terminal_poker::network_session::{LobbySession, NetworkSession, NetworkSessionError};
use terminal_poker::protocol::TableId;
use terminal_poker::ui::lobby::LobbyView;
use terminal_poker::ui::network_app::NetworkApp;
use terminal_poker::ui::render::{render_lobby_view, render_network_view};

const SESSION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Parser)]
#[command(
    name = "poker-client",
    about = "Projection-only multiplayer poker TUI",
    version
)]
struct Args {
    #[arg(long)]
    connect: SocketAddr,
    #[arg(long)]
    session: String,
    #[arg(long)]
    credential_file: Option<PathBuf>,
    #[arg(long)]
    headless: bool,
    #[arg(long, requires = "headless")]
    run_seconds: Option<u64>,
    #[arg(long, default_value_t = 1, requires = "headless", value_parser = clap::value_parser!(u64).range(1..))]
    hands: u64,
    #[arg(long, default_value_t = 0, requires = "headless")]
    hand_pause_ms: u64,
    #[arg(long)]
    disconnect_after_revision: Option<u64>,
    #[arg(long, requires = "disconnect_after_revision")]
    probe_rotated_credential: bool,
    #[arg(long, default_value_t = 0, requires = "disconnect_after_revision")]
    disconnect_pause_ms: u64,
    #[arg(long)]
    capture_dir: Option<PathBuf>,
    #[arg(long, conflicts_with_all = ["create_table", "join_table"])]
    lobby_list: bool,
    #[arg(long, conflicts_with_all = ["lobby_list", "create_table", "join_table"])]
    health: bool,
    #[arg(long, conflicts_with_all = ["lobby_list", "join_table"])]
    create_table: Option<String>,
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(2..=9))]
    table_seats: u8,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..))]
    table_stack: u32,
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u8).range(2..=9))]
    min_players: u8,
    #[arg(long, default_value = "public", value_parser = ["public", "unlisted", "private"])]
    table_visibility: String,
    #[arg(long)]
    join_code: Option<String>,
    #[arg(long, conflicts_with_all = ["lobby_list", "create_table"])]
    join_table: Option<u64>,
    #[arg(long, requires = "join_table", value_parser = clap::value_parser!(u8).range(0..=8))]
    seat: Option<u8>,
    #[arg(long, requires = "join_table")]
    probe_wrong_table: Option<u64>,
}

#[derive(Debug, Serialize)]
struct HeadlessSummary {
    session: String,
    table_id: u64,
    hand_id: u64,
    initial_revision: u64,
    initial_awards: usize,
    terminal_revision: u64,
    phase: String,
    stream_sequence: u64,
    reconnects: u64,
    controls_enabled: bool,
    chip_total: u32,
    server_errors: u64,
    old_credential_rejected: bool,
    hands_completed: u64,
    bootstrap_ms: u64,
    accepted_commands: usize,
    command_p99_ms: u64,
    command_max_ms: u64,
    reconnect_max_ms: u64,
}

#[derive(Serialize)]
struct CaptureCell {
    symbol: String,
    foreground: String,
    background: String,
    modifiers: u16,
}

#[derive(Serialize)]
struct NetworkCapture {
    renderer: &'static str,
    backend: &'static str,
    session: String,
    checkpoint: String,
    revision: u64,
    stream_sequence: u64,
    width: u16,
    height: u16,
    cells: Vec<CaptureCell>,
}

#[derive(Serialize)]
struct LobbyCapture {
    renderer: &'static str,
    backend: &'static str,
    checkpoint: String,
    lobby_revision: u64,
    capacity: usize,
    table_ids: Vec<u64>,
    width: u16,
    height: u16,
    cells: Vec<CaptureCell>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let bootstrap_started = Instant::now();
    let join_code = args
        .join_code
        .clone()
        .or_else(|| std::env::var("TERMINAL_POKER_JOIN_CODE").ok());
    if args.health {
        let (mut lobby, _, _, _) = LobbySession::connect(args.connect, args.session.clone())?;
        let health = lobby.health()?;
        let _ = lobby.close();
        println!("{}", serde_json::to_string_pretty(&health)?);
        return Ok(());
    }
    if args.lobby_list {
        let (mut lobby, lobby_revision, capacity, _) =
            LobbySession::connect(args.connect, args.session.clone())?;
        let tables = lobby.list_tables(PublicTableFilter::default())?;
        if let Some(directory) = &args.capture_dir {
            write_lobby_capture(
                directory,
                "public-table-directory",
                lobby_revision,
                usize::from(capacity),
                &tables,
            )?;
        }
        let _ = lobby.close();
        println!("{}", serde_json::to_string(&tables)?);
        return Ok(());
    }
    if let Some(name) = &args.create_table {
        let (mut lobby, _, _, _) = LobbySession::connect(args.connect, args.session.clone())?;
        let table = lobby.create_table(PublicTableConfig {
            name: name.clone(),
            seats: TableSize::new(args.table_seats)?,
            starting_stack: args.table_stack,
            min_players: args.min_players,
            visibility: match args.table_visibility.as_str() {
                "unlisted" => TableVisibility::Unlisted,
                "private" => TableVisibility::Private,
                _ => TableVisibility::Public,
            },
            join_code: join_code.clone(),
        })?;
        let _ = lobby.close();
        println!("{}", serde_json::to_string(&table)?);
        return Ok(());
    }
    let (session, initial) = if let Some(table_id) = args.join_table {
        let (lobby, _, _, _) = LobbySession::connect(args.connect, args.session.clone())?;
        let seat = args.seat.map(SeatId::new).transpose()?;
        let (session, initial, _) =
            lobby.join_and_wait_with_access(TableId(table_id), seat, join_code)?;
        (session, initial)
    } else if let Some(path) = args.credential_file.as_deref().filter(|path| path.exists()) {
        NetworkSession::reconnect(args.connect, args.session.clone(), read_credential(path)?)?
    } else {
        NetworkSession::connect(args.connect, args.session.clone())?
    };
    if let Some(path) = args.credential_file.as_deref() {
        write_credential(path, &session)?;
    }
    let client = ProjectionClient::bootstrap_from_update(initial)?;
    let app = NetworkApp::new(client, args.session.clone());
    if args.headless {
        let bootstrap_ms = elapsed_millis(bootstrap_started.elapsed());
        run_headless(args, session, app, bootstrap_ms)
    } else {
        run_tui(session, app)
    }
}

fn run_headless(
    args: Args,
    mut session: NetworkSession,
    mut app: NetworkApp,
    bootstrap_ms: u64,
) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let mut hand_started = Instant::now();
    let mut reconnects = 0u64;
    let mut hands_completed = 0u64;
    let mut reconnect_max_ms = 0u64;
    let mut command_latencies = Vec::new();
    let mut pending_command_started: Option<Instant> = None;
    let mut fault_reconnect_done = false;
    let initial_revision = app.client().snapshot().revision;
    let initial_awards = app.client().snapshot().snapshot.awards.len();
    let mut last_phase = app.client().snapshot().snapshot.phase;
    let mut last_server_errors = app.server_errors();
    let mut old_credential_rejected = false;
    if let Some(directory) = &args.capture_dir {
        write_capture(directory, "01-connected", &app)?;
    }
    if let Some(wrong_table) = args.probe_wrong_table {
        let snapshot = app.client().snapshot();
        let seat = match snapshot.snapshot.audience {
            terminal_poker::protocol::ProjectionKind::Player { seat } => seat,
            terminal_poker::protocol::ProjectionKind::Spectator => {
                return Err("wrong-table probe requires a player audience".into())
            }
        };
        session.send_command(terminal_poker::protocol::CommandEnvelope::act_for_hand(
            format!("{}-wrong-table-probe", args.session),
            TableId(wrong_table),
            snapshot.hand_id,
            snapshot.revision,
            seat,
            Action::Fold,
        ))?;
    }
    loop {
        if hand_started.elapsed() >= SESSION_TIMEOUT {
            return Err("network hand exceeded 30-second bounded acceptance timeout".into());
        }
        for message in session.poll()? {
            if matches!(
                message,
                terminal_poker::network_transport::ServerWireMessage::Response { .. }
            ) {
                if let Some(command_started) = pending_command_started.take() {
                    command_latencies.push(elapsed_millis(command_started.elapsed()));
                }
            }
            app.apply_message(message)?;
        }
        if app.server_errors() > last_server_errors {
            last_server_errors = app.server_errors();
            if let Some(directory) = &args.capture_dir {
                write_capture(directory, "wrong-table-rejected", &app)?;
            }
        }
        let phase = app.client().snapshot().snapshot.phase;
        if phase != last_phase {
            last_phase = phase;
            if let Some(directory) = &args.capture_dir {
                write_capture(
                    directory,
                    &format!("street-{}", phase.name().to_lowercase()),
                    &app,
                )?;
            }
        }
        if app.is_terminal() {
            hands_completed = hands_completed.saturating_add(1);
            if let Some(directory) = &args.capture_dir {
                write_capture(directory, "99-showdown", &app)?;
            }
            let continue_running = match args.run_seconds {
                Some(seconds) => started.elapsed() < Duration::from_secs(seconds),
                None => hands_completed < args.hands,
            };
            if continue_running {
                if args.hand_pause_ms > 0 {
                    thread::sleep(Duration::from_millis(args.hand_pause_ms));
                }
                let reconnect_token = session.reconnect_token();
                let _ = session.close();
                drop(session);
                app.mark_disconnected();
                let reconnect_started = Instant::now();
                let (next_session, update) =
                    reconnect_with_retry(args.connect, &args.session, reconnect_token)?;
                reconnect_max_ms =
                    reconnect_max_ms.max(elapsed_millis(reconnect_started.elapsed()));
                reconnects = reconnects.saturating_add(1);
                if let Some(path) = args.credential_file.as_deref() {
                    write_credential(path, &next_session)?;
                }
                session = next_session;
                app = NetworkApp::new(
                    ProjectionClient::bootstrap_from_update(update)?,
                    args.session.clone(),
                );
                hand_started = Instant::now();
                last_phase = app.client().snapshot().snapshot.phase;
                last_server_errors = app.server_errors();
                pending_command_started = None;
                continue;
            }
            let _ = session.close();
            command_latencies.sort_unstable();
            let summary = HeadlessSummary {
                session: args.session,
                table_id: app.client().snapshot().table_id.0,
                hand_id: app.client().snapshot().hand_id.0,
                initial_revision,
                initial_awards,
                terminal_revision: app.client().snapshot().revision,
                phase: phase.name().to_string(),
                stream_sequence: app.client().last_stream_sequence(),
                reconnects,
                controls_enabled: app.client().controls_enabled(),
                chip_total: authoritative_chip_total(app.client().snapshot()),
                server_errors: app.server_errors(),
                old_credential_rejected,
                hands_completed,
                bootstrap_ms,
                accepted_commands: command_latencies.len(),
                command_p99_ms: percentile_99(&command_latencies),
                command_max_ms: command_latencies.last().copied().unwrap_or(0),
                reconnect_max_ms,
            };
            println!("{}", serde_json::to_string(&summary)?);
            return Ok(());
        }
        if !fault_reconnect_done
            && args
                .disconnect_after_revision
                .is_some_and(|revision| app.client().snapshot().revision >= revision)
        {
            let reconnect_token = session.reconnect_token();
            drop(session);
            app.mark_disconnected();
            if let Some(directory) = &args.capture_dir {
                write_capture(directory, "disconnect", &app)?;
            }
            if args.disconnect_pause_ms > 0 {
                thread::sleep(Duration::from_millis(args.disconnect_pause_ms));
            }
            let reconnect_started = Instant::now();
            let (new_session, update) =
                reconnect_with_retry(args.connect, &args.session, reconnect_token.clone())?;
            reconnect_max_ms = reconnect_max_ms.max(elapsed_millis(reconnect_started.elapsed()));
            if args.probe_rotated_credential {
                let old = reconnect_token.ok_or("server did not issue a reconnect credential")?;
                old_credential_rejected = matches!(
                    NetworkSession::reconnect(args.connect, args.session.clone(), old),
                    Err(NetworkSessionError::Rejected { ref code, .. })
                        if code == "reconnect_rejected"
                );
                if !old_credential_rejected {
                    return Err("rotated reconnect credential was accepted again".into());
                }
            }
            if let Some(path) = args.credential_file.as_deref() {
                write_credential(path, &new_session)?;
            }
            session = new_session;
            app.apply_message(
                terminal_poker::network_transport::ServerWireMessage::Welcome {
                    update,
                    reconnect: None,
                },
            )?;
            reconnects = reconnects.saturating_add(1);
            fault_reconnect_done = true;
            if let Some(directory) = &args.capture_dir {
                write_capture(directory, "reconnected", &app)?;
            }
        }
        if app.client().controls_enabled() {
            let legal = app
                .client()
                .snapshot()
                .snapshot
                .legal_actions
                .as_ref()
                .expect("enabled controls include legal actions");
            let command = app.prepare_action(passive_action(legal))?;
            session.send_command(command)?;
            pending_command_started = Some(Instant::now());
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn elapsed_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn percentile_99(sorted: &[u64]) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let index = (sorted.len().saturating_mul(99).saturating_add(99) / 100)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[index]
}

fn read_credential(path: &Path) -> Result<BearerToken, Box<dyn Error>> {
    let value = fs::read_to_string(path)?;
    BearerToken::from_client(value.trim().to_string()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "credential file does not contain a valid opaque reconnect token",
        )
        .into()
    })
}

fn write_credential(path: &Path, session: &NetworkSession) -> Result<(), Box<dyn Error>> {
    let Some(token) = session.reconnect_token() else {
        return Ok(());
    };
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, token.expose_to_wire().as_bytes())?;
    Ok(())
}

fn authoritative_chip_total(snapshot: &terminal_poker::protocol::SnapshotEnvelope) -> u32 {
    let stacks = snapshot
        .snapshot
        .seats
        .iter()
        .map(|seat| seat.stack)
        .sum::<u32>();
    if matches!(
        snapshot.snapshot.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        stacks
    } else {
        stacks.saturating_add(snapshot.snapshot.pot_total)
    }
}

fn reconnect_with_retry(
    address: SocketAddr,
    session: &str,
    token: Option<BearerToken>,
) -> Result<
    (
        NetworkSession,
        terminal_poker::authorized_table::SubscriptionUpdate,
    ),
    Box<dyn Error>,
> {
    let started = Instant::now();
    loop {
        let attempt = match token.clone() {
            Some(token) => NetworkSession::reconnect(address, session.to_string(), token),
            None => NetworkSession::connect(address, session.to_string()),
        };
        match attempt {
            Ok(connected) => return Ok(connected),
            Err(NetworkSessionError::Rejected { code, .. })
                if code == "duplicate_active_session"
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(
                NetworkSessionError::HandshakeTimedOut | NetworkSessionError::ClosedDuringHandshake,
            ) if started.elapsed() < Duration::from_secs(5) => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(NetworkSessionError::Transport(
                terminal_poker::network_transport::TransportError::Io(ref error),
            )) if matches!(
                error.kind(),
                io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::UnexpectedEof
            ) && started.elapsed() < Duration::from_secs(5) =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => return Err(Box::new(error)),
        }
    }
}

fn run_tui(mut session: NetworkSession, mut app: NetworkApp) -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = (|| -> Result<(), Box<dyn Error>> {
        loop {
            for message in session.poll()? {
                app.apply_message(message)?;
            }
            let view = app.view("LIVE / q quit / f fold / c call-check / r min-raise / a all-in");
            terminal.draw(|frame| render_network_view(frame, &view))?;
            if event::poll(Duration::from_millis(20))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q')
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL))
                    {
                        let _ = session.close();
                        break;
                    }
                    if let Some(action) = action_for_key(key.code, &app) {
                        let command = app.prepare_action(action)?;
                        session.send_command(command)?;
                    }
                }
            }
        }
        Ok(())
    })();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    result
}

fn action_for_key(code: KeyCode, app: &NetworkApp) -> Option<Action> {
    if !app.client().controls_enabled() {
        return None;
    }
    let legal = app.client().snapshot().snapshot.legal_actions.as_ref()?;
    match code {
        KeyCode::Char('f') | KeyCode::Char('F') if legal.can_fold => Some(Action::Fold),
        KeyCode::Char('c') | KeyCode::Char('C') if legal.can_check => Some(Action::Check),
        KeyCode::Char('c') | KeyCode::Char('C') => legal.call_amount.map(Action::Call),
        KeyCode::Char('r') | KeyCode::Char('R') => legal
            .min_raise_to
            .map(Action::Raise)
            .or_else(|| legal.min_bet_to.map(Action::Bet)),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(Action::AllIn(legal.all_in_to)),
        _ => None,
    }
}

fn write_capture(
    directory: &Path,
    checkpoint: &str,
    app: &NetworkApp,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(directory)?;
    let view = app.view(checkpoint);
    let backend = TestBackend::new(150, 48);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render_network_view(frame, &view))?;
    let buffer = terminal.backend().buffer();
    let capture = NetworkCapture {
        renderer: "terminal_poker::ui::render::render_network_view",
        backend: "ratatui::backend::TestBackend",
        session: format!("{:?}", app.client().snapshot().snapshot.audience),
        checkpoint: checkpoint.to_string(),
        revision: app.client().snapshot().revision,
        stream_sequence: app.client().last_stream_sequence(),
        width: buffer.area.width,
        height: buffer.area.height,
        cells: buffer
            .content
            .iter()
            .map(|cell| CaptureCell {
                symbol: cell.symbol().to_string(),
                foreground: format!("{:?}", cell.fg),
                background: format!("{:?}", cell.bg),
                modifiers: cell.modifier.bits(),
            })
            .collect(),
    };
    fs::write(
        directory.join(format!("{checkpoint}.json")),
        serde_json::to_vec(&capture)?,
    )?;
    Ok(())
}

fn write_lobby_capture(
    directory: &Path,
    checkpoint: &str,
    lobby_revision: u64,
    capacity: usize,
    tables: &[PublicTableSummary],
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(directory)?;
    let view = LobbyView::new(checkpoint, lobby_revision, capacity, tables.to_vec());
    let backend = TestBackend::new(150, 48);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render_lobby_view(frame, &view))?;
    let buffer = terminal.backend().buffer();
    let capture = LobbyCapture {
        renderer: "terminal_poker::ui::render::render_lobby_view",
        backend: "ratatui::backend::TestBackend",
        checkpoint: checkpoint.to_string(),
        lobby_revision,
        capacity,
        table_ids: tables.iter().map(|table| table.table_id.0).collect(),
        width: buffer.area.width,
        height: buffer.area.height,
        cells: buffer
            .content
            .iter()
            .map(|cell| CaptureCell {
                symbol: cell.symbol().to_string(),
                foreground: format!("{:?}", cell.fg),
                background: format!("{:?}", cell.bg),
                modifiers: cell.modifier.bits(),
            })
            .collect(),
    };
    fs::write(
        directory.join(format!("{checkpoint}.json")),
        serde_json::to_vec(&capture)?,
    )?;
    Ok(())
}
