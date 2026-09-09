use ratatui::{backend::TestBackend, Terminal};
use terminal_poker::ui::{
    platform::{apply_terminal_palette, ColorDepth, SemanticTheme, ThemeMode},
    shell::{render_shell, ShellApp, ShellEvent},
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out)?;
    for (width, height) in [(80, 24), (120, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        let mut app = ShellApp::default();
        app.handle(ShellEvent::OpenSettings);
        terminal.draw(|frame| {
            render_shell(
                frame,
                &app,
                "profile.json",
                &SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor),
            );
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
            out.join(format!("settings-{width}x{height}.json")),
            serde_json::to_vec(&serde_json::json!({
                "checkpoint":format!("compact-settings-{width}x{height}"),"backend":"ratatui::backend::TestBackend","width":width,"height":height,"cells":cells
            }))?,
        )?;
    }
    Ok(())
}
