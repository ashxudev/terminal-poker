//! Reproducible production-renderer captures of public hand-participation states.
use std::{env, fs, path::PathBuf};

use ratatui::{backend::TestBackend, Terminal};
use serde_json::json;
use terminal_poker::{local_practice::LocalPractice, ui::ash_table::render_with_state};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = PathBuf::from(env::args_os().nth(1).ok_or("expected output directory")?);
    fs::create_dir_all(&output)?;
    let practice = LocalPractice::nine_handed_seeded_for_review(100, 14_001)?;
    for mixed in [false, true] {
        let mut view = practice.view();
        if mixed {
            // Presentation fixtures change only public state; no private cards are added.
            for seat in &mut view.seats {
                match seat.seat.as_u8() {
                    1 | 5 => {
                        seat.folded = true;
                        seat.status = "FOLDED".into();
                    }
                    2 => seat.status = "NOTDEALT".into(),
                    3 => seat.status = "ALLIN".into(),
                    _ => {}
                }
            }
            view.seats.retain(|seat| seat.seat.as_u8() != 8);
        }
        for (width, height) in [(80, 30), (72, 32), (64, 36), (56, 40), (120, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| render_with_state(frame, &view, 0, None, None))?;
            let checkpoint = format!("{}-{width}x{height}", if mixed { "mixed" } else { "dealt" });
            let buffer = terminal.backend().buffer();
            let cells: Vec<_> = buffer
                .content
                .iter()
                .map(|cell| {
                    json!({
                        "symbol": cell.symbol(), "foreground": format!("{:?}", cell.fg),
                        "background": format!("{:?}", cell.bg), "modifiers": cell.modifier.bits(),
                    })
                })
                .collect();
            fs::write(
                output.join(format!("{checkpoint}.json")),
                serde_json::to_vec_pretty(&json!({
                    "renderer": "terminal_poker::ui::ash_table::render_with_state",
                    "backend": "ratatui::backend::TestBackend", "checkpoint": checkpoint,
                    "width": width, "height": height, "cells": cells,
                    "fixture": "public participation states; authorized hero only",
                }))?,
            )?;
        }
    }
    println!("CARD_PRESENCE_CAPTURES_PASS");
    Ok(())
}
