use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::authorized_table_review::{
    build_authorized_table_review, AuthorizedReviewCheckpoint, BUILD_ID, HAND_ID,
    REVIEW_PROTOCOL_HAND_ID, REVIEW_SEED, REVIEW_TABLE_ID,
};
use terminal_poker::ui::app::App;
use terminal_poker::ui::multiway_review::{MultiwayReviewView, ProtocolReviewMetadata};
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
    table_id: u64,
    protocol_hand_id: u64,
    revision: u64,
    stream_sequence: u64,
    now_tick: u64,
    deadline_seat: Option<u8>,
    deadline_due_tick: Option<u64>,
    audience: String,
    command_id: String,
    outcome: String,
    checkpoint: String,
    width: u16,
    height: u16,
    cells: Vec<RatatuiCell>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = parse_output_dir()?;
    let capture_dir = output_dir.join("ratatui");
    fs::create_dir_all(&capture_dir)?;

    let review = build_authorized_table_review();
    fs::write(
        output_dir.join("evidence.json"),
        serde_json::to_string_pretty(&review.evidence)?,
    )?;
    for checkpoint in &review.checkpoints {
        write_capture(&capture_dir, checkpoint)?;
    }

    println!(
        "Authorized table review evidence generated: build={} hand={} frames={} timeout_ticks={}",
        review.evidence.build_id,
        review.evidence.hand_id,
        review.evidence.frames.len(),
        review.evidence.action_timeout_ticks
    );
    println!("Output: {}", output_dir.display());
    Ok(())
}

fn write_capture(
    capture_dir: &Path,
    checkpoint: &AuthorizedReviewCheckpoint,
) -> Result<(), Box<dyn std::error::Error>> {
    let audience = match checkpoint.snapshot.snapshot.audience {
        terminal_poker::protocol::ProjectionKind::Player { seat } => {
            format!("PLAYER S{}", seat.as_u8())
        }
        terminal_poker::protocol::ProjectionKind::Spectator => "SPECTATOR".to_string(),
    };
    let metadata = ProtocolReviewMetadata {
        version: checkpoint.snapshot.version,
        table_id: checkpoint.snapshot.table_id.0,
        hand_id: checkpoint.snapshot.hand_id.0,
        revision: checkpoint.snapshot.revision,
        audience: audience.clone(),
        command_id: checkpoint.command_id.clone(),
        outcome: checkpoint.outcome.clone(),
    };
    let mut action_log = checkpoint.action_log.clone();
    action_log.push(format!(
        "RUNTIME  stream {} / tick {} / bindings {} connected {} / auth denied {}",
        checkpoint.metrics.stream_sequence,
        checkpoint.metrics.now_tick,
        checkpoint.metrics.active_bindings,
        checkpoint.metrics.connected_bindings,
        checkpoint.metrics.authorization_rejections
    ));
    action_log.push(format!(
        "DELIVERY  sent {} / dropped {} / warnings {} / timeouts {} / actor accepted {}",
        checkpoint.metrics.subscription_deliveries,
        checkpoint.metrics.slow_subscribers_dropped,
        checkpoint.metrics.deadline_warnings,
        checkpoint.metrics.timeout_actions,
        checkpoint.metrics.actor.accepted_commands
    ));
    if let Some(deadline) = checkpoint.deadline {
        action_log.push(format!(
            "DEADLINE  S{} / warn {} / due {} / generation {}",
            deadline.seat.as_u8(),
            deadline.warning_tick,
            deadline.due_tick,
            deadline.generation
        ));
    } else {
        action_log.push("DEADLINE  none / hand terminal".to_string());
    }
    action_log.push(format!("CHECKPOINT  {}", checkpoint.event));

    let mut app = App::new(100, 0.5);
    app.multiway_review = Some(MultiwayReviewView::from_projection(
        &checkpoint.snapshot,
        BUILD_ID,
        HAND_ID,
        REVIEW_SEED,
        &checkpoint.screenshot_stem,
        metadata,
        action_log,
    ));

    let backend = TestBackend::new(CAPTURE_WIDTH, CAPTURE_HEIGHT);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render::render(frame, &app))?;
    let buffer = terminal.backend().buffer();
    let capture = RatatuiCapture {
        renderer: "terminal_poker::ui::render::render",
        backend: "ratatui::backend::TestBackend",
        build_id: BUILD_ID,
        hand_id: HAND_ID,
        seed: REVIEW_SEED,
        table_id: REVIEW_TABLE_ID.0,
        protocol_hand_id: REVIEW_PROTOCOL_HAND_ID.0,
        revision: checkpoint.snapshot.revision,
        stream_sequence: checkpoint.metrics.stream_sequence,
        now_tick: checkpoint.metrics.now_tick,
        deadline_seat: checkpoint.deadline.map(|deadline| deadline.seat.as_u8()),
        deadline_due_tick: checkpoint.deadline.map(|deadline| deadline.due_tick),
        audience,
        command_id: checkpoint.command_id.clone(),
        outcome: checkpoint.outcome.clone(),
        checkpoint: checkpoint.screenshot_stem.clone(),
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
        _ => Err("usage: review-authorized-table --output-dir <directory>".to_string()),
    }
}
