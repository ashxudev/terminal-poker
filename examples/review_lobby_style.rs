use ratatui::{backend::TestBackend, Terminal};
use terminal_poker::{
    table_registry::TableRegistry,
    tournament::TournamentConfig,
    ui::{
        game_lobby::{render_game_lobby, render_lobby_message, GameLobby},
        platform::{ColorDepth, SemanticTheme, ThemeMode},
    },
};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out)?;
    let mut registry = TableRegistry::new(2)?;
    let mut open = TournamentConfig::recommended(6, "");
    open.name = "Friday Night Poker".into();
    let mut locked = TournamentConfig::recommended(3, "review-only-password");
    locked.name = "Friends Table".into();
    let mut lobby = GameLobby::default();
    lobby.refresh(vec![
        registry.create_tournament(open, None)?,
        registry.create_tournament(locked, None)?,
    ]);
    lobby.move_selection(true);
    for (width, height) in [(40, 20), (80, 30), (120, 40)] {
        for kind in ["directory", "password", "waiting"] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            let theme = SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor);
            terminal.draw(|f| match kind {
                "directory" => render_game_lobby(f, &lobby, "127.0.0.1:7777", theme),
                "password" => render_lobby_message(
                    f,
                    "FRIENDS TABLE",
                    "Game password (case sensitive)\n\n> ********_",
                    "Enter confirm | Backspace edit\nEsc cancel",
                    theme,
                ),
                _ => render_lobby_message(
                    f,
                    "FRIENDS TABLE",
                    "2/3 players registered\nThe game starts when all players join.",
                    "Esc: Cancel registration",
                    theme,
                ),
            })?;
            let cells:Vec<_>=terminal.backend().buffer().content.iter().map(|c|serde_json::json!({"symbol":c.symbol(),"foreground":format!("{:?}",c.fg),"background":format!("{:?}",c.bg),"modifiers":c.modifier.bits()})).collect();
            let name = format!("{kind}-{width}x{height}");
            std::fs::write(
                out.join(format!("{name}.json")),
                serde_json::to_vec(
                    &serde_json::json!({"checkpoint":name,"backend":"ratatui::backend::TestBackend","width":width,"height":height,"cells":cells}),
                )?,
            )?;
        }
    }
    Ok(())
}
