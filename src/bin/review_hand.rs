use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::game::actions::Action;
use terminal_poker::game::command::SeatCommand;
use terminal_poker::game::review::{
    run_deterministic_review_hand, BUILD_ID, FIXTURE_ID, HAND_ID, REVIEW_SEED,
};
use terminal_poker::game::state::{GamePhase, GameState};
use terminal_poker::ui::app::{ActionLogEntry, App, BOT_SEAT, LOCAL_SEAT};
use terminal_poker::ui::render;

const CAPTURE_WIDTH: u16 = 120;
const CAPTURE_HEIGHT: u16 = 40;

#[derive(Serialize)]
struct RatatuiCell {
    symbol: String,
    foreground: String,
    background: String,
    modifiers: u16,
}

#[derive(Serialize)]
struct RatatuiCapture {
    renderer: &'static str,
    backend: &'static str,
    build_id: &'static str,
    fixture_id: &'static str,
    hand_id: &'static str,
    seed: u64,
    checkpoint: String,
    width: u16,
    height: u16,
    cells: Vec<RatatuiCell>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = parse_output_dir()?;
    fs::create_dir_all(&output_dir)?;

    let evidence = run_deterministic_review_hand();
    fs::write(
        output_dir.join("00-command-rejection.txt"),
        format!("{}\n", evidence.rejection_view),
    )?;
    for frame in &evidence.frames {
        fs::write(
            output_dir.join(format!("{}.txt", frame.screenshot_stem)),
            format!("{}\n", frame.terminal_view),
        )?;
    }
    fs::write(
        output_dir.join("evidence.json"),
        serde_json::to_string_pretty(&evidence)?,
    )?;
    write_ratatui_captures(&output_dir)?;

    println!(
        "Review evidence generated: build={} fixture={} hand={} seed={} frames={} rejection_unchanged={}",
        evidence.build_id,
        evidence.fixture_id,
        evidence.hand_id,
        evidence.seed,
        evidence.frames.len(),
        evidence.rejected_state_unchanged
    );
    println!("Output: {}", output_dir.display());
    Ok(())
}

fn write_ratatui_captures(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let capture_dir = output_dir.join("ratatui");
    fs::create_dir_all(&capture_dir)?;

    let mut app = App::new(100, 0.5);
    app.game_state = GameState::new_seeded_for_review(100, REVIEW_SEED);
    app.visible_player_bet = app.game_state.street_bet(LOCAL_SEAT);
    app.visible_bot_bet = app.game_state.street_bet(BOT_SEAT);
    app.action_log = vec![
        log("Pre-Flop", "You post SB (0.5BB)"),
        log("Pre-Flop", "Opp post BB (1BB)"),
    ];

    let rejected_signature = state_signature(&app.game_state);
    let error = app
        .game_state
        .apply_command(SeatCommand::new(BOT_SEAT, Action::Check))
        .expect_err("the deterministic fixture starts with the local seat to act");
    assert_eq!(rejected_signature, state_signature(&app.game_state));
    app.action_log.push(log(
        "Rejected",
        &format!("Opp Check - {error}; no mutation PASS"),
    ));
    write_capture(&capture_dir, "00-command-rejection", &app)?;

    app.action_log.pop();
    write_capture(&capture_dir, "01-preflop", &app)?;

    while matches!(
        app.game_state.phase,
        GamePhase::Preflop | GamePhase::Flop | GamePhase::Turn | GamePhase::River
    ) {
        let actor = app.game_state.to_act;
        let to_call = app.game_state.amount_to_call(actor);
        let action = if to_call > 0 {
            Action::Call(to_call)
        } else {
            Action::Check
        };
        let prior_phase = app.game_state.phase;
        let street = phase_name(prior_phase);
        let actor_name = if actor == LOCAL_SEAT { "You" } else { "Opp" };

        if actor == LOCAL_SEAT {
            app.player_last_action = Some(action);
        } else {
            app.bot_last_action = Some(action);
        }
        app.game_state
            .apply_command(SeatCommand::new(actor, action))
            .expect("the review trajectory only submits legal passive actions");
        app.action_log.push(log(
            street,
            &format!("{actor_name} {}", action_description(action)),
        ));

        if app.game_state.phase != prior_phase {
            app.visible_board_len = app.game_state.board.len();
            app.visible_player_bet = 0;
            app.visible_bot_bet = 0;
            app.player_last_action = None;
            app.bot_last_action = None;
            app.bot_thinking =
                app.game_state.phase != GamePhase::Showdown && app.game_state.to_act == BOT_SEAT;
            app.thinking_start_tick = 0;
            app.tick_count = 4;

            if app.game_state.phase == GamePhase::Showdown {
                app.showdown_revealed = true;
                app.showdown_result_shown = true;
                app.bot_thinking = false;
            }

            let stem = match app.game_state.phase {
                GamePhase::Flop => "02-flop",
                GamePhase::Turn => "03-turn",
                GamePhase::River => "04-river",
                GamePhase::Showdown => "05-showdown",
                _ => unreachable!("the passive fixture advances one street at a time"),
            };
            write_capture(&capture_dir, stem, &app)?;
        }
    }

    Ok(())
}

fn write_capture(
    capture_dir: &Path,
    checkpoint: &str,
    app: &App,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = TestBackend::new(CAPTURE_WIDTH, CAPTURE_HEIGHT);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render::render(frame, app))?;
    let buffer = terminal.backend().buffer();
    let capture = RatatuiCapture {
        renderer: "terminal_poker::ui::render::render",
        backend: "ratatui::backend::TestBackend",
        build_id: BUILD_ID,
        fixture_id: FIXTURE_ID,
        hand_id: HAND_ID,
        seed: REVIEW_SEED,
        checkpoint: checkpoint.to_string(),
        width: buffer.area.width,
        height: buffer.area.height,
        cells: buffer
            .content
            .iter()
            .map(|cell| RatatuiCell {
                symbol: cell.symbol().to_string(),
                foreground: format!("{:?}", cell.fg),
                background: format!("{:?}", cell.bg),
                modifiers: cell.modifier.bits(),
            })
            .collect(),
    };
    fs::write(
        capture_dir.join(format!("{checkpoint}.json")),
        serde_json::to_string(&capture)?,
    )?;
    Ok(())
}

fn log(street: &str, text: &str) -> ActionLogEntry {
    ActionLogEntry {
        street: street.to_string(),
        text: text.to_string(),
    }
}

fn phase_name(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::Preflop => "Pre-Flop",
        GamePhase::Flop => "Flop",
        GamePhase::Turn => "Turn",
        GamePhase::River => "River",
        _ => "",
    }
}

fn action_description(action: Action) -> String {
    match action {
        Action::Check => "check".to_string(),
        Action::Call(amount) => format!("call {amount}"),
        Action::Fold => "fold".to_string(),
        Action::Bet(amount) => format!("bet to {amount}"),
        Action::Raise(amount) => format!("raise to {amount}"),
        Action::AllIn(amount) => format!("all-in to {amount}"),
    }
}

fn state_signature(state: &GameState) -> String {
    format!(
        "{:?}|{}|{}|{}|{}|{}",
        state.phase,
        state.pot,
        state.to_act.as_u8(),
        state.stack(LOCAL_SEAT),
        state.stack(BOT_SEAT),
        state.board.len()
    )
}

fn parse_output_dir() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output-dir"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: review-hand --output-dir <directory>".to_string()),
    }
}
