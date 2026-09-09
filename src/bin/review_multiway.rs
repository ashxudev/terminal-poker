use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::game::multiway_review::{
    action_log_for_review, build_review_checkpoints, run_multiway_review_hand, BUILD_ID, HAND_ID,
    REVIEW_SEED,
};
use terminal_poker::game::seat::SeatId;
use terminal_poker::ui::app::App;
use terminal_poker::ui::multiway_review::MultiwayReviewView;
use terminal_poker::ui::render;

const CAPTURE_WIDTH: u16 = 140;
const CAPTURE_HEIGHT: u16 = 45;

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
    width: u16,
    height: u16,
    cells: Vec<RatatuiCell>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = parse_output_dir()?;
    let capture_dir = output_dir.join("ratatui");
    fs::create_dir_all(&capture_dir)?;

    let evidence = run_multiway_review_hand();
    fs::write(
        output_dir.join("evidence.json"),
        serde_json::to_string_pretty(&evidence)?,
    )?;

    for checkpoint in build_review_checkpoints() {
        let mut app = App::new(100, 0.5);
        app.multiway_review = Some(MultiwayReviewView::from_hand(
            &checkpoint.hand,
            BUILD_ID,
            HAND_ID,
            REVIEW_SEED,
            &checkpoint.screenshot_stem,
            SeatId::new(0).expect("review local seat is valid"),
            action_log_for_review(&checkpoint.hand, &checkpoint.event),
        ));
        write_capture(&capture_dir, &checkpoint.screenshot_stem, &app)?;
    }

    println!(
        "Multiway review evidence generated: build={} hand={} seed={} frames={}",
        evidence.build_id,
        evidence.hand_id,
        evidence.seed,
        evidence.frames.len()
    );
    println!("Output: {}", output_dir.display());
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

fn parse_output_dir() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output-dir"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: review-multiway --output-dir <directory>".to_string()),
    }
}
