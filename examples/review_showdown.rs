//! Capture the timed presentation stages of one completed authoritative hand.
use ratatui::{backend::TestBackend, Terminal};
use serde_json::json;
use std::{env, fs, path::PathBuf};
use terminal_poker::{
    game::{
        actions::Action,
        command::SeatCommand,
        multiway::{MultiwayHand, MultiwayPhase},
        seat::{SeatId, TableSize},
    },
    ui::{
        ash_table::render_with_state,
        multiway_review::{MultiwayReviewView, ShowdownStage},
    },
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(env::args_os().nth(1).ok_or("expected output directory")?);
    fs::create_dir_all(&output)?;
    let s = |i| SeatId::new(i).unwrap();
    let mut hand = MultiwayHand::new_seeded_for_review(
        TableSize::new(3)?,
        s(0),
        &[(s(0), 100), (s(1), 100), (s(2), 100)],
        31_415,
    )?;
    while !matches!(
        hand.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    ) {
        let actor = hand.to_act.ok_or("missing actor")?;
        let legal = hand
            .legal_actions_for(actor)
            .ok_or("missing legal actions")?;
        let action = if legal.can_check {
            Action::Check
        } else {
            Action::Call(legal.call_amount.ok_or("missing call")?)
        };
        hand.apply_command(SeatCommand::new(actor, action))?;
    }
    let view = MultiwayReviewView::from_hand(
        &hand,
        "showdown-1500ms-green-brackets",
        "showdown-001",
        31_415,
        "showdown",
        s(0),
        Vec::new(),
    );
    for (label, stage) in [
        ("01-reveal", ShowdownStage::Reveal),
        ("02-winners", ShowdownStage::Winners),
        ("03-award", ShowdownStage::Award),
    ] {
        for (width, height) in [(120, 40), (56, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| render_with_state(frame, &view, 0, None, Some(stage)))?;
            let cells: Vec<_> = terminal.backend().buffer().content.iter().map(|cell| json!({
                "symbol":cell.symbol(),"foreground":format!("{:?}",cell.fg),"background":format!("{:?}",cell.bg),"modifiers":cell.modifier.bits(),
            })).collect();
            let checkpoint = format!("{label}-{width}x{height}");
            fs::write(
                output.join(format!("{checkpoint}.json")),
                serde_json::to_vec_pretty(&json!({
                    "renderer":"terminal_poker::ui::ash_table::render_with_state", "backend":"ratatui::backend::TestBackend", "checkpoint":checkpoint,
                    "width":width,"height":height,"cells":cells,
                }))?,
            )?;
        }
    }
    println!("SHOWDOWN_CAPTURES_PASS");
    Ok(())
}
