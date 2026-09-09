use std::{error::Error, fs, path::PathBuf};

use clap::Parser;
use ratatui::{backend::TestBackend, Terminal};
use serde::Serialize;
use terminal_poker::ui::concept::{
    render_ash_continuity_mockup, render_concept, ConceptScreen, CONCEPT_HEIGHT, CONCEPT_WIDTH,
};
use terminal_poker::ui::shell::render_home;

#[derive(Debug, Parser)]
#[command(about = "Capture deterministic Sneaky Blinders Ratatui concept screens")]
struct Args {
    #[arg(long, default_value = "assets/concepts/captures")]
    output: PathBuf,

    #[arg(long, default_value = "assets/concepts/v2/captures")]
    v2_output: PathBuf,

    #[arg(long, default_value = "assets/concepts/production/captures")]
    production_output: PathBuf,
}

#[derive(Serialize)]
struct CaptureCell {
    symbol: String,
    foreground: String,
    background: String,
    modifiers: u16,
}

#[derive(Serialize)]
struct ConceptCapture {
    renderer: &'static str,
    backend: &'static str,
    fixture_version: u8,
    screen: &'static str,
    title: &'static str,
    width: u16,
    height: u16,
    cells: Vec<CaptureCell>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    fs::create_dir_all(&args.output)?;
    for screen in ConceptScreen::ALL {
        let backend = TestBackend::new(CONCEPT_WIDTH, CONCEPT_HEIGHT);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| render_concept(frame, screen))?;
        let buffer = terminal.backend().buffer();
        let capture = ConceptCapture {
            renderer: "terminal_poker::ui::concept::render_concept",
            backend: "ratatui::backend::TestBackend",
            fixture_version: 1,
            screen: screen.slug(),
            title: screen.title(),
            width: buffer.area.width,
            height: buffer.area.height,
            cells: buffer
                .content
                .iter()
                .map(|cell| CaptureCell {
                    symbol: cell.symbol().to_string(),
                    foreground: format!("{:?}", cell.fg),
                    background: format!("{:?}", cell.bg),
                    modifiers: cell.modifier.bits(),
                })
                .collect(),
        };
        fs::write(
            args.output.join(format!("{}.json", screen.slug())),
            serde_json::to_vec(&capture)?,
        )?;
    }
    fs::create_dir_all(&args.v2_output)?;
    let backend = TestBackend::new(CONCEPT_WIDTH, CONCEPT_HEIGHT);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(render_ash_continuity_mockup)?;
    let buffer = terminal.backend().buffer();
    let capture = ConceptCapture {
        renderer: "terminal_poker::ui::concept::render_ash_continuity_mockup",
        backend: "ratatui::backend::TestBackend",
        fixture_version: 2,
        screen: "ash-continuity-nine-seat-table",
        title: "ASH CONTINUITY / NINE-SEAT TABLE",
        width: buffer.area.width,
        height: buffer.area.height,
        cells: buffer
            .content
            .iter()
            .map(|cell| CaptureCell {
                symbol: cell.symbol().to_string(),
                foreground: format!("{:?}", cell.fg),
                background: format!("{:?}", cell.bg),
                modifiers: cell.modifier.bits(),
            })
            .collect(),
    };
    fs::write(
        args.v2_output.join("ash-continuity-nine-seat-table.json"),
        serde_json::to_vec(&capture)?,
    )?;

    fs::create_dir_all(&args.production_output)?;
    let backend = TestBackend::new(CONCEPT_WIDTH, CONCEPT_HEIGHT);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| render_home(frame, 0, "Ready · Quick Practice is available"))?;
    let buffer = terminal.backend().buffer();
    let capture = ConceptCapture {
        renderer: "terminal_poker::ui::shell::render_home",
        backend: "ratatui::backend::TestBackend",
        fixture_version: 3,
        screen: "installed-home",
        title: "INSTALLED HOME / QUICK PRACTICE READY",
        width: buffer.area.width,
        height: buffer.area.height,
        cells: buffer
            .content
            .iter()
            .map(|cell| CaptureCell {
                symbol: cell.symbol().to_string(),
                foreground: format!("{:?}", cell.fg),
                background: format!("{:?}", cell.bg),
                modifiers: cell.modifier.bits(),
            })
            .collect(),
    };
    fs::write(
        args.production_output.join("installed-home.json"),
        serde_json::to_vec(&capture)?,
    )?;
    println!(
        "PASS: captured {} v1 screens, one v2 mockup, and the production Home",
        ConceptScreen::ALL.len()
    );
    Ok(())
}
