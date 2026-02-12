mod bot;
mod game;
mod stats;
mod ui;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use game::state::GamePhase;
use stats::persistence::StatsStore;
use ui::app::App;

#[derive(Parser, Debug)]
#[command(name = "terminal-poker")]
#[command(about = "A heads-up No-Limit Texas Hold'em training tool")]
#[command(version)]
struct Args {
    /// Starting stack size in big blinds
    #[arg(long, default_value = "100")]
    stack: u32,

    /// Bot aggression level (0.0 = passive, 1.0 = aggressive)
    #[arg(long, default_value = "0.5")]
    aggression: f64,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Set up Ctrl+C handler for graceful exit
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc_handler(r);

    // Set up panic hook to restore terminal state on panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        original_hook(panic_info);
    }));

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load or create stats store
    let mut stats_store = StatsStore::load_or_create();

    // Create app state
    let mut app = App::new(args.stack, args.aggression);
    app.initialize(&mut stats_store);

    // Main game loop
    let result = run_game_loop(&mut terminal, &mut app, &mut stats_store, &running);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // Save stats on exit
    stats_store.save();

    result
}

fn ctrlc_handler(running: Arc<AtomicBool>) {
    if let Err(e) = ctrlc::set_handler(move || {
        running.store(false, Ordering::SeqCst);
    }) {
        eprintln!("Warning: Could not set Ctrl+C handler: {}", e);
    }
}

fn run_game_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    stats_store: &mut StatsStore,
    running: &Arc<AtomicBool>,
) -> io::Result<()> {
    while running.load(Ordering::SeqCst) {
        app.tick_count = app.tick_count.wrapping_add(1);

        // Draw UI
        terminal.draw(|f| ui::render::render(f, app))?;

        // Process pending game events (timed)
        app.process_next_event(stats_store);

        // Handle input (50ms poll for responsive event processing)
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                match app.game_state.phase {
                    GamePhase::Showdown => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                stats_store.record_session_end();
                                stats_store.record_profit(
                                    (app.game_state.session_profit_bb() * 2.0).round() as i64,
                                );
                                app.game_state.phase = GamePhase::Summary;
                            }
                            _ => {
                                app.continue_after_showdown(stats_store);
                            }
                        }
                    }
                    GamePhase::Summary | GamePhase::SessionEnd => match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => break,
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            if matches!(app.game_state.phase, GamePhase::SessionEnd) {
                                app.new_session(stats_store);
                            }
                        }
                        _ => {
                            if matches!(app.game_state.phase, GamePhase::Summary) {
                                break;
                            }
                        }
                    },
                    _ => {
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && key.code == KeyCode::Char('c')
                        {
                            break;
                        }

                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => {
                                stats_store.record_session_end();
                                stats_store.record_profit(
                                    (app.game_state.session_profit_bb() * 2.0).round() as i64,
                                );
                                app.game_state.phase = GamePhase::Summary;
                            }
                            KeyCode::Char('?') => {
                                app.toggle_help();
                            }
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                app.toggle_stats();
                            }
                            _ => {
                                // Block gameplay input while events are pending or overlays are open
                                if !app.has_pending_events() && !app.show_help && !app.show_stats {
                                    if let Some(action) = ui::input::handle_key(
                                        key,
                                        &app.game_state,
                                        &mut app.raise_input,
                                        &mut app.raise_mode,
                                    ) {
                                        app.apply_player_action(action, stats_store);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check for session end (player busted)
        if app.game_state.phase != GamePhase::SessionEnd
            && app.game_state.phase != GamePhase::Summary
        {
            if app.game_state.player_stack == 0 || app.game_state.bot_stack == 0 {
                app.game_state.phase = GamePhase::SessionEnd;
            }
        }
    }

    Ok(())
}
