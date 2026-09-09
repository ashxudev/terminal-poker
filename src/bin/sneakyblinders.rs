use std::error::Error;
use std::io;
use std::net::SocketAddr;
use std::thread;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use terminal_poker::game::actions::Action;
use terminal_poker::game_invite::game_server_address;
use terminal_poker::lobby::{PublicTableFilter, PublicTableSummary, TableVisibility};
use terminal_poker::local_practice::{LocalPractice, PracticeSession};
use terminal_poker::local_profile::{LocalProfile, ProfileStore};
use terminal_poker::network_client::ProjectionClient;
use terminal_poker::network_session::{LobbySession, NetworkSession, NetworkSessionError};
use terminal_poker::protocol::TableId;
use terminal_poker::tournament::{
    TournamentConfig, TournamentLevel, TournamentPayoutPlan, TournamentPublicState,
};
use terminal_poker::ui::branded_menu::BrandedMenu;
use terminal_poker::ui::game_lobby::{admission_label, render_game_lobby, GameLobby};
use terminal_poker::ui::multiway_review::{terminal_hold, MultiwayReviewView, ShowdownStage};
use terminal_poker::ui::network_app::NetworkApp;
use terminal_poker::ui::platform::{
    apply_terminal_palette, ColorDepth, PresentationEffects, SemanticTheme, TerminalCapabilities,
    ThemeMode,
};
use terminal_poker::ui::render::{render_practice_view_with_state, RaiseSizingView};
use terminal_poker::ui::shell::{
    render_shell, render_tournament_entry, render_tournament_result, ShellApp, ShellEffect,
    ShellEvent, ShellRoute, HOME_MIN_HEIGHT, HOME_MIN_WIDTH, MIN_HEIGHT, MIN_WIDTH,
};
use terminal_poker::ui::turn_attention::TurnAttention;

#[derive(Debug, Parser)]
#[command(
    name = "sneakyblinders",
    about = "Sneaky Blinders play-money poker",
    version
)]
struct Args {}

fn main() -> Result<(), Box<dyn Error>> {
    Args::parse();
    install_panic_restore_hook();
    let mut session = TerminalSession::enter()?;
    let result = run_shell(session.terminal_mut());
    let restore_result = session.restore();
    result?;
    restore_result?;
    Ok(())
}

fn run_shell(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<(), Box<dyn Error>> {
    let profile_store = ProfileStore::platform_default()?;
    let (profile, profile_status) = match profile_store.load() {
        Ok(Some(profile)) => (
            profile,
            format!("Profile loaded · {}", profile_store.path().display()),
        ),
        Ok(None) => (
            LocalProfile::default(),
            "Ready · new local profile will be saved from Settings".to_string(),
        ),
        Err(error) => (
            LocalProfile::default(),
            format!("Profile source preserved · using safe defaults · {error}"),
        ),
    };
    let mut app = ShellApp::new(profile);
    app.set_status(profile_status);
    let capabilities = TerminalCapabilities::detect();
    let mut branding = if capabilities.color_depth == ColorDepth::TrueColor {
        BrandedMenu::detect()
    } else {
        None
    };
    terminal.clear()?;
    let mut dirty = true;
    let mut previous_size = terminal.size()?;
    let mut effects = PresentationEffects::new(false);
    let mut last_frame = Instant::now();
    let mut practice_session: Option<PracticeSession> = None;
    let profile_path = profile_store.path().to_string_lossy().into_owned();
    loop {
        let elapsed = last_frame.elapsed();
        last_frame = Instant::now();
        let theme = SemanticTheme::resolve(app.profile().theme_mode(), capabilities.color_depth);
        let size = terminal.size()?;
        if size != previous_size {
            terminal.clear()?;
            previous_size = size;
            dirty = true;
        }
        let branded = app.route() == ShellRoute::Home
            && app.profile().theme_mode() == ThemeMode::Ash
            && branding.as_mut().is_some_and(|menu| menu.prepare(size));
        // Image protocols emit terminal commands: redraw only after input/resize,
        // and never recolor or animate their placeholder cells.
        if dirty || !branded {
            terminal.draw(|frame| {
                if branded {
                    branding
                        .as_ref()
                        .unwrap()
                        .render(frame, app.selected_home_item());
                } else {
                    render_shell(frame, &app, &profile_path, &theme);
                    let area = frame.area();
                    effects.process(elapsed, frame.buffer_mut(), area);
                    apply_terminal_palette(
                        frame.buffer_mut(),
                        area,
                        app.profile().theme_mode(),
                        capabilities.color_depth,
                    );
                }
            })?;
            dirty = false;
        }
        if !event::poll(Duration::from_millis(50))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !is_actionable_key(&key) {
            continue;
        }
        let size = terminal.size()?;
        let (minimum_width, minimum_height) = if app.route() == ShellRoute::Home {
            (HOME_MIN_WIDTH, HOME_MIN_HEIGHT)
        } else {
            (MIN_WIDTH, MIN_HEIGHT)
        };
        if size.width < minimum_width || size.height < minimum_height {
            if key.code == KeyCode::Esc {
                app.handle(ShellEvent::Back);
                dirty = true;
            }
            if is_quit_key(&key) {
                return Ok(());
            }
            continue;
        }
        let shell_event = shell_event_for_key(&key, app.route(), app.editing_name());
        let Some(shell_event) = shell_event else {
            continue;
        };
        let previous_route = app.route();
        let effect = app.handle(shell_event);
        dirty = true;
        if branded && (app.route() != previous_route || effect != ShellEffect::None) {
            // Remove terminal-owned images before another screen or game starts.
            terminal.clear()?;
        }
        if app.route() != previous_route {
            effects.begin_route_transition(theme.screen);
        }
        match effect {
            ShellEffect::None => {}
            ShellEffect::SaveProfile => match profile_store.save(app.profile()) {
                Ok(()) => {
                    app.set_status(format!("Settings saved atomically · {profile_path}"));
                }
                Err(error) => {
                    app.handle(ShellEvent::Failure(format!(
                        "Could not save settings; existing source preserved · {error}"
                    )));
                }
            },
            ShellEffect::Quit => return Ok(()),
            ShellEffect::StartHostTournament => {
                let result = run_host_tournament(
                    terminal,
                    app.profile_mut(),
                    &profile_store,
                    capabilities.color_depth,
                );
                match result {
                    Ok(()) => app.set_status("Tournament session ended · Ready"),
                    Err(error) if error.downcast_ref::<EntryCancelled>().is_some() => {
                        app.set_status("Ready")
                    }
                    Err(error) => {
                        app.handle(ShellEvent::Failure(format!("Host failed · {error}")));
                    }
                }
            }
            ShellEffect::StartJoinTournament => {
                let result = run_join_tournament(
                    terminal,
                    app.profile_mut(),
                    &profile_store,
                    capabilities.color_depth,
                );
                match result {
                    Ok(()) => app.set_status("Tournament session ended · Ready"),
                    Err(error) if error.downcast_ref::<EntryCancelled>().is_some() => {
                        app.set_status("Ready")
                    }
                    Err(error) => {
                        app.handle(ShellEvent::Failure(format!("Join failed · {error}")));
                    }
                }
            }
            ShellEffect::StartQuickPractice => {
                if practice_session.is_none() {
                    practice_session = Some(PracticeSession::nine_handed(
                        app.profile().quick_starting_stack,
                    )?);
                }
                run_quick_practice(
                    terminal,
                    practice_session
                        .as_mut()
                        .expect("practice session was initialized"),
                    app.profile().theme_mode(),
                    capabilities.color_depth,
                )?;
                practice_session = None;
                let previous_route = app.route();
                app.handle(ShellEvent::Back);
                if app.route() != previous_route {
                    effects.begin_route_transition(theme.screen);
                }
            }
        }
        if effect != ShellEffect::None {
            terminal.clear()?;
        }
    }
}

fn run_host_tournament(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    profile: &mut LocalProfile,
    store: &ProfileStore,
    color_depth: ColorDepth,
) -> Result<(), Box<dyn Error>> {
    let display_name = profile.display_name.clone();
    let theme_mode = profile.theme_mode();
    let (address, mut lobby) = connect_game_server(terminal, profile, store, false)?;
    let name = loop {
        let name = prompt_text(
            terminal,
            "HOST GAME",
            "Game name (1-32 letters, numbers, spaces)",
            "Sneaky Freezeout",
        )?;
        if name.len() <= 32
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_'))
        {
            break name;
        }
    };
    let join_code = prompt_password(terminal, "HOST GAME", true)?;
    let entrants = prompt_number(terminal, "HOST TOURNAMENT", "Players (2-9)", 2, 2, 9)?;
    let starting_stack = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Starting stack (100-1000000)",
        3_000,
        100,
        1_000_000,
    )?;
    let small_blind = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Starting small blind",
        25,
        1,
        499_999,
    )?;
    let big_blind = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Starting big blind",
        50,
        small_blind + 1,
        500_000,
    )?;
    let ante = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Starting ante (0 allowed)",
        0,
        0,
        big_blind,
    )?;
    let level_minutes = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Minutes per level (1-120)",
        10,
        1,
        120,
    )?;
    let level_count = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Number of levels (1-64; final repeats)",
        8,
        1,
        64,
    )?;
    let break_after_level = if level_count > 1 {
        prompt_number(
            terminal,
            "HOST TOURNAMENT",
            "Break after level (0=none)",
            0,
            0,
            level_count - 1,
        )?
    } else {
        0
    };
    let break_minutes = if break_after_level > 0 {
        prompt_number(
            terminal,
            "HOST TOURNAMENT",
            "Break minutes (1-30)",
            5,
            1,
            30,
        )?
    } else {
        0
    };
    let payout_pool = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Play-money result pool",
        1_000,
        1,
        1_000_000,
    )?;
    let payout_choice = prompt_number(
        terminal,
        "HOST TOURNAMENT",
        "Payout: 1=WTA, 2=70/30, 3=50/30/20",
        1,
        1,
        entrants.min(3),
    )?;
    if starting_stack < big_blind.saturating_mul(20) {
        return Err("starting stack must be at least 20 starting big blinds".into());
    }

    let levels = (0..level_count)
        .map(|index| {
            let multiplier = 1u32.checked_shl(index.min(15)).unwrap_or(u32::MAX);
            TournamentLevel {
                small_blind: small_blind.saturating_mul(multiplier),
                big_blind: big_blind.saturating_mul(multiplier),
                ante: ante.saturating_mul(multiplier),
                duration_seconds: level_minutes.saturating_mul(60),
                break_after_seconds: if index + 1 == break_after_level {
                    break_minutes.saturating_mul(60)
                } else {
                    0
                },
            }
        })
        .collect::<Vec<_>>();
    let shares_bps = match payout_choice {
        2 => vec![7_000, 3_000],
        3 => vec![5_000, 3_000, 2_000],
        _ => vec![10_000],
    };
    let config = TournamentConfig {
        name,
        entrants: u8::try_from(entrants)?,
        starting_stack,
        levels,
        payout: TournamentPayoutPlan {
            pool: payout_pool,
            shares_bps,
        },
        join_code: join_code.clone(),
    };
    config.validate()?;

    let table = lobby.create_tournament(config)?;
    let tournament_state = table
        .tournament
        .clone()
        .ok_or("host response omitted tournament state")?;
    let (session, initial, _) = wait_for_game(terminal, lobby, &table, &join_code)?;
    run_network_tournament(
        terminal,
        address,
        &display_name,
        session,
        initial,
        table.table_id,
        &join_code,
        tournament_state,
        theme_mode,
        color_depth,
    )
}

fn run_join_tournament(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    profile: &mut LocalProfile,
    store: &ProfileStore,
    color_depth: ColorDepth,
) -> Result<(), Box<dyn Error>> {
    let theme_mode = profile.theme_mode();
    let theme = SemanticTheme::resolve(theme_mode, color_depth);
    let (mut address, mut connection) = connect_game_server(terminal, profile, store, false)?;
    let mut directory = GameLobby::default();
    let mut refresh = true;
    let mut last_refresh = Instant::now();
    loop {
        if refresh || (directory.connected && last_refresh.elapsed() >= Duration::from_secs(2)) {
            match connection.list_tables(PublicTableFilter::default()) {
                Ok(tables) => directory.refresh(tables),
                Err(_) => {
                    directory.connected = false;
                    directory.status = "Connection lost. R: Retry or S: Change server".into();
                }
            }
            refresh = false;
            last_refresh = Instant::now();
        }
        terminal.draw(|frame| render_game_lobby(frame, &directory, &address.to_string(), theme))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !is_actionable_key(&key) {
            continue;
        }
        match key.code {
            KeyCode::Esc => {
                let _ = connection.close();
                return Ok(());
            }
            KeyCode::Up => directory.move_selection(false),
            KeyCode::Down => directory.move_selection(true),
            KeyCode::Char('r' | 'R') => {
                if !directory.connected {
                    match LobbySession::connect(address, profile.display_name.clone()) {
                        Ok((lobby, _, _, _)) => {
                            connection = lobby;
                            directory.status.clear();
                        }
                        Err(_) => {
                            directory.status =
                                "Server unavailable. R: Retry or S: Change server".into();
                            continue;
                        }
                    }
                }
                refresh = true;
            }
            KeyCode::Char('s' | 'S') => match connect_game_server(terminal, profile, store, true) {
                Ok((next_address, lobby)) => {
                    let _ = connection.close();
                    address = next_address;
                    connection = lobby;
                    directory = GameLobby::default();
                    refresh = true;
                }
                Err(error) if error.downcast_ref::<EntryCancelled>().is_some() => {}
                Err(error) => directory.status = error.to_string(),
            },
            KeyCode::Enter if directory.connected => {
                let Some(table) = directory.selection().cloned() else {
                    continue;
                };
                if !table.joinable || table.tournament.is_none() {
                    directory.status = admission_label(&table).into();
                    continue;
                }
                let password = if table.visibility == TableVisibility::PasswordProtected {
                    match prompt_password(terminal, &table.name, false) {
                        Ok(password) => password,
                        Err(error) if error.downcast_ref::<EntryCancelled>().is_some() => continue,
                        Err(error) => return Err(error),
                    }
                } else {
                    String::new()
                };
                let result = (|| -> Result<(), Box<dyn Error>> {
                    let (lobby, _, _, _) =
                        LobbySession::connect(address, profile.display_name.clone())?;
                    let (session, initial, _) = wait_for_game(terminal, lobby, &table, &password)?;
                    run_network_tournament(
                        terminal,
                        address,
                        &profile.display_name,
                        session,
                        initial,
                        table.table_id,
                        &password,
                        table.tournament.clone().expect("tournament row"),
                        theme_mode,
                        color_depth,
                    )
                })();
                match result {
                    Ok(()) => directory.status = "Game session ended.".into(),
                    Err(error) if error.downcast_ref::<EntryCancelled>().is_some() => {
                        directory.status = "Registration cancelled.".into()
                    }
                    Err(_) => {
                        directory.status =
                            "Could not join. Check password; game may have filled or closed.".into()
                    }
                }
                refresh = true;
            }
            _ => {}
        }
    }
}

fn connect_game_server(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    profile: &mut LocalProfile,
    store: &ProfileStore,
    change: bool,
) -> Result<(SocketAddr, LobbySession), Box<dyn Error>> {
    let mut endpoint =
        terminal_poker::game_stream::automatic_endpoint(profile.server_address.as_deref())
            .to_string();
    let mut prompt = change;
    let mut label = "Server address".to_string();
    loop {
        if prompt {
            endpoint = prompt_text(terminal, "CONNECT TO SERVER", &label, &endpoint)?;
        }
        let address = match game_server_address(&endpoint) {
            Ok(address) => address,
            Err(_) => {
                label = "Use the server IP:port, e.g. 192.168.5.250:6969".into();
                prompt = true;
                continue;
            }
        };
        show_notice(
            terminal,
            "CONNECT TO SERVER",
            &["Connecting securely...", "Esc: Back"],
        )?;
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let name = profile.display_name.clone();
        std::thread::spawn(move || {
            let _ = sender.send(LobbySession::connect(address, name));
        });
        let result = loop {
            match receiver.try_recv() {
                Ok(result) => break result,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Err("Connection attempt ended".into())
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
            if event::poll(Duration::from_millis(30))? {
                match event::read()? {
                    Event::Key(key) if is_actionable_key(&key) && key.code == KeyCode::Esc => {
                        return Err(Box::new(EntryCancelled))
                    }
                    Event::Resize(_, _) => show_notice(
                        terminal,
                        "CONNECT TO SERVER",
                        &["Connecting securely...", "Esc: Back"],
                    )?,
                    _ => {}
                }
            }
        };
        match result {
            Ok((lobby, _, _, _)) => {
                profile.server_address = Some(address.to_string());
                store.save(profile)?;
                return Ok((address, lobby));
            }
            Err(_) => {
                label = "Could not connect securely. Enter: Retry / Esc: Back".into();
                prompt = true;
            }
        }
    }
}

fn prompt_password(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    title: &str,
    allow_empty: bool,
) -> Result<String, Box<dyn Error>> {
    let mut value = String::new();
    let label = if allow_empty {
        "Password: blank = Open; 4-96 characters = Protected"
    } else {
        "Game password (case sensitive)"
    };
    loop {
        terminal
            .draw(|frame| render_tournament_entry(frame, title, label, &"*".repeat(value.len())))?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !is_actionable_key(&key) {
            continue;
        }
        match key.code {
            KeyCode::Enter
                if (allow_empty && value.is_empty()) || (4..=96).contains(&value.len()) =>
            {
                return Ok(value)
            }
            KeyCode::Backspace => {
                value.pop();
            }
            KeyCode::Esc => return Err(EntryCancelled.into()),
            KeyCode::Char(c) if c.is_ascii() && !c.is_control() && value.len() < 96 => {
                value.push(c)
            }
            _ => {}
        }
    }
}

fn wait_for_game(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    lobby: LobbySession,
    table: &PublicTableSummary,
    password: &str,
) -> Result<
    (
        NetworkSession,
        terminal_poker::authorized_table::SubscriptionUpdate,
        terminal_poker::game::seat::SeatId,
    ),
    Box<dyn Error>,
> {
    let result =
        lobby.join_and_wait_while(table.table_id, None, Some(password.into()), |current| {
            let count = format!(
                "{}/{} players registered",
                current.occupied + current.reserved,
                current.seats.get()
            );
            let _ = show_notice(
                terminal,
                &current.name,
                &[
                    &count,
                    "The game starts when all players join.",
                    "Esc: Cancel registration",
                ],
            );
            if event::poll(Duration::from_millis(10)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    return !(is_actionable_key(&key) && key.code == KeyCode::Esc);
                }
            }
            true
        });
    match result {
        Err(NetworkSessionError::JoinCancelled) => Err(EntryCancelled.into()),
        result => result.map_err(Into::into),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_network_tournament(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    address: SocketAddr,
    display_name: &str,
    mut session: NetworkSession,
    initial: terminal_poker::authorized_table::SubscriptionUpdate,
    table_id: TableId,
    join_code: &str,
    mut tournament_state: TournamentPublicState,
    theme_mode: ThemeMode,
    color_depth: ColorDepth,
) -> Result<(), Box<dyn Error>> {
    let mut app = NetworkApp::new(
        ProjectionClient::bootstrap_from_update(initial)?,
        display_name.to_string(),
    );
    let mut terminal_since: Option<Instant> = None;
    let mut hand_started = Instant::now();
    let mut raise_sizing = None;
    let mut console_scroll = 0usize;
    let mut turn_attention = TurnAttention::default();
    loop {
        for message in session.poll()? {
            app.apply_message(message)?;
        }
        if app.is_terminal() {
            terminal_since.get_or_insert_with(Instant::now);
        } else {
            terminal_since = None;
        }
        let showdown = terminal_since.map(|started| ShowdownStage::after_reveal(started.elapsed()));
        let remaining = tournament_state
            .level_remaining_millis
            .saturating_sub(u64::try_from(hand_started.elapsed().as_millis()).unwrap_or(u64::MAX));
        let checkpoint = format!(
            "LEVEL {} · {}/{} A{} · {:02}:{:02}",
            tournament_state.level_number,
            tournament_state.level.small_blind,
            tournament_state.level.big_blind,
            tournament_state.level.ante,
            remaining / 60_000,
            (remaining / 1_000) % 60
        );
        let view = app.view(&checkpoint);
        turn_attention.update(
            &view,
            app.client().controls_enabled(),
            app.client().connection()
                == terminal_poker::network_client::ClientConnectionState::Connected,
        );
        raise_sizing = RaiseSizing::sync_from_view(&view, raise_sizing);
        terminal.draw(|frame| {
            render_practice_view_with_state(
                frame,
                &view,
                raise_sizing.map(Into::into),
                console_scroll,
                showdown,
            );
            let area = frame.area();
            apply_terminal_palette(frame.buffer_mut(), area, theme_mode, color_depth);
        })?;
        if app.is_terminal() {
            let completed_at = terminal_since.expect("terminal hand starts showdown timing");
            if completed_at.elapsed() >= terminal_hold(app.client().snapshot().snapshot.phase) {
                let token = session.reconnect_token();
                let _ = session.close();
                drop(session);
                match reconnect_tournament(address, display_name, token)? {
                    Some((next_session, update)) => {
                        session = next_session;
                        app = NetworkApp::new(
                            ProjectionClient::bootstrap_from_update(update)?,
                            display_name.to_string(),
                        );
                        tournament_state =
                            fetch_tournament_state(address, display_name, table_id, join_code)?;
                        hand_started = Instant::now();
                        terminal_since = None;
                        continue;
                    }
                    None => {
                        show_tournament_result(
                            terminal,
                            address,
                            display_name,
                            table_id,
                            join_code,
                        )?;
                        return Ok(());
                    }
                }
            }
        }
        if event::poll(Duration::from_millis(20))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if !is_actionable_key(&key) {
                continue;
            }
            if key.code == KeyCode::Char('q')
                || key.code == KeyCode::Esc
                || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
            {
                let _ = session.close();
                return Ok(());
            }
            if key.code == KeyCode::Char('h') {
                if let Ok(command) = app.prepare_showdown_preference(!view.always_show) {
                    session.send_command(command)?;
                }
                continue;
            }
            if let Some(control) = raise_control_for_key(key.code, raise_sizing) {
                match control {
                    RaiseControl::Increase => {
                        if let Some(sizing) = raise_sizing.as_mut() {
                            sizing.increase_one();
                        }
                    }
                    RaiseControl::Decrease => {
                        if let Some(sizing) = raise_sizing.as_mut() {
                            sizing.decrease_one();
                        }
                    }
                    RaiseControl::SelectPreset(preset) => {
                        if let Some(sizing) = raise_sizing.as_mut() {
                            sizing.select_preset(preset);
                        }
                    }
                    RaiseControl::Submit(action) => {
                        session.send_command(app.prepare_action(action)?)?;
                        raise_sizing = None;
                        console_scroll = 0;
                    }
                }
                continue;
            }
            match key.code {
                KeyCode::PageUp => {
                    console_scroll = console_scroll
                        .saturating_add(4)
                        .min(view.action_log.len().saturating_sub(1));
                    continue;
                }
                KeyCode::PageDown => {
                    console_scroll = console_scroll.saturating_sub(4);
                    continue;
                }
                _ => {}
            }
            if let Some(action) = network_action_for_key(key.code, &app) {
                session.send_command(app.prepare_action(action)?)?;
                raise_sizing = None;
                console_scroll = 0;
            }
        }
    }
}

fn fetch_tournament_state(
    address: SocketAddr,
    display_name: &str,
    table_id: TableId,
    join_code: &str,
) -> Result<TournamentPublicState, Box<dyn Error>> {
    let (mut lobby, _, _, _) = LobbySession::connect(address, format!("{display_name}-clock"))?;
    lobby
        .inspect_table_with_access(table_id, join_code.to_string())?
        .tournament
        .ok_or_else(|| "server omitted tournament state".into())
}

fn show_tournament_result(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    address: SocketAddr,
    display_name: &str,
    table_id: TableId,
    join_code: &str,
) -> Result<(), Box<dyn Error>> {
    let (mut lobby, _, _, _) = LobbySession::connect(address, format!("{display_name}-result"))?;
    let table = lobby.inspect_table_with_access(table_id, join_code.to_string())?;
    let tournament = table.tournament.ok_or("server omitted tournament result")?;
    let mut lines = vec![format!(
        "{} · {} hands · result pool {}",
        table.name, tournament.hands_completed, tournament.payout_pool
    )];
    lines.push(String::new());
    for standing in tournament.standings {
        lines.push(format!(
            "#{:<2}  Seat S{}  ·  payout {}",
            standing.place,
            standing.seat.as_u8(),
            standing.payout
        ));
    }
    lines.push(String::new());
    lines.push("Press any key to continue".to_string());
    terminal.draw(|frame| {
        render_tournament_result(frame, "TOURNAMENT COMPLETE", &lines);
    })?;
    let _ = event::read()?;
    Ok(())
}

fn reconnect_tournament(
    address: SocketAddr,
    display_name: &str,
    token: Option<terminal_poker::credentials::BearerToken>,
) -> Result<
    Option<(
        NetworkSession,
        terminal_poker::authorized_table::SubscriptionUpdate,
    )>,
    Box<dyn Error>,
> {
    let started = Instant::now();
    loop {
        let attempt = match token.clone() {
            Some(token) => NetworkSession::reconnect(address, display_name.to_string(), token),
            None => NetworkSession::connect(address, display_name.to_string()),
        };
        match attempt {
            Ok(connected) => return Ok(Some(connected)),
            Err(NetworkSessionError::Rejected { ref code, .. }) if code == "reconnect_rejected" => {
                return Ok(None);
            }
            Err(error) if started.elapsed() < Duration::from_secs(31 * 60) => {
                let _ = error;
                thread::sleep(Duration::from_millis(250));
            }
            Err(error) => return Err(Box::new(error)),
        }
    }
}

fn network_action_for_key(code: KeyCode, app: &NetworkApp) -> Option<Action> {
    if !app.client().controls_enabled() {
        return None;
    }
    let legal = app.client().snapshot().snapshot.legal_actions.as_ref()?;
    match code {
        KeyCode::Char('f') | KeyCode::Char('F') if legal.can_fold => Some(Action::Fold),
        KeyCode::Char('c') | KeyCode::Char('C') if legal.can_check => Some(Action::Check),
        KeyCode::Char('c') | KeyCode::Char('C') => legal.call_amount.map(Action::Call),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(Action::AllIn(legal.all_in_to)),
        _ => None,
    }
}

fn prompt_number(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    title: &str,
    label: &str,
    default: u32,
    minimum: u32,
    maximum: u32,
) -> Result<u32, Box<dyn Error>> {
    loop {
        let value = prompt_text(terminal, title, label, &default.to_string())?;
        if let Ok(parsed) = value.parse::<u32>() {
            if (minimum..=maximum).contains(&parsed) {
                return Ok(parsed);
            }
        }
        show_notice(
            terminal,
            "INVALID VALUE",
            &[
                &format!("Enter a number from {minimum} to {maximum}."),
                "Press any key.",
            ],
        )?;
        let _ = event::read()?;
    }
}

#[derive(Debug)]
struct EntryCancelled;
impl std::fmt::Display for EntryCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Entry cancelled")
    }
}
impl Error for EntryCancelled {}

fn prompt_text(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    title: &str,
    label: &str,
    default: &str,
) -> Result<String, Box<dyn Error>> {
    let mut value = default.to_string();
    loop {
        terminal.draw(|frame| {
            render_tournament_entry(frame, title, label, &value);
        })?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !is_actionable_key(&key) {
            continue;
        }
        match key.code {
            KeyCode::Enter if !value.trim().is_empty() => return Ok(value.trim().to_string()),
            KeyCode::Backspace => {
                value.pop();
            }
            KeyCode::Esc => return Err(EntryCancelled.into()),
            KeyCode::Char(character) if !character.is_control() && value.len() < 160 => {
                value.push(character);
            }
            _ => {}
        }
    }
}

fn show_notice(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    title: &str,
    lines: &[&str],
) -> Result<(), Box<dyn Error>> {
    let theme = SemanticTheme::resolve(ThemeMode::Ash, TerminalCapabilities::detect().color_depth);
    let footer = lines
        .last()
        .copied()
        .filter(|line| line.starts_with("Esc:"))
        .unwrap_or("");
    let content = if footer.is_empty() {
        lines
    } else {
        &lines[..lines.len() - 1]
    };
    terminal.draw(|frame| {
        terminal_poker::ui::game_lobby::render_lobby_message(
            frame,
            title,
            &content.join("\n"),
            footer,
            theme,
        );
    })?;
    Ok(())
}

fn run_quick_practice(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    session: &mut PracticeSession,
    theme_mode: ThemeMode,
    color_depth: ColorDepth,
) -> Result<(), Box<dyn Error>> {
    let mut next_bot_action = Instant::now() + Duration::from_millis(500);
    let mut terminal_since = None;
    let mut raise_sizing = None;
    let mut console_scroll = 0usize;
    let mut turn_attention = TurnAttention::default();
    loop {
        {
            let practice = session.current_mut();
            practice.apply_updates()?;
            if !practice.app().is_terminal()
                && Instant::now() >= next_bot_action
                && practice.step_bot()?
            {
                next_bot_action = Instant::now() + Duration::from_millis(350);
            }
        }
        let terminal_hand = session.current().app().is_terminal();
        if terminal_hand {
            let completed_at = terminal_since.get_or_insert_with(Instant::now);
            if completed_at.elapsed()
                >= terminal_hold(session.current().app().client().snapshot().snapshot.phase)
            {
                let mut terminal_view = session.view();
                let completed = session.complete_hand()?;
                if completed.can_continue {
                    terminal_since = None;
                    console_scroll = 0;
                    next_bot_action = Instant::now() + Duration::from_millis(650);
                    continue;
                }
                terminal_view.action_log = session.table_console().to_vec();
                return wait_at_completed_session(
                    terminal,
                    &terminal_view,
                    theme_mode,
                    color_depth,
                );
            }
        } else {
            terminal_since = None;
        }
        let showdown = terminal_since.map(|started| ShowdownStage::after_reveal(started.elapsed()));
        let view = session.view();
        turn_attention.update(
            &view,
            session.current().app().client().controls_enabled(),
            true,
        );
        raise_sizing = RaiseSizing::sync_from_view(&view, raise_sizing);
        terminal.draw(|frame| {
            render_practice_view_with_state(
                frame,
                &view,
                raise_sizing.map(Into::into),
                console_scroll,
                showdown,
            );
            let area = frame.area();
            apply_terminal_palette(frame.buffer_mut(), area, theme_mode, color_depth);
        })?;

        if !event::poll(Duration::from_millis(20))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !is_actionable_key(&key) {
            continue;
        }
        if key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            return Ok(());
        }
        if key.code == KeyCode::Esc {
            return Ok(());
        }
        if key.code == KeyCode::Char('h') {
            if view.showdown_progress.is_none()
                && !terminal_hand
                && session.current().app().client().pending().is_none()
            {
                session
                    .current_mut()
                    .set_showdown_preference(!view.always_show)?;
            }
            continue;
        }
        if let Some(control) = raise_control_for_key(key.code, raise_sizing) {
            match control {
                RaiseControl::Increase => {
                    if let Some(sizing) = raise_sizing.as_mut() {
                        sizing.increase_one();
                    }
                }
                RaiseControl::Decrease => {
                    if let Some(sizing) = raise_sizing.as_mut() {
                        sizing.decrease_one();
                    }
                }
                RaiseControl::SelectPreset(preset) => {
                    if let Some(sizing) = raise_sizing.as_mut() {
                        sizing.select_preset(preset);
                    }
                }
                RaiseControl::Submit(action) => {
                    session.current_mut().submit_local(action)?;
                    raise_sizing = None;
                    console_scroll = 0;
                    next_bot_action = Instant::now() + Duration::from_millis(350);
                }
            }
            continue;
        }
        match key.code {
            KeyCode::PageUp => {
                console_scroll = console_scroll
                    .saturating_add(4)
                    .min(view.action_log.len().saturating_sub(1));
                continue;
            }
            KeyCode::PageDown => {
                console_scroll = console_scroll.saturating_sub(4);
                continue;
            }
            KeyCode::Home => {
                console_scroll = view.action_log.len().saturating_sub(1);
                continue;
            }
            KeyCode::End => {
                console_scroll = 0;
                continue;
            }
            _ => {}
        }
        if let Some(action) = action_for_key(key.code, session.current()) {
            session.current_mut().submit_local(action)?;
            raise_sizing = None;
            console_scroll = 0;
            next_bot_action = Instant::now() + Duration::from_millis(350);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RaisePreset {
    QuarterPot,
    HalfPot,
    ThreeQuarterPot,
    Pot,
    OneAndHalfPot,
}

impl RaisePreset {
    const fn index(self) -> usize {
        match self {
            Self::QuarterPot => 0,
            Self::HalfPot => 1,
            Self::ThreeQuarterPot => 2,
            Self::Pot => 3,
            Self::OneAndHalfPot => 4,
        }
    }

    const fn fraction(self) -> (u32, u32) {
        match self {
            Self::QuarterPot => (1, 4),
            Self::HalfPot => (1, 2),
            Self::ThreeQuarterPot => (3, 4),
            Self::Pot => (1, 1),
            Self::OneAndHalfPot => (3, 2),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RaiseSizing {
    target: u32,
    minimum: u32,
    maximum: u32,
    is_bet: bool,
    context: RaiseContext,
    selected_preset: Option<RaisePreset>,
}

impl RaiseSizing {
    fn sync_from_view(view: &MultiwayReviewView, current: Option<Self>) -> Option<Self> {
        if view
            .client
            .as_ref()
            .is_some_and(|client| client.controls != "ENABLED")
        {
            return None;
        }
        let legal = view.legal_actions.as_ref()?;
        let (minimum, is_bet) = legal
            .min_raise_to
            .map(|minimum| (minimum, false))
            .or_else(|| legal.min_bet_to.map(|minimum| (minimum, true)))?;
        let maximum = legal.all_in_to.saturating_sub(1);
        let contribution = view
            .seats
            .iter()
            .find(|seat| seat.seat == view.local_seat)
            .map_or(0, |seat| seat.contribution);
        let context = RaiseContext {
            minimum,
            maximum,
            pot_total: view.pot_total,
            current_wager: view.current_wager,
            contribution,
            call_amount: legal.call_amount.unwrap_or(0),
            is_bet,
        };
        if minimum > maximum {
            return None;
        }
        if let Some(current) = current.filter(|current| current.context == context) {
            return Some(Self {
                target: current.target.clamp(minimum, maximum),
                ..current
            });
        }
        Some(Self {
            target: minimum,
            minimum,
            maximum,
            is_bet,
            context,
            selected_preset: None,
        })
    }

    fn increase_one(&mut self) {
        self.target = self.target.saturating_add(1).min(self.maximum);
        self.selected_preset = None;
    }

    fn decrease_one(&mut self) {
        self.target = self.target.saturating_sub(1).max(self.minimum);
        self.selected_preset = None;
    }

    fn select_preset(&mut self, preset: RaisePreset) {
        self.target = self.context.target_for(preset);
        self.selected_preset = Some(preset);
    }

    const fn action(self) -> Action {
        if self.is_bet {
            Action::Bet(self.target)
        } else {
            Action::Raise(self.target)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RaiseControl {
    Decrease,
    Increase,
    SelectPreset(RaisePreset),
    Submit(Action),
}

fn raise_control_for_key(code: KeyCode, sizing: Option<RaiseSizing>) -> Option<RaiseControl> {
    sizing.and_then(|sizing| match code {
        KeyCode::Up => Some(RaiseControl::Increase),
        KeyCode::Down => Some(RaiseControl::Decrease),
        KeyCode::Char('1') => Some(RaiseControl::SelectPreset(RaisePreset::QuarterPot)),
        KeyCode::Char('2') => Some(RaiseControl::SelectPreset(RaisePreset::HalfPot)),
        KeyCode::Char('3') => Some(RaiseControl::SelectPreset(RaisePreset::ThreeQuarterPot)),
        KeyCode::Char('4') => Some(RaiseControl::SelectPreset(RaisePreset::Pot)),
        KeyCode::Char('5') => Some(RaiseControl::SelectPreset(RaisePreset::OneAndHalfPot)),
        KeyCode::Char('r') | KeyCode::Char('R') => Some(RaiseControl::Submit(sizing.action())),
        _ => None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RaiseContext {
    minimum: u32,
    maximum: u32,
    pot_total: u32,
    current_wager: u32,
    contribution: u32,
    call_amount: u32,
    is_bet: bool,
}

impl RaiseContext {
    fn target_for(self, preset: RaisePreset) -> u32 {
        let (numerator, denominator) = preset.fraction();
        let base = if self.is_bet {
            self.pot_total
        } else {
            self.pot_total.saturating_add(self.call_amount)
        };
        let product = u64::from(base) * u64::from(numerator);
        let increment = u32::try_from(product.div_ceil(u64::from(denominator))).unwrap_or(u32::MAX);
        let desired = if self.is_bet {
            self.contribution.saturating_add(increment)
        } else {
            self.current_wager.saturating_add(increment)
        };
        desired.max(self.minimum).min(self.maximum)
    }
}

impl From<RaiseSizing> for RaiseSizingView {
    fn from(value: RaiseSizing) -> Self {
        Self {
            target: value.target,
            minimum: value.minimum,
            maximum: value.maximum,
            preset_index: value.selected_preset.map(RaisePreset::index),
        }
    }
}

fn wait_at_completed_session(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    view: &MultiwayReviewView,
    theme_mode: ThemeMode,
    color_depth: ColorDepth,
) -> Result<(), Box<dyn Error>> {
    let mut console_scroll = 0usize;
    loop {
        terminal.draw(|frame| {
            render_practice_view_with_state(
                frame,
                view,
                None,
                console_scroll,
                Some(ShowdownStage::Award),
            );
            let area = frame.area();
            apply_terminal_palette(frame.buffer_mut(), area, theme_mode, color_depth);
        })?;
        if !event::poll(Duration::from_millis(40))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !is_actionable_key(&key) {
            continue;
        }
        match key.code {
            KeyCode::PageUp | KeyCode::Home => {
                console_scroll = console_scroll
                    .saturating_add(4)
                    .min(view.action_log.len().saturating_sub(1));
            }
            KeyCode::PageDown => console_scroll = console_scroll.saturating_sub(4),
            KeyCode::End => console_scroll = 0,
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(()),
            _ => {}
        }
    }
}

fn is_quit_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('q')
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}

fn shell_event_for_key(
    key: &KeyEvent,
    route: ShellRoute,
    editing_name: bool,
) -> Option<ShellEvent> {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Some(ShellEvent::Quit);
    }
    if route == ShellRoute::Settings && editing_name {
        return match key.code {
            KeyCode::Enter => Some(ShellEvent::Select),
            KeyCode::Esc => Some(ShellEvent::Back),
            KeyCode::Backspace => Some(ShellEvent::Backspace),
            KeyCode::Char(character) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(ShellEvent::InputChar(character))
            }
            _ => None,
        };
    }
    if key.code == KeyCode::Char('q') {
        return Some(ShellEvent::Quit);
    }
    match key.code {
        KeyCode::Up => Some(ShellEvent::MoveUp),
        KeyCode::Down => Some(ShellEvent::MoveDown),
        KeyCode::Left => Some(ShellEvent::MoveLeft),
        KeyCode::Right => Some(ShellEvent::MoveRight),
        KeyCode::Enter => Some(ShellEvent::Select),
        KeyCode::Esc => Some(ShellEvent::Back),
        KeyCode::Char('?') | KeyCode::F(1) => Some(ShellEvent::OpenHelp),
        KeyCode::Char('s') | KeyCode::Char('S') => Some(ShellEvent::OpenSettings),
        _ => None,
    }
}

fn is_actionable_key(key: &KeyEvent) -> bool {
    matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

fn action_for_key(code: KeyCode, practice: &LocalPractice) -> Option<Action> {
    if !practice.app().client().controls_enabled() {
        return None;
    }
    let legal = practice
        .app()
        .client()
        .snapshot()
        .snapshot
        .legal_actions
        .as_ref()?;
    match code {
        KeyCode::Char('f') | KeyCode::Char('F') if legal.can_fold => Some(Action::Fold),
        KeyCode::Char('c') | KeyCode::Char('C') if legal.can_check => Some(Action::Check),
        KeyCode::Char('c') | KeyCode::Char('C') => legal.call_amount.map(Action::Call),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(Action::AllIn(legal.all_in_to)),
        _ => None,
    }
}

fn install_panic_restore_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        previous(info);
    }));
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    restored: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self, Box<dyn Error>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error.into());
        }
        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => Ok(Self {
                terminal,
                restored: false,
            }),
            Err(error) => {
                let _ = disable_raw_mode();
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                Err(error.into())
            }
        }
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }

    fn restore(&mut self) -> Result<(), Box<dyn Error>> {
        if self.restored {
            return Ok(());
        }
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        self.restored = true;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if self.restored {
            return;
        }
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
        self.restored = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_conpty_release_event_cannot_select_a_menu_item() {
        let release =
            KeyEvent::new_with_kind(KeyCode::Enter, KeyModifiers::NONE, KeyEventKind::Release);

        assert!(!is_actionable_key(&release));
    }

    #[test]
    fn press_and_repeat_events_remain_actionable() {
        let press = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        let repeat =
            KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Repeat);

        assert!(is_actionable_key(&press));
        assert!(is_actionable_key(&repeat));
    }

    #[test]
    fn global_keys_map_to_the_shell_reducer() {
        let help = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        let settings = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
        let back = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let quit = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);

        assert_eq!(
            shell_event_for_key(&help, ShellRoute::Home, false),
            Some(ShellEvent::OpenHelp)
        );
        assert_eq!(
            shell_event_for_key(&settings, ShellRoute::Home, false),
            Some(ShellEvent::OpenSettings)
        );
        assert_eq!(
            shell_event_for_key(&back, ShellRoute::Home, false),
            Some(ShellEvent::Back)
        );
        assert_eq!(
            shell_event_for_key(&quit, ShellRoute::Home, false),
            Some(ShellEvent::Quit)
        );
    }

    #[test]
    fn settings_name_editor_accepts_reserved_printable_keys_without_quitting() {
        let q = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        let question = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
        assert_eq!(
            shell_event_for_key(&q, ShellRoute::Settings, true),
            Some(ShellEvent::InputChar('q'))
        );
        assert_eq!(
            shell_event_for_key(&question, ShellRoute::Settings, true),
            Some(ShellEvent::InputChar('?'))
        );
    }

    #[test]
    fn bet_and_raise_presets_use_pot_sizing_and_clamp_to_legal_bounds() {
        let raise = RaiseContext {
            minimum: 20,
            maximum: 70,
            pot_total: 40,
            current_wager: 10,
            contribution: 2,
            call_amount: 8,
            is_bet: false,
        };

        assert_eq!(raise.target_for(RaisePreset::QuarterPot), 22);
        assert_eq!(raise.target_for(RaisePreset::HalfPot), 34);
        assert_eq!(raise.target_for(RaisePreset::ThreeQuarterPot), 46);
        assert_eq!(raise.target_for(RaisePreset::Pot), 58);
        assert_eq!(raise.target_for(RaisePreset::OneAndHalfPot), 70);

        let bet = RaiseContext {
            minimum: 12,
            maximum: 99,
            pot_total: 40,
            current_wager: 0,
            contribution: 0,
            call_amount: 0,
            is_bet: true,
        };
        assert_eq!(bet.target_for(RaisePreset::QuarterPot), 12);
        assert_eq!(bet.target_for(RaisePreset::HalfPot), 20);
        assert_eq!(bet.target_for(RaisePreset::ThreeQuarterPot), 30);
        assert_eq!(bet.target_for(RaisePreset::Pot), 40);
        assert_eq!(bet.target_for(RaisePreset::OneAndHalfPot), 60);
    }

    #[test]
    fn number_hotkeys_select_all_five_pot_presets() {
        let sizing = test_raise_sizing();
        let expected = [
            RaisePreset::QuarterPot,
            RaisePreset::HalfPot,
            RaisePreset::ThreeQuarterPot,
            RaisePreset::Pot,
            RaisePreset::OneAndHalfPot,
        ];
        for (key, preset) in ['1', '2', '3', '4', '5'].into_iter().zip(expected) {
            assert_eq!(
                raise_control_for_key(KeyCode::Char(key), Some(sizing)),
                Some(RaiseControl::SelectPreset(preset))
            );
        }
    }

    #[test]
    fn arrows_adjust_chips_and_r_submits_without_a_confirmation_mode() {
        let sizing = test_raise_sizing();

        assert_eq!(
            raise_control_for_key(KeyCode::Up, Some(sizing)),
            Some(RaiseControl::Increase)
        );
        assert_eq!(
            raise_control_for_key(KeyCode::Down, Some(sizing)),
            Some(RaiseControl::Decrease)
        );
        assert_eq!(
            raise_control_for_key(KeyCode::Char('r'), Some(sizing)),
            Some(RaiseControl::Submit(Action::Raise(34)))
        );
        assert_eq!(raise_control_for_key(KeyCode::Enter, Some(sizing)), None);
    }

    #[test]
    fn chip_adjustment_is_one_chip_clamped_and_clears_the_preset_highlight() {
        let mut sizing = test_raise_sizing();
        sizing.increase_one();
        assert_eq!(sizing.target, 35);
        assert_eq!(sizing.selected_preset, None);

        sizing.decrease_one();
        assert_eq!(sizing.target, 34);
        sizing.target = sizing.maximum;
        sizing.increase_one();
        assert_eq!(sizing.target, sizing.maximum);
        sizing.target = sizing.minimum;
        sizing.decrease_one();
        assert_eq!(sizing.target, sizing.minimum);

        sizing.select_preset(RaisePreset::ThreeQuarterPot);
        assert_eq!(sizing.target, 46);
        assert_eq!(sizing.selected_preset, Some(RaisePreset::ThreeQuarterPot));
    }

    fn test_raise_sizing() -> RaiseSizing {
        let context = RaiseContext {
            minimum: 20,
            maximum: 99,
            pot_total: 40,
            current_wager: 10,
            contribution: 2,
            call_amount: 8,
            is_bet: false,
        };
        RaiseSizing {
            target: 34,
            minimum: context.minimum,
            maximum: context.maximum,
            is_bet: context.is_bet,
            context,
            selected_preset: Some(RaisePreset::HalfPot),
        }
    }
}
