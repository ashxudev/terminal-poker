use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde::Serialize;
use terminal_poker::game::actions::Action;
use terminal_poker::game::command::SeatCommand;
use terminal_poker::game::multiway::{MultiwayHand, MultiwayPhase};
use terminal_poker::game::seat::{SeatId, TableSize};
use terminal_poker::local_practice::{LocalPractice, PracticeSession};
use terminal_poker::local_profile::{LocalProfile, ProfileTheme};
use terminal_poker::network_client::passive_action;
use terminal_poker::ui::multiway_review::{MultiwayReviewView, ShowdownStage};
use terminal_poker::ui::platform::{apply_terminal_palette, ColorDepth, SemanticTheme, ThemeMode};
use terminal_poker::ui::render::{
    render_practice_view, render_practice_view_with_raise, render_practice_view_with_state,
    RaiseSizingView,
};
use terminal_poker::ui::shell::{
    render_shell, ShellApp, ShellEvent, STANDARD_HEIGHT, STANDARD_WIDTH,
};

const BUILD_ID: &str = "terminal-poker-v1.0.1-sprint14-px1";
const TRAJECTORY_ID: &str = "S14-QUICK-001";
const REVIEW_SEED: u64 = 14_001;
const STARTING_STACK: u32 = 100;

#[derive(Serialize)]
struct RatatuiCell {
    symbol: String,
    foreground: String,
    background: String,
    modifiers: u16,
}

#[derive(Serialize)]
struct RatatuiCapture {
    renderer: String,
    backend: &'static str,
    build_id: &'static str,
    trajectory_id: &'static str,
    seed: u64,
    checkpoint: String,
    phase: String,
    trajectory: bool,
    width: u16,
    height: u16,
    cells: Vec<RatatuiCell>,
}

#[derive(Debug, Serialize)]
struct ContinuityFrame {
    checkpoint: String,
    phase: String,
    revision: u64,
    board: Vec<String>,
    pot: u32,
    s0_stack: u32,
    s0_contribution: u32,
    eligible_seats: Vec<u8>,
    visible_private_seats: Vec<u8>,
    conserved_total: u32,
}

#[derive(Serialize)]
struct ReviewEvidence {
    schema: &'static str,
    build_id: &'static str,
    trajectory_id: &'static str,
    review_seed: u64,
    table_id: u64,
    hand_id: u64,
    human_seat: u8,
    bot_seats: Vec<u8>,
    frames: Vec<ContinuityFrame>,
    final_stack: u32,
    final_awards: usize,
    safe_history_actions: usize,
    terminal_total: u32,
    privacy_pass: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = parse_output_dir()?;
    let table_dir = output_dir.join("ratatui");
    let shell_dir = output_dir.join("shell");
    let showdown_dir = output_dir.join("showdown");
    fs::create_dir_all(&table_dir)?;
    fs::create_dir_all(&shell_dir)?;
    fs::create_dir_all(&showdown_dir)?;

    let mut session = PracticeSession::nine_handed_seeded_for_review(STARTING_STACK, REVIEW_SEED)?;
    let mut frames = Vec::new();
    let mut captured_phases = Vec::new();
    let mut captured_raise_sizing = false;

    capture_practice(
        &table_dir,
        session.current(),
        "01-deal",
        &mut frames,
        &mut captured_phases,
        true,
    )?;
    let mut captured_preflop_action = false;
    for _ in 0..256 {
        session.current_mut().apply_updates()?;
        if session.current().app().is_terminal() {
            break;
        }
        let local_acted = if session.current().app().client().controls_enabled() {
            let legal = session
                .current()
                .app()
                .client()
                .snapshot()
                .snapshot
                .legal_actions
                .as_ref()
                .ok_or("review player has controls but no legal actions")?
                .clone();
            if !captured_raise_sizing {
                let minimum = legal
                    .min_raise_to
                    .or(legal.min_bet_to)
                    .ok_or("review player cannot demonstrate raise sizing")?;
                let maximum = legal.all_in_to.saturating_sub(1);
                let mut view = session.view();
                let contribution = view
                    .seats
                    .iter()
                    .find(|seat| seat.seat == view.local_seat)
                    .map_or(0, |seat| seat.contribution);
                let target = if legal.min_bet_to.is_some() {
                    contribution.saturating_add(view.pot_total.div_ceil(2))
                } else {
                    view.current_wager.saturating_add(
                        view.pot_total
                            .saturating_add(legal.call_amount.unwrap_or(0))
                            .div_ceil(2),
                    )
                }
                .max(minimum)
                .min(maximum);
                view.build_id = BUILD_ID.to_string();
                view.hand_id = TRAJECTORY_ID.to_string();
                view.seed = REVIEW_SEED;
                view.checkpoint = "raise-sizing".to_string();
                write_raise_capture(
                    shell_dir.join("raise-sizing.json"),
                    &view,
                    RaiseSizingView {
                        target,
                        minimum,
                        maximum,
                        preset_index: Some(1),
                    },
                )?;
                captured_raise_sizing = true;
            }
            session.current_mut().submit_local(passive_action(&legal))?;
            true
        } else {
            session.current_mut().step_bot()?;
            false
        };
        session.current_mut().apply_updates()?;
        let phase = session.view().phase;
        if local_acted && phase == MultiwayPhase::Preflop && !captured_preflop_action {
            capture_practice(
                &table_dir,
                session.current(),
                "02-preflop",
                &mut frames,
                &mut captured_phases,
                false,
            )?;
            captured_preflop_action = true;
        }
        let checkpoint = match phase {
            MultiwayPhase::Flop => Some("03-flop"),
            MultiwayPhase::Turn => Some("04-turn"),
            MultiwayPhase::River => Some("05-river"),
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete => Some("06-showdown-award"),
            MultiwayPhase::Preflop => None,
        };
        if let Some(checkpoint) = checkpoint {
            if !captured_phases.contains(&phase) {
                capture_practice(
                    &table_dir,
                    session.current(),
                    checkpoint,
                    &mut frames,
                    &mut captured_phases,
                    true,
                )?;
            }
        }
    }
    if !session.current().app().is_terminal() {
        return Err("Sprint 14 review hand did not terminate".into());
    }
    if !captured_raise_sizing {
        return Err("Sprint 14 review omitted raise sizing".into());
    }
    let mut terminal_view = session.view();
    terminal_view.build_id = BUILD_ID.to_string();
    terminal_view.hand_id = TRAJECTORY_ID.to_string();
    terminal_view.seed = REVIEW_SEED;
    for (name, stage) in [
        ("01-reveal", ShowdownStage::Reveal),
        ("02-winners", ShowdownStage::Winners),
        ("03-award", ShowdownStage::Award),
    ] {
        write_showdown_capture(
            showdown_dir.join(format!("{name}.json")),
            &terminal_view,
            name,
            stage,
            STANDARD_WIDTH,
            STANDARD_HEIGHT,
        )?;
    }
    let folded_view = folded_hand_view()?;
    write_showdown_capture(
        showdown_dir.join("00-folded-demarcation.json"),
        &folded_view,
        "00-folded-demarcation",
        ShowdownStage::Reveal,
        STANDARD_WIDTH,
        STANDARD_HEIGHT,
    )?;
    for (name, stage) in [
        ("04-narrow-winners", ShowdownStage::Winners),
        ("05-narrow-award", ShowdownStage::Award),
    ] {
        write_showdown_capture(
            showdown_dir.join(format!("{name}.json")),
            &terminal_view,
            name,
            stage,
            56,
            40,
        )?;
    }
    for required in [
        MultiwayPhase::Preflop,
        MultiwayPhase::Flop,
        MultiwayPhase::Turn,
        MultiwayPhase::River,
        MultiwayPhase::Showdown,
    ] {
        if !captured_phases.contains(&required) {
            return Err(format!("review trajectory omitted {required:?}").into());
        }
    }

    let history = session.current().safe_history()?;
    let final_stack = history
        .final_stacks
        .iter()
        .find_map(|(seat, stack)| (seat.as_u8() == 0).then_some(*stack))
        .ok_or("review history omitted S0")?;
    let terminal_total = history.final_stacks.iter().map(|(_, stack)| stack).sum();
    let privacy_pass = frames
        .iter()
        .filter(|frame| !matches!(frame.phase.as_str(), "Showdown" | "Complete"))
        .all(|frame| frame.visible_private_seats == vec![0]);
    if terminal_total != 900 || !privacy_pass {
        return Err("review trajectory failed conservation or privacy".into());
    }

    let summary = session.complete_hand()?;
    if !summary.can_continue || session.current().hand_id().0 != 2 {
        return Err("review session did not roll automatically into hand 2".into());
    }
    let mut next_hand_view = session.view();
    next_hand_view.build_id = BUILD_ID.to_string();
    next_hand_view.hand_id = TRAJECTORY_ID.to_string();
    next_hand_view.seed = REVIEW_SEED;
    next_hand_view.checkpoint = "07-next-hand-console".to_string();
    write_table_capture(table_dir.join("07-next-hand-console.json"), &next_hand_view)?;
    write_shell_captures(&shell_dir)?;
    let evidence = ReviewEvidence {
        schema: "terminal-poker-sprint14-review-v1",
        build_id: BUILD_ID,
        trajectory_id: TRAJECTORY_ID,
        review_seed: REVIEW_SEED,
        table_id: history.table_id.0,
        hand_id: history.hand_id.0,
        human_seat: 0,
        bot_seats: (1..9).collect(),
        frames,
        final_stack,
        final_awards: history.awards.len(),
        safe_history_actions: history.actions.len(),
        terminal_total,
        privacy_pass,
    };
    fs::write(
        output_dir.join("review-evidence.json"),
        serde_json::to_string_pretty(&evidence)?,
    )?;
    println!(
        "SPRINT14_REVIEW_PASS trajectory={} frames={} final_stack={} total={} privacy={}",
        TRAJECTORY_ID,
        evidence.frames.len(),
        evidence.final_stack,
        evidence.terminal_total,
        evidence.privacy_pass
    );
    Ok(())
}

fn write_showdown_capture(
    path: PathBuf,
    view: &MultiwayReviewView,
    checkpoint: &str,
    stage: ShowdownStage,
    width: u16,
    height: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    write_capture(
        path,
        "terminal_poker::ui::render::render_practice_view_with_state",
        checkpoint.to_string(),
        view.phase.name().to_string(),
        false,
        width,
        height,
        |terminal| {
            terminal.draw(|frame| {
                render_practice_view_with_state(frame, view, None, 0, Some(stage));
                let area = frame.area();
                apply_terminal_palette(
                    frame.buffer_mut(),
                    area,
                    ThemeMode::Ash,
                    ColorDepth::TrueColor,
                );
            })?;
            Ok(())
        },
    )
}

fn folded_hand_view() -> Result<MultiwayReviewView, Box<dyn std::error::Error>> {
    let table_size = TableSize::new(3)?;
    let local_seat = SeatId::new(0)?;
    let mut hand = MultiwayHand::new_seeded_for_review(
        table_size,
        local_seat,
        &[
            (local_seat, STARTING_STACK),
            (SeatId::new(1)?, STARTING_STACK),
            (SeatId::new(2)?, STARTING_STACK),
        ],
        REVIEW_SEED + 1,
    )?;
    while !matches!(
        hand.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        hand.apply_command(SeatCommand::new(
            hand.to_act.ok_or("fold fixture omitted actor")?,
            Action::Fold,
        ))?;
    }
    Ok(MultiwayReviewView::from_hand(
        &hand,
        BUILD_ID,
        "S14-FOLD-001",
        REVIEW_SEED + 1,
        "00-folded-demarcation",
        local_seat,
        Vec::new(),
    ))
}

fn write_raise_capture(
    path: PathBuf,
    view: &MultiwayReviewView,
    sizing: RaiseSizingView,
) -> Result<(), Box<dyn std::error::Error>> {
    write_capture(
        path,
        "terminal_poker::ui::render::render_practice_view_with_raise",
        "raise-sizing".to_string(),
        view.phase.name().to_string(),
        false,
        STANDARD_WIDTH,
        STANDARD_HEIGHT,
        |terminal| {
            terminal.draw(|frame| {
                render_practice_view_with_raise(frame, view, Some(sizing));
                let area = frame.area();
                apply_terminal_palette(
                    frame.buffer_mut(),
                    area,
                    ThemeMode::Ash,
                    ColorDepth::TrueColor,
                );
            })?;
            Ok(())
        },
    )
}

fn capture_practice(
    output_dir: &Path,
    practice: &LocalPractice,
    checkpoint: &str,
    frames: &mut Vec<ContinuityFrame>,
    captured_phases: &mut Vec<MultiwayPhase>,
    mark_phase: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut view = practice.view();
    view.build_id = BUILD_ID.to_string();
    view.hand_id = TRAJECTORY_ID.to_string();
    view.seed = REVIEW_SEED;
    view.checkpoint = checkpoint.to_string();
    write_table_capture(output_dir.join(format!("{checkpoint}.json")), &view)?;

    let s0 = view
        .seats
        .iter()
        .find(|seat| seat.seat.as_u8() == 0)
        .ok_or("review view omitted S0")?;
    let terminal = matches!(
        view.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    );
    let conserved_total = view.seats.iter().map(|seat| seat.stack).sum::<u32>()
        + if terminal { 0 } else { view.pot_total };
    if conserved_total != 900 {
        return Err(format!("{checkpoint} failed chip conservation").into());
    }
    frames.push(ContinuityFrame {
        checkpoint: checkpoint.to_string(),
        phase: view.phase.name().to_string(),
        revision: view.protocol.as_ref().map_or(0, |item| item.revision),
        board: view.board.iter().map(|card| format!("{card:?}")).collect(),
        pot: view.pot_total,
        s0_stack: s0.stack,
        s0_contribution: s0.contribution,
        eligible_seats: view
            .seats
            .iter()
            .filter(|seat| matches!(seat.status.as_str(), "LIVE" | "ALLIN"))
            .map(|seat| seat.seat.as_u8())
            .collect(),
        visible_private_seats: view
            .seats
            .iter()
            .filter(|seat| seat.cards_visible)
            .map(|seat| seat.seat.as_u8())
            .collect(),
        conserved_total,
    });
    if mark_phase {
        captured_phases.push(view.phase);
    }
    Ok(())
}

fn write_table_capture(
    path: PathBuf,
    view: &MultiwayReviewView,
) -> Result<(), Box<dyn std::error::Error>> {
    write_capture(
        path,
        "terminal_poker::ui::render::render_practice_view",
        view.checkpoint.clone(),
        view.phase.name().to_string(),
        true,
        STANDARD_WIDTH,
        STANDARD_HEIGHT,
        |terminal| {
            terminal.draw(|frame| {
                render_practice_view(frame, view);
                let area = frame.area();
                apply_terminal_palette(
                    frame.buffer_mut(),
                    area,
                    ThemeMode::Ash,
                    ColorDepth::TrueColor,
                );
            })?;
            Ok(())
        },
    )
}

fn write_shell_captures(output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let profile = LocalProfile {
        display_name: "Ada".to_string(),
        theme: ProfileTheme::Ash,
        reduced_motion: true,
        quick_starting_stack: STARTING_STACK,
        ..LocalProfile::default()
    };
    let cases = [
        (
            "home-standard",
            ShellEvent::Back,
            120,
            40,
            ColorDepth::TrueColor,
        ),
        (
            "home-compact",
            ShellEvent::Back,
            80,
            24,
            ColorDepth::TrueColor,
        ),
        (
            "home-unsupported",
            ShellEvent::Back,
            79,
            23,
            ColorDepth::TrueColor,
        ),
        (
            "settings",
            ShellEvent::OpenSettings,
            120,
            40,
            ColorDepth::TrueColor,
        ),
        (
            "help-basic",
            ShellEvent::OpenHelp,
            120,
            40,
            ColorDepth::Basic,
        ),
    ];
    for (checkpoint, event, width, height, depth) in cases {
        let mut app = ShellApp::new(profile.clone());
        app.handle(event);
        let theme = SemanticTheme::resolve(app.profile().theme_mode(), depth);
        write_capture(
            output_dir.join(format!("{checkpoint}.json")),
            "terminal_poker::ui::shell::render_shell",
            checkpoint.to_string(),
            "Shell".to_string(),
            false,
            width,
            height,
            |terminal| {
                terminal.draw(|frame| {
                    render_shell(
                        frame,
                        &app,
                        "<platform>/sneakyblinders/profile.json",
                        &theme,
                    );
                    let area = frame.area();
                    apply_terminal_palette(
                        frame.buffer_mut(),
                        area,
                        app.profile().theme_mode(),
                        depth,
                    );
                })?;
                Ok(())
            },
        )?;
    }

    let mut error = ShellApp::new(profile.clone());
    error.handle(ShellEvent::Failure(
        "Could not save settings; existing source preserved".to_string(),
    ));
    let error_theme = SemanticTheme::resolve(error.profile().theme_mode(), ColorDepth::TrueColor);
    write_capture(
        output_dir.join("recoverable-error.json"),
        "terminal_poker::ui::shell::render_shell",
        "recoverable-error".to_string(),
        "Error".to_string(),
        false,
        STANDARD_WIDTH,
        STANDARD_HEIGHT,
        |terminal| {
            terminal.draw(|frame| {
                render_shell(
                    frame,
                    &error,
                    "<platform>/sneakyblinders/profile.json",
                    &error_theme,
                );
            })?;
            Ok(())
        },
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn write_capture<F>(
    path: PathBuf,
    renderer: &str,
    checkpoint: String,
    phase: String,
    trajectory: bool,
    width: u16,
    height: u16,
    render: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(&mut Terminal<TestBackend>) -> Result<(), Box<dyn std::error::Error>>,
{
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend)?;
    render(&mut terminal)?;
    let buffer = terminal.backend().buffer();
    let capture = RatatuiCapture {
        renderer: renderer.to_string(),
        backend: "ratatui::backend::TestBackend",
        build_id: BUILD_ID,
        trajectory_id: TRAJECTORY_ID,
        seed: REVIEW_SEED,
        checkpoint,
        phase,
        trajectory,
        width,
        height,
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
    fs::write(path, serde_json::to_string(&capture)?)?;
    Ok(())
}

fn parse_output_dir() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output-dir"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: review-sprint14 --output-dir <directory>".to_string()),
    }
}
