use std::{
    env, fs,
    path::{Path, PathBuf},
};

use ratatui::{backend::TestBackend, Terminal};
use serde::Serialize;
use terminal_poker::{
    game::multiway::MultiwayPhase,
    local_practice::PracticeSession,
    network_client::passive_action,
    ui::{multiway_review::MultiwayReviewView, render::render_practice_view},
};

const BUILD_ID: &str = "terminal-poker-v1.0.1-sprint16-portrait-v1";
const TRAJECTORY_ID: &str = "S16-PORTRAIT-001";
const REVIEW_SEED: u64 = 16_001;

#[derive(Serialize)]
struct Cell {
    symbol: String,
    foreground: String,
    background: String,
    modifiers: u16,
}

#[derive(Serialize)]
struct Capture {
    renderer: &'static str,
    backend: &'static str,
    build_id: &'static str,
    trajectory_id: &'static str,
    seed: u64,
    checkpoint: String,
    phase: String,
    trajectory: bool,
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

#[derive(Serialize)]
struct Frame {
    checkpoint: String,
    phase: String,
    board: Vec<String>,
    pot: u32,
    stacks: Vec<(u8, u32, u32)>,
    visible_private_seats: Vec<u8>,
    conserved_total: u32,
}

#[derive(Serialize)]
struct Evidence {
    schema: &'static str,
    build_id: &'static str,
    trajectory_id: &'static str,
    seed: u64,
    supported_viewports: Vec<(u16, u16)>,
    frames: Vec<Frame>,
    privacy_pass: bool,
    chip_conservation_pass: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = output_dir()?;
    fs::create_dir_all(&output)?;
    let mut session = PracticeSession::nine_handed_seeded_for_review(100, REVIEW_SEED)?;
    let mut initial = review_view(session.current().view(), "viewport");
    for (width, height) in [(80, 30), (72, 32), (64, 36), (56, 40), (120, 40)] {
        initial.checkpoint = format!("viewport-{width}x{height}");
        capture(
            &output,
            &format!("viewport-{width}x{height}"),
            &initial,
            width,
            height,
            false,
        )?;
    }

    let mut frames = Vec::new();
    let mut captured = Vec::new();
    record(
        &output,
        "01-deal",
        session.current().view(),
        &mut frames,
        &mut captured,
    )?;
    for _ in 0..512 {
        session.current_mut().apply_updates()?;
        if session.current().app().is_terminal() {
            break;
        }
        if session.current().app().client().controls_enabled() {
            let legal = session
                .current()
                .app()
                .client()
                .snapshot()
                .snapshot
                .legal_actions
                .as_ref()
                .ok_or("controls enabled without legal actions")?
                .clone();
            session.current_mut().submit_local(passive_action(&legal))?;
        } else {
            session.current_mut().step_bot()?;
        }
        session.current_mut().apply_updates()?;
        let phase = session.view().phase;
        let checkpoint = match phase {
            MultiwayPhase::Preflop => Some("02-preflop"),
            MultiwayPhase::Flop => Some("03-flop"),
            MultiwayPhase::Turn => Some("04-turn"),
            MultiwayPhase::River => Some("05-river"),
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete => Some("06-showdown-award"),
        };
        if let Some(checkpoint) = checkpoint {
            let already_captured = frames.iter().any(|frame| frame.checkpoint == checkpoint);
            if !already_captured && (phase == MultiwayPhase::Preflop || !captured.contains(&phase))
            {
                record(
                    &output,
                    checkpoint,
                    session.current().view(),
                    &mut frames,
                    &mut captured,
                )?;
            }
        }
    }
    if !session.current().app().is_terminal() {
        return Err("Sprint 16 review hand did not terminate".into());
    }
    for required in [
        MultiwayPhase::Preflop,
        MultiwayPhase::Flop,
        MultiwayPhase::Turn,
        MultiwayPhase::River,
        MultiwayPhase::Showdown,
    ] {
        if !captured.contains(&required) {
            return Err(format!("trajectory omitted {required:?}").into());
        }
    }
    let privacy_pass = frames
        .iter()
        .filter(|frame| !matches!(frame.phase.as_str(), "Showdown" | "Complete"))
        .all(|frame| frame.visible_private_seats == vec![0]);
    let chip_conservation_pass = frames.iter().all(|frame| frame.conserved_total == 900);
    if !privacy_pass || !chip_conservation_pass {
        return Err("trajectory privacy or conservation failed".into());
    }
    fs::write(
        output.join("review-evidence.json"),
        serde_json::to_vec_pretty(&Evidence {
            schema: "terminal-poker-sprint16-review-v1",
            build_id: BUILD_ID,
            trajectory_id: TRAJECTORY_ID,
            seed: REVIEW_SEED,
            supported_viewports: vec![(80, 30), (72, 32), (64, 36), (56, 40), (120, 40)],
            frames,
            privacy_pass,
            chip_conservation_pass,
        })?,
    )?;
    println!("SPRINT16_REVIEW_PASS {TRAJECTORY_ID}");
    Ok(())
}

fn record(
    output: &Path,
    checkpoint: &str,
    view: MultiwayReviewView,
    frames: &mut Vec<Frame>,
    captured: &mut Vec<MultiwayPhase>,
) -> Result<(), Box<dyn std::error::Error>> {
    let view = review_view(view, checkpoint);
    capture(output, checkpoint, &view, 80, 30, true)?;
    let terminal = matches!(
        view.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    );
    frames.push(Frame {
        checkpoint: checkpoint.to_string(),
        phase: view.phase.name().to_string(),
        board: view.board.iter().map(|card| format!("{card:?}")).collect(),
        pot: view.pot_total,
        stacks: view
            .seats
            .iter()
            .map(|seat| (seat.seat.as_u8(), seat.stack, seat.contribution))
            .collect(),
        visible_private_seats: view
            .seats
            .iter()
            .filter(|seat| seat.cards_visible)
            .map(|seat| seat.seat.as_u8())
            .collect(),
        conserved_total: view.seats.iter().map(|seat| seat.stack).sum::<u32>()
            + if terminal { 0 } else { view.pot_total },
    });
    if !captured.contains(&view.phase) {
        captured.push(view.phase);
    }
    Ok(())
}

fn review_view(mut view: MultiwayReviewView, checkpoint: &str) -> MultiwayReviewView {
    view.build_id = BUILD_ID.to_string();
    view.hand_id = TRAJECTORY_ID.to_string();
    view.seed = REVIEW_SEED;
    view.checkpoint = checkpoint.to_string();
    view
}

fn capture(
    output: &Path,
    checkpoint: &str,
    view: &MultiwayReviewView,
    width: u16,
    height: u16,
    trajectory: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render_practice_view(frame, view))?;
    let buffer = terminal.backend().buffer();
    let capture = Capture {
        renderer: "terminal_poker::ui::render::render_practice_view",
        backend: "ratatui::backend::TestBackend",
        build_id: BUILD_ID,
        trajectory_id: TRAJECTORY_ID,
        seed: REVIEW_SEED,
        checkpoint: checkpoint.to_string(),
        phase: view.phase.name().to_string(),
        trajectory,
        width,
        height,
        cells: buffer
            .content
            .iter()
            .map(|cell| Cell {
                symbol: cell.symbol().to_string(),
                foreground: format!("{:?}", cell.fg),
                background: format!("{:?}", cell.bg),
                modifiers: cell.modifier.bits(),
            })
            .collect(),
    };
    fs::write(
        output.join(format!("{checkpoint}.json")),
        serde_json::to_vec_pretty(&capture)?,
    )?;
    Ok(())
}

fn output_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--output-dir")) {
        return Err("usage: review-sprint16 --output-dir PATH".into());
    }
    let path = args.next().ok_or("missing output directory")?;
    if args.next().is_some() {
        return Err("unexpected review arguments".into());
    }
    Ok(PathBuf::from(path))
}
