use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::game::command::SeatCommand;
use terminal_poker::game::multiway::{BlindValues, MultiwayHand, MultiwayPhase};
use terminal_poker::game::seat::{SeatId, TableSize};
use terminal_poker::network_client::passive_action;
use terminal_poker::ui::multiway_review::MultiwayReviewView;
use terminal_poker::ui::platform::{apply_terminal_palette, ColorDepth, SemanticTheme, ThemeMode};
use terminal_poker::ui::render::render_practice_view;
use terminal_poker::ui::shell::{
    render_shell, render_tournament_entry, render_tournament_result, ShellApp, ShellEvent,
};

const BUILD_ID: &str = "terminal-poker-v1.0.1-sprint15-d1";
const TRAJECTORY_ID: &str = "S15-TOURNAMENT-001";
const REVIEW_SEED: u64 = 15_001;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = output_dir()?;
    fs::create_dir_all(&output)?;
    capture_home(&output)?;
    capture_entry(&output)?;

    let size = TableSize::new(6)?;
    let stacks = size.seats().map(|seat| (seat, 3_000)).collect::<Vec<_>>();
    let blinds = BlindValues::new(25, 50, 5).expect("review level is valid");
    let mut hand = MultiwayHand::new_seeded_with_blinds(
        size,
        SeatId::new(0)?,
        &stacks,
        &[],
        blinds,
        REVIEW_SEED,
    )?;
    capture_hand(&output, &hand, "01-deal", true)?;
    let mut last_phase = hand.phase;
    let mut captured_preflop = false;
    for _ in 0..256 {
        if matches!(
            hand.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            break;
        }
        let seat = hand.to_act.ok_or("active review hand has no actor")?;
        let legal = hand
            .legal_actions_for(seat)
            .ok_or("acting review seat has no legal actions")?;
        hand.apply_command(SeatCommand::new(seat, passive_action(&legal)))?;
        if !captured_preflop && hand.phase == MultiwayPhase::Preflop {
            capture_hand(&output, &hand, "02-preflop", true)?;
            captured_preflop = true;
        }
        if hand.phase != last_phase {
            last_phase = hand.phase;
            let checkpoint = match hand.phase {
                MultiwayPhase::Flop => Some("03-flop"),
                MultiwayPhase::Turn => Some("04-turn"),
                MultiwayPhase::River => Some("05-river"),
                MultiwayPhase::Showdown | MultiwayPhase::HandComplete => Some("06-showdown"),
                MultiwayPhase::Preflop => None,
            };
            if let Some(checkpoint) = checkpoint {
                capture_hand(&output, &hand, checkpoint, true)?;
            }
        }
    }
    if !matches!(
        hand.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        return Err("review hand did not terminate".into());
    }
    capture_result(&output, &hand)?;
    Ok(())
}

fn capture_home(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = ShellApp::default();
    app.handle(ShellEvent::MoveDown);
    capture(
        output,
        "00-home-host",
        "HOME",
        false,
        "terminal_poker::ui::shell::render_shell",
        |frame| {
            let theme = SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor);
            render_shell(frame, &app, "profile.json", &theme);
            let area = frame.area();
            apply_terminal_palette(
                frame.buffer_mut(),
                area,
                ThemeMode::Ash,
                ColorDepth::TrueColor,
            );
        },
    )
}

fn capture_entry(output: &Path) -> Result<(), Box<dyn std::error::Error>> {
    capture(
        output,
        "00-host-setup",
        "SETUP",
        false,
        "terminal_poker::ui::shell::render_tournament_entry",
        |frame| {
            render_tournament_entry(
                frame,
                "HOST TOURNAMENT · 2 OF 11",
                "Starting stack (100-1000000)",
                "3000",
            );
        },
    )
}

fn capture_hand(
    output: &Path,
    hand: &MultiwayHand,
    checkpoint: &str,
    trajectory: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let view = MultiwayReviewView::from_hand(
        hand,
        BUILD_ID,
        "table-1-hand-1",
        REVIEW_SEED,
        "LEVEL 1 · 25/50 A5 · 09:42",
        SeatId::new(0)?,
        vec![
            "Dealer · Tournament started · 6 registered".to_string(),
            "Dealer · Level 1 · blinds 25/50 · ante 5".to_string(),
            "Dealer · Private freezeout · play money".to_string(),
        ],
    );
    let phase = hand.phase.name().to_string();
    capture(
        output,
        checkpoint,
        &phase,
        trajectory,
        "terminal_poker::ui::render::render_practice_view",
        |frame| {
            render_practice_view(frame, &view);
        },
    )
}

fn capture_result(output: &Path, hand: &MultiwayHand) -> Result<(), Box<dyn std::error::Error>> {
    let winner = hand
        .awards
        .iter()
        .flat_map(|award| award.payouts.iter())
        .max_by_key(|payout| payout.amount)
        .map_or(0, |payout| payout.seat.as_u8());
    let lines = vec![
        "Sneaky Freezeout · 18 hands · result pool 1000".to_string(),
        String::new(),
        format!("#1   Seat S{winner}  ·  payout 1000"),
        "#2   Seat S4  ·  payout 0".to_string(),
        "#3   Seat S2  ·  payout 0".to_string(),
        String::new(),
        "Press any key to return Home".to_string(),
    ];
    capture(
        output,
        "07-winner",
        "COMPLETE",
        false,
        "terminal_poker::ui::shell::render_tournament_result",
        |frame| {
            render_tournament_result(frame, "TOURNAMENT COMPLETE", &lines);
        },
    )
}

fn capture<F>(
    output: &Path,
    checkpoint: &str,
    phase: &str,
    trajectory: bool,
    renderer: &'static str,
    render: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&mut ratatui::Frame<'_>),
{
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(render)?;
    let buffer = terminal.backend().buffer();
    let capture = Capture {
        renderer,
        backend: "ratatui::backend::TestBackend",
        build_id: BUILD_ID,
        trajectory_id: TRAJECTORY_ID,
        seed: REVIEW_SEED,
        checkpoint: checkpoint.to_string(),
        phase: phase.to_string(),
        trajectory,
        width: buffer.area.width,
        height: buffer.area.height,
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
        return Err("usage: review-sprint15 --output-dir PATH".into());
    }
    let path = args.next().ok_or("missing output directory")?;
    if args.next().is_some() {
        return Err("unexpected review arguments".into());
    }
    Ok(PathBuf::from(path))
}
