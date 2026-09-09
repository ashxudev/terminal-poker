//! Public-only lobby view model for the production Ratatui renderer.

use crate::lobby::PublicTableSummary;
use crate::protocol::TableId;

#[derive(Debug, Clone)]
pub struct LobbyView {
    pub build_id: String,
    pub checkpoint: String,
    pub lobby_revision: u64,
    pub capacity: usize,
    pub selected: Option<TableId>,
    pub status: String,
    pub tables: Vec<PublicTableSummary>,
}

impl LobbyView {
    pub fn new(
        checkpoint: impl Into<String>,
        lobby_revision: u64,
        capacity: usize,
        tables: Vec<PublicTableSummary>,
    ) -> Self {
        Self {
            build_id: "terminal-poker-v1.0.1-sprint13-durable-private-beta".to_string(),
            checkpoint: checkpoint.into(),
            lobby_revision,
            capacity,
            selected: tables.first().map(|table| table.table_id),
            status: "PUBLIC DIRECTORY / private-by-code / bounded wait / one table per terminal"
                .to_string(),
            tables,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::*;
    use crate::game::seat::TableSize;
    use crate::lobby::{PublicTableStatus, PublicTableSummary};
    use crate::ui::render::render_lobby_view;

    #[test]
    fn actual_lobby_renderer_shows_public_rows_without_private_terms() {
        let view = LobbyView::new(
            "two-table-directory",
            2,
            16,
            vec![PublicTableSummary {
                table_id: TableId(1),
                name: "Alpha".to_string(),
                seats: TableSize::new(2).unwrap(),
                starting_stack: 100,
                min_players: 2,
                small_blind: 1,
                big_blind: 2,
                occupied: 0,
                reserved: 0,
                waiting: 3,
                status: PublicTableStatus::Waiting,
                joinable: true,
                visibility: crate::lobby::TableVisibility::Public,
                tournament: None,
            }],
        );
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_lobby_view(frame, &view))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("PUBLIC TABLE DIRECTORY"));
        assert!(text.contains("Alpha"));
        assert!(text.contains("WAIT"));
        assert!(text.contains("min 2"));
        for forbidden in ["hole_cards", "session token", "deck order", "reconnect"] {
            assert!(!text.contains(forbidden));
        }
    }
}
