use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::game::lifecycle_review::{
    build_lifecycle_review, LifecycleReviewCheckpoint, BUILD_ID, HAND_ID, REVIEW_SEED,
};
use terminal_poker::game::seat::SeatId;
use terminal_poker::ui::app::App;
use terminal_poker::ui::multiway_review::{LifecycleReviewStatus, MultiwayReviewView};
use terminal_poker::ui::render;

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
    hand_id: &'static str,
    seed: u64,
    checkpoint: String,
    table_state: String,
    hand_active: bool,
    occupied: usize,
    eligible: usize,
    reservations: usize,
    pending: usize,
    boundary: String,
    width: u16,
    height: u16,
    cells: Vec<RatatuiCell>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = parse_output_dir()?;
    let capture_dir = output_dir.join("ratatui");
    fs::create_dir_all(&capture_dir)?;
    let (evidence, checkpoints) = build_lifecycle_review();
    fs::write(
        output_dir.join("evidence.json"),
        serde_json::to_string_pretty(&evidence)?,
    )?;
    for checkpoint in &checkpoints {
        write_capture(&capture_dir, checkpoint)?;
    }
    println!(
        "Lifecycle review generated: build={} hand={} captures={}",
        BUILD_ID,
        HAND_ID,
        checkpoints.len()
    );
    println!("Output: {}", output_dir.display());
    Ok(())
}

fn write_capture(
    capture_dir: &Path,
    checkpoint: &LifecycleReviewCheckpoint,
) -> Result<(), Box<dyn std::error::Error>> {
    let lifecycle = &checkpoint.lifecycle;
    let mut view = MultiwayReviewView::from_hand(
        &checkpoint.hand,
        BUILD_ID,
        HAND_ID,
        REVIEW_SEED,
        &checkpoint.screenshot_stem,
        SeatId::new(0).unwrap(),
        checkpoint.action_log.clone(),
    );
    view.lifecycle = Some(LifecycleReviewStatus {
        state: format!("{:?}", lifecycle.state()).to_uppercase(),
        hand_active: lifecycle.hand_active(),
        occupied: lifecycle.seats().occupied_count(),
        eligible: lifecycle.eligible_count(),
        reservations: lifecycle.reservations().count(),
        pending: lifecycle.pending().count(),
        boundary: checkpoint.boundary.clone(),
    });
    let mut app = App::new(100, 0.5);
    app.multiway_review = Some(view);
    let backend = TestBackend::new(132, 50);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render::render(frame, &app))?;
    let buffer = terminal.backend().buffer();
    let capture = RatatuiCapture {
        renderer: "terminal_poker::ui::render::render",
        backend: "ratatui::backend::TestBackend",
        build_id: BUILD_ID,
        hand_id: HAND_ID,
        seed: REVIEW_SEED,
        checkpoint: checkpoint.screenshot_stem.clone(),
        table_state: format!("{:?}", lifecycle.state()),
        hand_active: lifecycle.hand_active(),
        occupied: lifecycle.seats().occupied_count(),
        eligible: lifecycle.eligible_count(),
        reservations: lifecycle.reservations().count(),
        pending: lifecycle.pending().count(),
        boundary: checkpoint.boundary.clone(),
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
        capture_dir.join(format!("{}.json", checkpoint.screenshot_stem)),
        serde_json::to_string(&capture)?,
    )?;
    Ok(())
}

fn parse_output_dir() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output-dir"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: review-table-lifecycle --output-dir <directory>".to_string()),
    }
}
