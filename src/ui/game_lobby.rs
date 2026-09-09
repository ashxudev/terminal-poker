//! Player-facing directory. Selection follows table identity across refreshes.
use crate::{
    lobby::{PublicTableSummary, TableVisibility},
    protocol::TableId,
    ui::platform::SemanticTheme,
};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

#[derive(Default)]
pub struct GameLobby {
    pub tables: Vec<PublicTableSummary>,
    pub selected: Option<TableId>,
    pub status: String,
    pub connected: bool,
}

impl GameLobby {
    pub fn refresh(&mut self, tables: Vec<PublicTableSummary>) {
        if !tables.iter().any(|t| Some(t.table_id) == self.selected) {
            self.selected = tables.first().map(|t| t.table_id);
        }
        self.tables = tables;
        self.connected = true;
    }
    pub fn move_selection(&mut self, down: bool) {
        let index = self
            .tables
            .iter()
            .position(|t| Some(t.table_id) == self.selected)
            .unwrap_or(0);
        let index = if down {
            index
                .saturating_add(1)
                .min(self.tables.len().saturating_sub(1))
        } else {
            index.saturating_sub(1)
        };
        self.selected = self.tables.get(index).map(|t| t.table_id);
    }
    pub fn selection(&self) -> Option<&PublicTableSummary> {
        self.tables
            .iter()
            .find(|t| Some(t.table_id) == self.selected)
    }
}

pub fn admission_label(table: &PublicTableSummary) -> &'static str {
    use crate::tournament::TournamentStatus;
    match table.tournament.as_ref().map(|t| t.status) {
        None => "Ring game - client support pending",
        Some(TournamentStatus::Complete) => "Complete",
        Some(TournamentStatus::Cancelled) => "Cancelled",
        Some(TournamentStatus::Running) => "In progress - registration closed",
        Some(TournamentStatus::Break) => "On break - registration closed",
        Some(TournamentStatus::Registering) if table.joinable => "Waiting for players",
        _ => "Full or closed",
    }
}

pub fn render_game_lobby(
    frame: &mut Frame<'_>,
    lobby: &GameLobby,
    server: &str,
    theme: SemanticTheme,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.screen).fg(theme.text)),
        area,
    );
    if area.width < 40 || area.height < 20 {
        frame.render_widget(
            Paragraph::new("Resize to 40 x 20\nEsc: Back").wrap(Wrap { trim: false }),
            area,
        );
        return;
    }
    // Match the shell's centered, inset detail screens without raising the
    // lobby's smaller minimum terminal size.
    let width = area.width.min(super::shell::STANDARD_WIDTH);
    let height = area.height.min(super::shell::STANDARD_HEIGHT);
    let canvas = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let [header, body, status, footer] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(3),
        Constraint::Length(2),
        Constraint::Length(3),
    ])
    .margin(1)
    .areas(canvas);
    let mut heading = vec![Span::styled(
        " JOIN GAME ",
        Style::default()
            .fg(theme.screen)
            .bg(theme.info)
            .add_modifier(Modifier::BOLD),
    )];
    if width >= 64 {
        heading.push(Span::styled(
            "  SNEAKY BLINDERS",
            Style::default().fg(theme.text),
        ));
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(heading),
            Line::from(Span::styled(
                format!(" Server: {server}"),
                Style::default().fg(theme.muted),
            )),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        ),
        header,
    );
    let capacity = usize::from(body.height.saturating_sub(2) / 3).max(1);
    let selected = lobby
        .tables
        .iter()
        .position(|t| Some(t.table_id) == lobby.selected)
        .unwrap_or(0);
    let start = selected.saturating_sub(capacity - 1);
    let mut lines = Vec::new();
    for table in lobby.tables.iter().skip(start).take(capacity) {
        let active = Some(table.table_id) == lobby.selected;
        let access = if table.visibility == TableVisibility::PasswordProtected {
            "LOCK"
        } else {
            "OPEN"
        };
        let title = format!(
            "{} [{}] {}",
            if active { ">" } else { " " },
            access,
            table.name
        );
        let title_width = usize::from(body.width.saturating_sub(2));
        let title = if title.chars().count() > title_width {
            format!(
                "{}...",
                title
                    .chars()
                    .take(title_width.saturating_sub(3))
                    .collect::<String>()
            )
        } else {
            format!("{title:<title_width$}")
        };
        lines.push(Line::from(Span::styled(
            title,
            if active {
                Style::default()
                    .fg(theme.screen)
                    .bg(theme.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text)
            },
        )));
        lines.push(Line::from(format!(
            "  {}/{} players | Blinds {}/{}",
            table.occupied.saturating_add(table.reserved),
            table.seats.get(),
            table.small_blind,
            table.big_blind
        )));
        lines.push(Line::from(Span::styled(
            format!("  {}", admission_label(table)),
            Style::default().fg(theme.muted),
        )));
    }
    if lines.is_empty() {
        lines.push(Line::from("No games yet. Host one from Home."));
    }
    frame.render_widget(
        Paragraph::new(lines).block(super::shell::panel(
            &format!("GAMES / {}", lobby.tables.len()),
            &theme,
        )),
        body,
    );
    let message = if lobby.status.is_empty() {
        lobby
            .selection()
            .map(|t| {
                format!(
                    "Starting stack: {}\n[LOCK] requires a password.",
                    t.starting_stack
                )
            })
            .unwrap_or_else(|| "Choose a game or return Home to host.".into())
    } else {
        lobby.status.clone()
    };
    frame.render_widget(
        Paragraph::new(message)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(if lobby.connected {
                theme.muted
            } else {
                theme.danger
            })),
        status,
    );
    frame.render_widget(
        Paragraph::new("Up/Down: Select   Enter: Join\nR: Refresh   S: Server   Esc: Back")
            .style(Style::default().fg(theme.muted))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.border)),
            ),
        footer,
    );
}

/// Setup and registration share the same shell chrome as the game directory.
pub fn render_lobby_message(
    frame: &mut Frame<'_>,
    title: &str,
    text: &str,
    footer: &str,
    theme: SemanticTheme,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.screen).fg(theme.text)),
        area,
    );
    let width = area.width.min(super::shell::STANDARD_WIDTH);
    let height = area.height.min(super::shell::STANDARD_HEIGHT);
    let canvas = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let [header, body, footer_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .margin(1)
    .areas(canvas);
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(theme.screen)
                .bg(theme.info)
                .add_modifier(Modifier::BOLD),
        )]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        ),
        header,
    );
    frame.render_widget(
        Paragraph::new(format!("\n{text}"))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .block(super::shell::panel("GAME", &theme)),
        body,
    );
    frame.render_widget(
        Paragraph::new(footer.to_string())
            .style(Style::default().fg(theme.muted))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.border)),
            ),
        footer_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        table_registry::TableRegistry,
        tournament::TournamentConfig,
        ui::platform::{ColorDepth, ThemeMode},
    };
    use ratatui::{backend::TestBackend, Terminal};
    #[test]
    fn selection_survives_refresh_and_removed_games_fall_back() {
        let mut registry = TableRegistry::new(2).unwrap();
        let a = registry
            .create_tournament(TournamentConfig::recommended(2, ""), None)
            .unwrap();
        let b = registry
            .create_tournament(TournamentConfig::recommended(2, ""), None)
            .unwrap();
        let mut lobby = GameLobby::default();
        lobby.refresh(vec![a.clone(), b.clone()]);
        lobby.move_selection(true);
        lobby.refresh(vec![b.clone(), a.clone()]);
        assert_eq!(lobby.selected, Some(b.table_id));
        lobby.refresh(vec![a.clone()]);
        assert_eq!(lobby.selected, Some(a.table_id));
        lobby.refresh(vec![]);
        lobby.move_selection(true);
        assert!(lobby.selection().is_none());
    }
    #[test]
    fn compact_directory_renders_access_empty_and_connection_states() {
        let mut registry = TableRegistry::new(2).unwrap();
        let open = registry
            .create_tournament(TournamentConfig::recommended(2, ""), None)
            .unwrap();
        let locked = registry
            .create_tournament(TournamentConfig::recommended(2, "secret"), None)
            .unwrap();
        for (width, height) in [(40, 20), (80, 30), (120, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let theme = SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor);
            let mut lobby = GameLobby::default();
            lobby.refresh(vec![open.clone(), locked.clone()]);
            terminal
                .draw(|f| render_game_lobby(f, &lobby, "127.0.0.1:7777", theme))
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            for expected in ["[OPEN]", "[LOCK]", "Enter: Join", "Esc: Back"] {
                assert!(text.contains(expected), "{width}x{height}: {expected}");
            }
            assert!(!text.contains("secret"));
            lobby.refresh(vec![]);
            lobby.status = "Connection lost. R: Retry".into();
            lobby.connected = false;
            terminal
                .draw(|f| render_game_lobby(f, &lobby, "127.0.0.1:7777", theme))
                .unwrap();
            let text: String = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(text.contains("No games yet"));
            assert!(text.contains("Connection lost"));
        }
    }
}
