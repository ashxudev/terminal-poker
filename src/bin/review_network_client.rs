use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::network_client_review::{
    build_network_client_review, ClientReviewCheckpoint, BUILD_ID, HAND_LABEL, REVIEW_SEED,
};
use terminal_poker::ui::multiway_review::MultiwayReviewView;
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
    table_id: u64,
    protocol_hand_id: u64,
    revision: u64,
    stream_sequence: u64,
    connection: String,
    pending_command: Option<String>,
    command_id: String,
    outcome: String,
    checkpoint: String,
    trajectory: bool,
    width: u16,
    height: u16,
    cells: Vec<RatatuiCell>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = parse_output_dir()?;
    let capture_dir = output_dir.join("ratatui");
    fs::create_dir_all(&capture_dir)?;

    let review = build_network_client_review();
    fs::write(
        output_dir.join("evidence.json"),
        serde_json::to_string_pretty(&review.evidence)?,
    )?;
    for checkpoint in &review.checkpoints {
        write_capture(&capture_dir, checkpoint)?;
    }

    println!(
        "Network client review evidence generated: build={} hand={} trajectory_frames={} total_captures={} campaign_occupancies={}",
        review.evidence.build_id,
        review.evidence.hand_id,
        review.evidence.frames.len(),
        review.checkpoints.len(),
        review.evidence.campaign.len()
    );
    println!("Output: {}", output_dir.display());
    Ok(())
}

fn write_capture(
    capture_dir: &Path,
    checkpoint: &ClientReviewCheckpoint,
) -> Result<(), Box<dyn std::error::Error>> {
    let view = MultiwayReviewView::from_network_client(
        &checkpoint.client,
        BUILD_ID,
        HAND_LABEL,
        REVIEW_SEED,
        &checkpoint.screenshot_stem,
        &checkpoint.command_id,
        &checkpoint.outcome,
        checkpoint.action_log.clone(),
    );
    let backend = TestBackend::new(checkpoint.viewport[0], checkpoint.viewport[1]);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render::render_network_view(frame, &view))?;
    let buffer = terminal.backend().buffer();
    let capture = RatatuiCapture {
        renderer: "terminal_poker::ui::render::render_network_view",
        backend: "ratatui::backend::TestBackend",
        build_id: BUILD_ID,
        hand_id: HAND_LABEL,
        seed: REVIEW_SEED,
        table_id: checkpoint.client.snapshot().table_id.0,
        protocol_hand_id: checkpoint.client.snapshot().hand_id.0,
        revision: checkpoint.client.snapshot().revision,
        stream_sequence: checkpoint.client.last_stream_sequence(),
        connection: checkpoint.client.connection().label().to_string(),
        pending_command: checkpoint
            .client
            .pending()
            .map(|pending| pending.command_id.clone()),
        command_id: checkpoint.command_id.clone(),
        outcome: checkpoint.outcome.clone(),
        checkpoint: checkpoint.screenshot_stem.clone(),
        trajectory: checkpoint.trajectory,
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
        _ => Err("usage: review-network-client --output-dir <directory>".to_string()),
    }
}
