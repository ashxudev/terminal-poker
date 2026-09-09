use ratatui::{backend::TestBackend, Terminal};
use terminal_poker::ui::{
    platform::{apply_terminal_palette, ColorDepth, ThemeMode},
    shell::render_home,
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out)?;
    for (width, height) in [(40, 20), (56, 40), (80, 24)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| {
            render_home(frame, 0, "Ready");
            let area = frame.area();
            apply_terminal_palette(
                frame.buffer_mut(),
                area,
                ThemeMode::Ash,
                ColorDepth::TrueColor,
            );
        })?;
        let cells: Vec<_> = terminal.backend().buffer().content.iter().map(|c| serde_json::json!({
            "symbol":c.symbol(),"foreground":format!("{:?}",c.fg),"background":format!("{:?}",c.bg),"modifiers":c.modifier.bits()
        })).collect();
        std::fs::write(
            out.join(format!("home-{width}x{height}.json")),
            serde_json::to_vec(&serde_json::json!({
                "checkpoint":format!("compact-home-{width}x{height}"),"backend":"ratatui::backend::TestBackend","width":width,"height":height,"cells":cells
            }))?,
        )?;
    }
    Ok(())
}
