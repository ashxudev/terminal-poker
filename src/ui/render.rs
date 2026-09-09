use crate::game::actions::Action;
use crate::game::deck::Card;
use crate::game::seat::SeatId;
use crate::game::state::{GamePhase, BIG_BLIND};
use crate::stats::models::STAT_DEFINITIONS;
use crate::ui::app::{App, BOT_SEAT, LOCAL_SEAT};
use crate::ui::lobby::LobbyView;
use crate::ui::multiway_review::{MultiwayReviewSeatView, MultiwayReviewView};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

pub fn render_lobby_view(frame: &mut Frame, view: &LobbyView) {
    let area = frame.area();
    let outer = Block::default()
        .title(" TERMINAL POKER / PUBLIC TABLE DIRECTORY ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TABLE_BORDER));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(5),
        ])
        .split(inner);

    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                "NETWORKED MULTI-TABLE RING",
                Style::default()
                    .fg(GOLD_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  /  lobby revision {}  /  {} of {} registered",
                view.lobby_revision,
                view.tables.len(),
                view.capacity
            )),
        ]),
        Line::from(Span::styled(
            format!("{}  /  checkpoint {}", view.build_id, view.checkpoint),
            Style::default().fg(DIM),
        )),
    ])
    .alignment(Alignment::Center);
    frame.render_widget(header, chunks[0]);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("STATUS  ", Style::default().fg(LABEL)),
            Span::styled(
                &view.status,
                Style::default()
                    .fg(ACTION_CHECK)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(Block::default().borders(Borders::TOP | Borders::BOTTOM)),
        chunks[1],
    );

    let header_row = Row::new([
        "ID", "TABLE", "BLINDS", "STACK", "SEATS", "WAIT", "STATUS", "JOIN",
    ])
    .style(
        Style::default()
            .fg(GOLD_BRIGHT)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);
    let rows = view.tables.iter().map(|table| {
        let selected = view.selected == Some(table.table_id);
        let style = if selected {
            Style::default()
                .fg(Color::White)
                .bg(FELT_GREEN)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        Row::new(vec![
            Cell::from(format!("T{}", table.table_id.0)),
            Cell::from(table.name.clone()),
            Cell::from(format!("{}/{}", table.small_blind, table.big_blind)),
            Cell::from(table.starting_stack.to_string()),
            Cell::from(format!(
                "{}+{} / {} (min {})",
                table.occupied,
                table.reserved,
                table.seats.get(),
                table.min_players,
            )),
            Cell::from(table.waiting.to_string()),
            Cell::from(format!("{:?}", table.status).to_uppercase()),
            Cell::from(if table.joinable { "OPEN" } else { "LOCKED" }),
        ])
        .style(style)
    });
    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(28),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(16),
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(10),
        ],
    )
    .header(header_row)
    .column_spacing(2)
    .block(
        Block::default()
            .title(" SERVER-AUTHORITATIVE PUBLIC METADATA ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );
    frame.render_widget(table, chunks[2]);

    let footer = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("ENTER", Style::default().fg(ACTION_CALL)),
            Span::raw(" join selected   "),
            Span::styled("N", Style::default().fg(ACTION_RAISE)),
            Span::raw(" new public table   "),
            Span::styled("R", Style::default().fg(ACTION_CHECK)),
            Span::raw(" refresh   "),
            Span::styled("Q", Style::default().fg(ACTION_FOLD)),
            Span::raw(" quit"),
        ]),
        Line::from(Span::styled(
            "PRIVACY  no cards / sessions / command ledgers / random state in lobby projection",
            Style::default().fg(DIM),
        )),
    ])
    .block(Block::default().borders(Borders::TOP))
    .alignment(Alignment::Center);
    frame.render_widget(footer, chunks[3]);
}

// ── Color Palette ──────────────────────────────────────────
const FELT_GREEN: Color = Color::Rgb(0, 80, 40);
const CARD_BG: Color = Color::Rgb(214, 213, 209);
const CARD_RED: Color = Color::Rgb(200, 40, 40);
const CARD_BLACK: Color = Color::Rgb(30, 30, 30);
const LABEL: Color = Color::Rgb(200, 200, 200);
const CARD_BACK: Color = Color::Rgb(60, 60, 120);
const CARD_EMPTY: Color = Color::DarkGray;
const TABLE_BORDER: Color = Color::Rgb(100, 110, 100);
const GOLD: Color = Color::Yellow;
const GOLD_BRIGHT: Color = Color::LightYellow;
const ACTION_FOLD: Color = Color::Rgb(200, 60, 60);
const ACTION_CHECK: Color = Color::Rgb(80, 200, 80);
const ACTION_CALL: Color = Color::Rgb(80, 180, 220);
const ACTION_RAISE: Color = Color::Rgb(220, 180, 40);
// Darker button backgrounds for white-text contrast across terminals
const ACTION_FOLD_BG: Color = Color::Rgb(140, 35, 35);
const ACTION_CHECK_BG: Color = Color::Rgb(45, 130, 45);
const ACTION_CALL_BG: Color = Color::Rgb(40, 120, 160);
const ACTION_RAISE_BG: Color = Color::Rgb(160, 120, 15);
const ACTION_ALLIN_BG: Color = Color::Rgb(140, 55, 160);
const DIM: Color = Color::DarkGray;
const BTN_COLOR: Color = Color::Rgb(220, 160, 40);
const OVERLAY_BG: Color = Color::Rgb(20, 20, 30);
const OVERLAY_BORDER: Color = Color::Rgb(100, 100, 140);
const CHIP_FLAT: Color = Color::Rgb(255, 255, 255);

// ── Helpers ────────────────────────────────────────────────

fn format_bb(chips: u32) -> String {
    let bb = chips as f64 / BIG_BLIND as f64;
    if bb == bb.floor() {
        format!("{}BB", bb as u32)
    } else {
        format!("{:.1}BB", bb)
    }
}

fn overlay_block(title: &str) -> Block<'_> {
    Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(OVERLAY_BG))
        .border_style(Style::default().fg(OVERLAY_BORDER))
}

// ── Card Rendering (7-wide × 5-tall, half-block glyph borders) ──

fn render_card_lines(card: &Card) -> [Line<'static>; 5] {
    let suit_color = if card.suit.is_red() {
        CARD_RED
    } else {
        CARD_BLACK
    };
    let face_style = Style::default()
        .fg(suit_color)
        .bg(CARD_BG)
        .add_modifier(Modifier::BOLD);
    let bg_style = Style::default().bg(CARD_BG);

    let rank = card.rank.symbol();
    let suit = card.suit.symbol();
    let wide = rank.len() > 1; // "10" is 2 display chars

    // Diagonal pip: rank+suit top-left, suit center, suit+rank bottom-right
    // Each content row uses exactly 3 spans (bg, face, bg) to minimize
    // span boundaries that can cause terminal rendering artifacts.
    [
        Line::from(Span::styled("       ", bg_style)),
        Line::from(vec![
            Span::styled(" ", bg_style),
            Span::styled(rank.to_string(), face_style),
            Span::styled(if wide { "    " } else { "     " }, bg_style),
        ]),
        Line::from(vec![
            Span::styled("   ", bg_style),
            Span::styled(suit.to_string(), face_style),
            Span::styled("   ", bg_style),
        ]),
        Line::from(vec![
            Span::styled(if wide { "    " } else { "     " }, bg_style),
            Span::styled(rank.to_string(), face_style),
            Span::styled(" ", bg_style),
        ]),
        Line::from(Span::styled("       ", bg_style)),
    ]
}

fn render_facedown_lines() -> [Line<'static>; 5] {
    let bg_style = Style::default().bg(CARD_BACK);
    let back_style = Style::default().fg(Color::Rgb(100, 100, 170)).bg(CARD_BACK);

    [
        Line::from(Span::styled("       ", bg_style)),
        Line::from(vec![
            Span::styled(" ", bg_style),
            Span::styled(" ✦ ✦ ", back_style),
            Span::styled(" ", bg_style),
        ]),
        Line::from(vec![
            Span::styled(" ", bg_style),
            Span::styled("  ✦  ", back_style),
            Span::styled(" ", bg_style),
        ]),
        Line::from(vec![
            Span::styled(" ", bg_style),
            Span::styled(" ✦ ✦ ", back_style),
            Span::styled(" ", bg_style),
        ]),
        Line::from(Span::styled("       ", bg_style)),
    ]
}

fn render_empty_slot_lines() -> [Line<'static>; 5] {
    let style = Style::default().fg(CARD_EMPTY);

    [
        Line::from(Span::styled("┌╌╌╌╌╌┐", style)),
        Line::from(Span::styled("╎     ╎", style)),
        Line::from(Span::styled("╎     ╎", style)),
        Line::from(Span::styled("╎     ╎", style)),
        Line::from(Span::styled("└╌╌╌╌╌┘", style)),
    ]
}

fn compose_card_row(cards: &[[Line<'static>; 5]], gap: &str) -> Vec<Line<'static>> {
    let mut result = Vec::with_capacity(5);
    for row in 0..5 {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, card) in cards.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(gap.to_string()));
            }
            spans.extend(card[row].spans.clone());
        }
        result.push(Line::from(spans));
    }
    result
}

// ── Main Render ────────────────────────────────────────────

pub fn render(frame: &mut Frame, app: &App) {
    if let Some(view) = &app.multiway_review {
        render_multiway_review(frame, view);
        return;
    }
    let size = frame.area();

    // Outer table border (replaces margin(1))
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TABLE_BORDER));
    let full_inner = outer_block.inner(size);
    frame.render_widget(outer_block, size);

    // Cap layout height so spacers don't over-expand on tall terminals.
    // Content needs ~35 rows; beyond that, center vertically and leave
    // the surplus as empty padding above/below.
    const MAX_LAYOUT_HEIGHT: u16 = 45;
    let inner_area = if full_inner.height > MAX_LAYOUT_HEIGHT {
        let pad = (full_inner.height - MAX_LAYOUT_HEIGHT) / 2;
        Rect {
            x: full_inner.x,
            y: full_inner.y + pad,
            width: full_inner.width,
            height: MAX_LAYOUT_HEIGHT,
        }
    } else {
        full_inner
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // [0]  Status bar
            Constraint::Fill(1),   // [1]  Spacer
            Constraint::Length(1), // [2]  Opponent label
            Constraint::Fill(1),   // [3]  Spacer
            Constraint::Length(1), // [4]  Opponent stack
            Constraint::Length(5), // [5]  Opponent cards
            Constraint::Fill(1),   // [6]  Spacer
            Constraint::Length(1), // [7]  Bot action indicator
            Constraint::Fill(1),   // [8]  Spacer
            Constraint::Min(12),   // [9]  Board box (protected)
            Constraint::Fill(1),   // [10] Spacer
            Constraint::Length(1), // [11] Player action indicator
            Constraint::Fill(1),   // [12] Spacer
            Constraint::Length(5), // [13] Player cards
            Constraint::Fill(1),   // [14] Spacer
            Constraint::Length(1), // [15] Player stack
            Constraint::Fill(1),   // [16] Spacer
            Constraint::Length(1), // [17] Action bar
            Constraint::Length(1), // [18] Quick bets / raise input
            Constraint::Min(5),    // [19] Action log (bordered box)
        ])
        .split(inner_area);

    render_status_bar(frame, app, chunks[0]);
    // chunks[1] = spacer
    render_opponent_label(frame, app, chunks[2]);
    // chunks[3] = spacer
    render_opponent_stack(frame, app, chunks[4]);
    render_opponent_cards(frame, app, chunks[5]);
    // chunks[6] = spacer
    // chunks[7] = bot action indicator / showdown result
    if app.showdown_result_shown {
        if let Some(ref result) = app.game_state.showdown_result {
            let description = &result
                .hand_for(BOT_SEAT)
                .expect("offline showdown includes the bot seat")
                .description;
            let line = showdown_indicator_line(result.winner, BOT_SEAT, description);
            frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), chunks[7]);
        }
    } else if app.bot_thinking {
        let elapsed = app.tick_count.wrapping_sub(app.thinking_start_tick);
        let flower_frames = ["·", "✢", "✳", "✴", "✻", "✽", "✻", "✴", "✳", "✢"];
        let flower_idx = ((elapsed / 3) % flower_frames.len() as u64) as usize;
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                flower_frames[flower_idx],
                Style::default().fg(Color::Rgb(150, 160, 230)),
            )))
            .alignment(Alignment::Center),
            chunks[7],
        );
    } else if let Some(ref action) = app.bot_last_action {
        let paragraph = Paragraph::new(Line::from(Span::styled(
            action_label(action),
            Style::default().fg(Color::Rgb(255, 255, 255)),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(paragraph, chunks[7]);
    }
    // chunks[8] = spacer
    render_board_box(frame, app, chunks[9]);
    // chunks[10] = spacer
    render_player_label(frame, app, chunks[11]);
    // chunks[12] = spacer
    render_player_cards(frame, app, chunks[13]);
    // chunks[14] = spacer
    render_player_stack(frame, app, chunks[15]);
    // chunks[16] = spacer
    render_action_bar(frame, app, chunks[17]);
    render_raise_row(frame, app, chunks[18]);
    render_action_log(frame, app, chunks[19]);

    // Overlays (mutually exclusive — stats/help take priority over phase overlays)
    if app.show_stats {
        render_stats_overlay(frame, app);
    } else if app.show_help {
        render_help_overlay(frame);
    } else {
        match app.game_state.phase {
            GamePhase::SessionEnd => render_session_end_overlay(frame, app),
            GamePhase::Summary => render_summary_overlay(frame, app),
            _ => {}
        }
    }
}

fn render_multiway_review(frame: &mut Frame, view: &MultiwayReviewView) {
    let size = frame.area();
    let outer = Block::default()
        .title(if view.lifecycle.is_some() {
            " AUTHORITATIVE TABLE LIFECYCLE / RATATUI "
        } else if view.client.is_some() {
            " NETWORK CLIENT / AUTHORITATIVE PROJECTION "
        } else {
            " MULTIWAY TABLE / READ ONLY REVIEW "
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TABLE_BORDER));
    let inner = outer.inner(size);
    frame.render_widget(outer, size);

    let dense_seats = view.seats.len() > 6;
    let compact_height = inner.height < 44;
    let status_height = 2 + u16::from(view.client.is_some()) + u16::from(view.lifecycle.is_some());
    let seats_height = if dense_seats {
        if compact_height {
            14
        } else {
            18
        }
    } else {
        11
    };
    let board_height = if compact_height { 9 } else { 11 };
    let pots_height = if compact_height { 3 } else { 4 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(status_height),
            Constraint::Length(seats_height),
            Constraint::Length(board_height),
            Constraint::Length(pots_height),
            Constraint::Min(5),
        ])
        .split(inner);

    let mut status = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} ", view.phase.name().to_uppercase()),
                Style::default()
                    .fg(Color::Black)
                    .bg(ACTION_CALL)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  {}  /  {}  /  seed {}  /  {}",
                    view.build_id, view.hand_id, view.seed, view.checkpoint
                ),
                Style::default().fg(LABEL),
            ),
        ]),
        if let Some(protocol) = &view.protocol {
            Line::from(vec![
                Span::styled(
                    format!(" PROTOCOL v{} ", protocol.version),
                    Style::default()
                        .fg(Color::Black)
                        .bg(GOLD_BRIGHT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        " TABLE {}  HAND {}  REV {}  VIEW {}  CMD {}  {}",
                        protocol.table_id,
                        protocol.hand_id,
                        protocol.revision,
                        protocol.audience,
                        protocol.command_id,
                        protocol.outcome
                    ),
                    Style::default().fg(LABEL),
                ),
            ])
        } else {
            Line::from(vec![
                Span::styled(" POT ", Style::default().fg(GOLD_BRIGHT)),
                Span::styled(
                    view.pot_total.to_string(),
                    Style::default()
                        .fg(GOLD_BRIGHT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        "    BLINDS {}/{}  ANTE {}    CURRENT WAGER {}    PLAYER VIEW S{}",
                        view.small_blind_amount,
                        view.big_blind_amount,
                        view.ante_amount,
                        view.current_wager,
                        view.local_seat.as_u8()
                    ),
                    Style::default().fg(DIM),
                ),
            ])
        },
    ];
    if let Some(client) = &view.client {
        status.push(Line::from(vec![
            Span::styled(
                format!(" {} ", client.connection),
                Style::default()
                    .fg(Color::Black)
                    .bg(if client.connection == "CONNECTED" {
                        ACTION_CHECK
                    } else {
                        GOLD_BRIGHT
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " STREAM {}  PENDING {}  DEADLINE {}  CONTROLS {}",
                    client.stream_sequence,
                    client.pending_command,
                    client.deadline,
                    client.controls
                ),
                Style::default().fg(LABEL),
            ),
        ]));
    }
    if let Some(lifecycle) = &view.lifecycle {
        status.push(Line::from(vec![
            Span::styled(
                format!(" {} ", lifecycle.state),
                Style::default()
                    .fg(Color::Black)
                    .bg(if lifecycle.state == "PAUSED" {
                        GOLD_BRIGHT
                    } else if lifecycle.state == "CLOSED" {
                        ACTION_FOLD
                    } else {
                        ACTION_CHECK
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " HAND {}  OCCUPIED {}  ELIGIBLE {}  RESERVED {}  PENDING {}  |  {}",
                    if lifecycle.hand_active {
                        "ACTIVE"
                    } else {
                        "BOUNDARY"
                    },
                    lifecycle.occupied,
                    lifecycle.eligible,
                    lifecycle.reservations,
                    lifecycle.pending,
                    lifecycle.boundary
                ),
                Style::default().fg(LABEL),
            ),
        ]));
    }
    frame.render_widget(Paragraph::new(status), rows[0]);

    let seat_areas = multiway_seat_areas(rows[1], view.seats.len());
    for (seat, area) in view.seats.iter().zip(seat_areas.iter()) {
        render_multiway_seat(
            frame,
            seat,
            *area,
            view.highlight_local_seat.then_some(view.local_seat),
        );
    }

    let board_block = Block::default()
        .title(" BOARD ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(FELT_GREEN))
        .border_style(Style::default().fg(TABLE_BORDER));
    let board_inner = board_block.inner(rows[2]);
    frame.render_widget(board_block, rows[2]);
    let board_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(5),
            Constraint::Length(2),
        ])
        .split(board_inner);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("POT: {} chips", view.pot_total),
            Style::default()
                .fg(GOLD_BRIGHT)
                .bg(FELT_GREEN)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center)
        .style(Style::default().bg(FELT_GREEN)),
        board_rows[0],
    );
    let cards: Vec<[Line<'static>; 5]> = (0..5)
        .map(|index| {
            view.board
                .get(index)
                .map_or_else(render_empty_slot_lines, render_card_lines)
        })
        .collect();
    frame.render_widget(
        Paragraph::new(compose_card_row(&cards, " "))
            .alignment(Alignment::Center)
            .style(Style::default().bg(FELT_GREEN)),
        board_rows[1],
    );
    let actor = view.seats.iter().find(|seat| seat.to_act).map_or_else(
        || "TO ACT: -".to_string(),
        |seat| format!("TO ACT: S{}", seat.seat.as_u8()),
    );
    frame.render_widget(
        Paragraph::new(actor)
            .alignment(Alignment::Center)
            .style(Style::default().fg(ACTION_CALL).bg(FELT_GREEN)),
        board_rows[2],
    );

    let pot_text = if view.pots.is_empty() {
        format!(
            " LIVE POT {} / side pots form at terminal contribution caps ",
            view.pot_total
        )
    } else {
        view.pots
            .iter()
            .map(|pot| {
                let eligible = pot
                    .eligible
                    .iter()
                    .map(|seat| format!("S{}", seat.as_u8()))
                    .collect::<Vec<_>>()
                    .join(",");
                let winners = pot
                    .winners
                    .iter()
                    .map(|seat| format!("S{}", seat.as_u8()))
                    .collect::<Vec<_>>()
                    .join(",");
                let result = if winners.is_empty() {
                    "pending"
                } else {
                    &winners
                };
                if view.pots.len() > 3 {
                    format!("{} {} -> {}", pot.label, pot.amount, result)
                } else {
                    format!(
                        "{} {} [eligible {}] -> {}",
                        pot.label, pot.amount, eligible, result
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("   |   ")
    };
    frame.render_widget(
        Paragraph::new(pot_text)
            .block(
                Block::default()
                    .title(" POTS / AWARDS ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(TABLE_BORDER)),
            )
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(GOLD)),
        rows[3],
    );

    let log_lines = view
        .action_log
        .iter()
        .rev()
        .take(rows[4].height.saturating_sub(2) as usize)
        .rev()
        .map(|entry| {
            Line::from(Span::styled(
                format!("  {entry}"),
                Style::default().fg(LABEL),
            ))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(log_lines).block(
            Block::default()
                .title(" AUTHORITATIVE ACTION / HAND HISTORY ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(TABLE_BORDER)),
        ),
        rows[4],
    );
}

/// Render a production network client directly from its projection-derived view.
/// This deliberately bypasses the offline [`App`] and its review-fixture field.
pub fn render_network_view(frame: &mut Frame, view: &MultiwayReviewView) {
    render_network_view_with_console_scroll(frame, view, 0);
}

pub fn render_network_view_with_console_scroll(
    frame: &mut Frame,
    view: &MultiwayReviewView,
    console_scroll: usize,
) {
    crate::ui::ash_table::render_with_console_scroll(frame, view, console_scroll);
}

/// Render the production table with the installed Quick Practice lifecycle hint.
pub fn render_practice_view(frame: &mut Frame, view: &MultiwayReviewView) {
    render_practice_view_with_raise(frame, view, None);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaiseSizingView {
    pub target: u32,
    pub minimum: u32,
    pub maximum: u32,
    pub preset_index: Option<usize>,
}

/// Render Quick Practice plus its local, presentation-only raise sizing state.
/// The selected amount is still submitted through the ordinary authority.
pub fn render_practice_view_with_raise(
    frame: &mut Frame,
    view: &MultiwayReviewView,
    raise: Option<RaiseSizingView>,
) {
    let showdown = matches!(
        view.phase,
        crate::game::multiway::MultiwayPhase::Showdown
            | crate::game::multiway::MultiwayPhase::HandComplete
    )
    .then_some(crate::ui::multiway_review::ShowdownStage::Award);
    render_practice_view_with_state(frame, view, raise, 0, showdown);
}

pub fn render_practice_view_with_state(
    frame: &mut Frame,
    view: &MultiwayReviewView,
    raise: Option<RaiseSizingView>,
    console_scroll: usize,
    showdown: Option<crate::ui::multiway_review::ShowdownStage>,
) {
    crate::ui::ash_table::render_with_state(
        frame,
        view,
        console_scroll,
        raise.map(|value| (value.preset_index, value.target)),
        showdown,
    );
}

fn multiway_seat_areas(area: Rect, seat_count: usize) -> Vec<Rect> {
    if seat_count <= 6 {
        return Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, seat_count as u32); seat_count])
            .split(area)
            .to_vec();
    }
    let top_count = seat_count.div_ceil(2);
    let bottom_count = seat_count - top_count;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(area);
    let mut areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Ratio(1, top_count as u32); top_count])
        .split(rows[0])
        .to_vec();
    areas.extend(
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Ratio(1, bottom_count as u32);
                bottom_count
            ])
            .split(rows[1])
            .iter()
            .copied(),
    );
    areas
}

fn render_multiway_seat(
    frame: &mut Frame,
    seat: &MultiwayReviewSeatView,
    area: Rect,
    local_seat: Option<SeatId>,
) {
    let title = format!(
        " S{}{}{} ",
        seat.seat.as_u8(),
        if seat.position.is_empty() { "" } else { " / " },
        seat.position
    );
    let border_color = if local_seat == Some(seat.seat) {
        ACTION_CALL
    } else {
        TABLE_BORDER
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if area.height < 11 || area.width < 18 {
        let cards = if seat.cards_visible {
            seat.cards
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            "?? ??".to_string()
        };
        let compact = vec![
            Line::from(Span::styled(
                format!("{} / IN {}", seat.stack, seat.contribution),
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(cards, Style::default().fg(LABEL))),
            Line::from(Span::styled(
                if seat.to_act {
                    format!("{} / ACT", seat.status)
                } else {
                    seat.status.clone()
                },
                Style::default().fg(if seat.to_act { GOLD_BRIGHT } else { DIM }),
            )),
        ];
        frame.render_widget(
            Paragraph::new(compact)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("STACK {}", seat.stack),
                Style::default().fg(GOLD).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  IN {}", seat.contribution),
                Style::default().fg(ACTION_CALL),
            ),
        ]))
        .alignment(Alignment::Center),
        rows[0],
    );
    let cards = if seat.cards_visible {
        seat.cards.iter().map(render_card_lines).collect::<Vec<_>>()
    } else {
        vec![render_facedown_lines(), render_facedown_lines()]
    };
    frame.render_widget(
        Paragraph::new(compose_card_row(&cards, " ")).alignment(Alignment::Center),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(seat.status.clone())
            .alignment(Alignment::Center)
            .style(Style::default().fg(DIM)),
        rows[2],
    );
    if seat.to_act {
        frame.render_widget(
            Paragraph::new("[TO ACT]")
                .alignment(Alignment::Center)
                .style(
                    Style::default()
                        .fg(GOLD_BRIGHT)
                        .add_modifier(Modifier::BOLD),
                ),
            rows[3],
        );
    }
}

// ── Status Bar ─────────────────────────────────────────────

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ])
        .split(area);

    let hand_num = Paragraph::new(Line::from(vec![
        Span::styled(" Hand ", Style::default().fg(DIM)),
        Span::styled(
            format!("#{}", app.game_state.hand_number),
            Style::default().fg(DIM),
        ),
    ]));
    frame.render_widget(hand_num, cols[0]);

    let controls = Paragraph::new(Line::from(vec![
        Span::styled("S", Style::default().fg(Color::Blue)),
        Span::styled("tats ", Style::default().fg(DIM)),
        Span::styled("?", Style::default().fg(Color::Blue)),
        Span::styled("Help ", Style::default().fg(DIM)),
        Span::styled("Q", Style::default().fg(ACTION_FOLD)),
        Span::styled("uit ", Style::default().fg(DIM)),
    ]))
    .alignment(Alignment::Right);
    frame.render_widget(controls, cols[2]);
}

// ── Opponent Info ──────────────────────────────────────────

fn render_opponent_label(frame: &mut Frame, _app: &App, area: Rect) {
    let paragraph = Paragraph::new(Line::from(Span::styled(
        "OPPONENT",
        Style::default().fg(LABEL).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_opponent_stack(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        format_bb(app.game_state.stack(BOT_SEAT)),
        Style::default().fg(GOLD),
    )];

    if app.game_state.button == BOT_SEAT {
        spans.push(Span::styled(" [D]", Style::default().fg(BTN_COLOR)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

// ── Opponent Cards ─────────────────────────────────────────

fn render_opponent_cards(frame: &mut Frame, app: &App, area: Rect) {
    let card_data: Vec<[Line<'static>; 5]> = if app.showdown_revealed {
        app.game_state
            .hole_cards(BOT_SEAT)
            .iter()
            .map(|c| render_card_lines(c))
            .collect()
    } else {
        vec![render_facedown_lines(), render_facedown_lines()]
    };

    let card_lines = compose_card_row(&card_data, " ");
    let paragraph = Paragraph::new(card_lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

// ── Board Box (bordered, green felt, pot + to-call inside) ──

fn render_bet_chips(frame: &mut Frame, bet: u32, area: Rect) {
    if bet == 0 || area.is_empty() {
        return;
    }

    let amount = format_bb(bet);
    let amount_style = Style::default().fg(GOLD_BRIGHT).bg(FELT_GREEN);
    // TODO: allow players to customize the chip symbol
    let mut line_spans = vec![Span::styled(
        "⦿",
        Style::default().fg(CHIP_FLAT).add_modifier(Modifier::BOLD),
    )];
    line_spans.push(Span::raw(" "));
    line_spans.push(Span::styled(amount, amount_style));

    frame.render_widget(
        Paragraph::new(Line::from(line_spans))
            .alignment(Alignment::Center)
            .style(Style::default().bg(FELT_GREEN)),
        area,
    );
}

fn render_board_box(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TABLE_BORDER))
        .style(Style::default().bg(FELT_GREEN));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Split inner (10 rows): opp bet (1) + spacer (1) + pot info (1) + cards (5) + spacer (1) + player bet (1)
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // opponent bet chips
            Constraint::Length(1), // spacer
            Constraint::Length(1), // pot info
            Constraint::Length(5), // community cards
            Constraint::Length(1), // spacer
            Constraint::Length(1), // player bet chips
        ])
        .split(inner);

    // Bet chips (use visible snapshots so they persist until card reveal)
    render_bet_chips(frame, app.visible_bot_bet, inner_chunks[0]);

    // Pot + To Call info line — padded to card-row width so centering stays stable
    let pot_style = Style::default()
        .fg(GOLD_BRIGHT)
        .add_modifier(Modifier::BOLD);

    // During showdown, pot is zeroed (distributed to stacks), so use showdown_result
    let display_pot = if let Some(ref result) = app.game_state.showdown_result {
        result.pot_won
    } else {
        app.game_state.pot
    };
    let pot_text = format!("POT: {}", format_bb(display_pot));
    let to_call = app.game_state.amount_to_call(LOCAL_SEAT);
    let call_text = if to_call > 0 {
        format!("To call: {}", format_bb(to_call))
    } else {
        String::new()
    };

    // 39 = 5 cards × 7 chars + 4 separators (matches card row width)
    let content_len = pot_text.len() + call_text.len();
    let padding = if content_len < 39 {
        39 - content_len
    } else {
        2
    };

    let mut info_spans: Vec<Span<'static>> = vec![
        Span::styled("POT: ", pot_style),
        Span::styled(format_bb(display_pot), pot_style),
        Span::raw(" ".repeat(padding)),
    ];
    if to_call > 0 {
        info_spans.push(Span::styled("To call: ", Style::default().fg(LABEL)));
        info_spans.push(Span::styled(
            format_bb(to_call),
            Style::default()
                .fg(ACTION_CALL)
                .add_modifier(Modifier::BOLD),
        ));
    }

    let info_line = Paragraph::new(Line::from(info_spans)).alignment(Alignment::Center);
    frame.render_widget(info_line, inner_chunks[2]);

    // Community cards (use visible count so card reveal can be delayed)
    let board = &app.game_state.board;
    let visible = app.visible_board_len;
    let card_data: Vec<[Line<'static>; 5]> = (0..5)
        .map(|i| {
            if i < visible {
                render_card_lines(&board[i])
            } else {
                render_empty_slot_lines()
            }
        })
        .collect();

    let card_lines = compose_card_row(&card_data, " ");
    let paragraph = Paragraph::new(card_lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner_chunks[3]);

    render_bet_chips(frame, app.visible_player_bet, inner_chunks[5]);
}

// ── Player Info ────────────────────────────────────────────

fn action_label(action: &Action) -> &'static str {
    match action {
        Action::Fold => "FOLD",
        Action::Check => "CHECK",
        Action::Call(_) => "CALL",
        Action::Bet(_) => "BET",
        Action::Raise(_) => "RAISE",
        Action::AllIn(_) => "ALL-IN",
    }
}

fn showdown_indicator_line(
    winner: Option<SeatId>,
    this_player: SeatId,
    description: &str,
) -> Line<'static> {
    let mut spans = Vec::new();
    match winner {
        Some(w) if w == this_player => {
            spans.push(Span::styled(
                "[WIN] ",
                Style::default()
                    .fg(GOLD_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        None => {
            spans.push(Span::styled(
                "[TIE] ",
                Style::default()
                    .fg(GOLD_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        _ => {
            spans.push(Span::styled(
                "[LOSE] ",
                Style::default()
                    .fg(Color::Rgb(140, 140, 140))
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
    spans.push(Span::styled(
        description.to_string(),
        Style::default().fg(Color::Rgb(255, 255, 255)),
    ));
    Line::from(spans)
}

fn render_player_label(frame: &mut Frame, app: &App, area: Rect) {
    if app.showdown_result_shown {
        if let Some(ref result) = app.game_state.showdown_result {
            let description = &result
                .hand_for(LOCAL_SEAT)
                .expect("offline showdown includes the local seat")
                .description;
            let line = showdown_indicator_line(result.winner, LOCAL_SEAT, description);
            frame.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
        }
    } else if let Some(ref action) = app.player_last_action {
        let paragraph = Paragraph::new(Line::from(Span::styled(
            action_label(action),
            Style::default().fg(Color::Rgb(255, 255, 255)),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
    }
}

fn render_player_stack(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        format_bb(app.game_state.stack(LOCAL_SEAT)),
        Style::default().fg(GOLD),
    )];

    if app.game_state.button == LOCAL_SEAT {
        spans.push(Span::styled(" [D]", Style::default().fg(BTN_COLOR)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

// ── Player Cards ───────────────────────────────────────────

fn render_player_cards(frame: &mut Frame, app: &App, area: Rect) {
    let card_data: Vec<[Line<'static>; 5]> = app
        .game_state
        .hole_cards(LOCAL_SEAT)
        .iter()
        .map(|c| render_card_lines(c))
        .collect();

    let card_lines = compose_card_row(&card_data, "  ");
    let paragraph = Paragraph::new(card_lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

// ── Action Bar ─────────────────────────────────────────────

const BRIGHT_WHITE: Color = Color::Rgb(220, 220, 220);

fn render_action_bar(frame: &mut Frame, app: &App, area: Rect) {
    let available = app.game_state.available_actions();
    let is_player_turn = app.game_state.is_turn(LOCAL_SEAT);

    let mut spans: Vec<Span<'static>> = Vec::new();

    if app.showdown_result_shown {
        if app.game_state.stack(LOCAL_SEAT) == 0 || app.game_state.stack(BOT_SEAT) == 0 {
            spans.push(Span::styled(
                " Game Over! Press any key ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(Color::Rgb(170, 130, 10)),
            ));
        } else {
            spans.push(Span::styled(
                " N Next Hand ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(FELT_GREEN),
            ));
        }
    } else if is_player_turn && app.raise_mode {
        // Raise mode replaces the action bar
        render_raise_bar(&mut spans, app, &available);
    } else if is_player_turn {
        if available.can_fold {
            spans.push(Span::styled(
                " F Fold ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(ACTION_FOLD_BG),
            ));
            spans.push(Span::raw("   "));
        }
        if available.can_check {
            spans.push(Span::styled(
                " C Check ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(ACTION_CHECK_BG),
            ));
            spans.push(Span::raw("   "));
        }
        if let Some(amount) = available.can_call {
            spans.push(Span::styled(
                format!(" C Call {} ", format_bb(amount)),
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(ACTION_CALL_BG),
            ));
            spans.push(Span::raw("   "));
        }
        if available.min_bet.is_some() || available.min_raise.is_some() {
            spans.push(Span::styled(
                " R Raise ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(ACTION_RAISE_BG),
            ));
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(
            " A All-in ",
            Style::default()
                .fg(Color::Rgb(255, 255, 255))
                .bg(ACTION_ALLIN_BG),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_raise_bar(
    spans: &mut Vec<Span<'static>>,
    app: &App,
    available: &crate::game::actions::AvailableActions,
) {
    let min_chips = available
        .min_raise
        .unwrap_or(available.min_bet.unwrap_or(2));
    let min_bb = min_chips.div_ceil(2);
    let pot_bb = app.game_state.pot / 2;
    let stack_bb = (app.game_state.street_bet(LOCAL_SEAT) + app.game_state.stack(LOCAL_SEAT)) / 2;

    spans.push(Span::styled(
        "Raise to: ",
        Style::default().fg(ACTION_RAISE),
    ));

    if app.raise_input.is_empty() {
        spans.push(Span::styled(
            "___",
            Style::default()
                .fg(BRIGHT_WHITE)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        let typed_bb = app.raise_input.parse::<u32>().unwrap_or(0);
        spans.push(Span::styled(
            app.raise_input.clone(),
            Style::default()
                .fg(BRIGHT_WHITE)
                .add_modifier(Modifier::BOLD),
        ));
        if typed_bb > 0
            && typed_bb * 2
                >= app.game_state.street_bet(LOCAL_SEAT) + app.game_state.stack(LOCAL_SEAT)
        {
            spans.push(Span::styled(" (all-in)", Style::default().fg(GOLD)));
        } else if typed_bb > 0 && typed_bb < min_bb {
            spans.push(Span::styled(
                format!(" (min {}BB)", min_bb),
                Style::default().fg(DIM),
            ));
        }
    }

    spans.push(Span::styled("BB", Style::default().fg(BRIGHT_WHITE)));

    spans.push(Span::styled(
        format!(
            "          min {}BB · pot {}BB · stack {}BB",
            min_bb, pot_bb, stack_bb
        ),
        Style::default().fg(DIM),
    ));

    spans.push(Span::styled(
        "          Esc cancel",
        Style::default().fg(Color::Rgb(100, 100, 100)),
    ));
}

// ── Raise Row (reserved space, now unused) ────────────────

fn render_raise_row(frame: &mut Frame, _app: &App, area: Rect) {
    frame.render_widget(Paragraph::new(""), area);
}

// ── Action Log ─────────────────────────────────────────────

const LOG_TEXT: Color = Color::Rgb(220, 220, 220);
const LOG_STREET: Color = Color::Rgb(120, 120, 120);
const LOG_SEPARATOR: Color = Color::Rgb(80, 80, 80);

fn render_action_log(frame: &mut Frame, app: &App, area: Rect) {
    let log_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(TABLE_BORDER));

    if app.action_log.is_empty() {
        frame.render_widget(log_block, area);
        return;
    }

    let inner = log_block.inner(area);
    frame.render_widget(log_block, area);

    let max_entries = inner.height as usize;
    let start = app.action_log.len().saturating_sub(max_entries);

    let lines: Vec<Line<'static>> = app.action_log[start..]
        .iter()
        .map(|entry| {
            if entry.text.starts_with("──") {
                // Hand separator line
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!(
                            "{:^width$}",
                            entry.text,
                            width = inner.width.saturating_sub(4) as usize
                        ),
                        Style::default().fg(LOG_SEPARATOR),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("{:>9}", entry.street),
                        Style::default().fg(LOG_STREET),
                    ),
                    Span::styled(" │ ", Style::default().fg(LOG_SEPARATOR)),
                    Span::styled(entry.text.clone(), Style::default().fg(LOG_TEXT)),
                ])
            }
        })
        .collect();

    let paragraph = Paragraph::new(lines).alignment(Alignment::Left);
    frame.render_widget(paragraph, inner);
}

// ── Overlays ───────────────────────────────────────────────

fn render_help_overlay(frame: &mut Frame) {
    let area = centered_rect(55, 70, frame.area());
    frame.render_widget(Clear, area);

    let section_style = Style::default().fg(GOLD).add_modifier(Modifier::BOLD);
    let key_style = Style::default()
        .fg(Color::Rgb(255, 255, 255))
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::Rgb(180, 180, 180));

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("Actions", section_style)),
        Line::from(vec![
            Span::styled("  F ", key_style),
            Span::styled("Fold   ", desc_style),
            Span::styled("C ", key_style),
            Span::styled("Check   ", desc_style),
            Span::styled("C ", key_style),
            Span::styled("Call", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  A ", key_style),
            Span::styled("All-in", desc_style),
        ]),
        Line::from(""),
        Line::from(Span::styled("Raise Mode", section_style)),
        Line::from(vec![
            Span::styled("  R ", key_style),
            Span::styled("Enter raise mode", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  ", desc_style),
            Span::styled("Type BB amount", desc_style),
            Span::styled(" · ", Style::default().fg(DIM)),
            Span::styled("↑↓ ", key_style),
            Span::styled("adjust", desc_style),
        ]),
        Line::from(vec![
            Span::styled("  Enter/R ", key_style),
            Span::styled("confirm", desc_style),
            Span::styled(" · ", Style::default().fg(DIM)),
            Span::styled("Esc ", key_style),
            Span::styled("cancel", desc_style),
        ]),
        Line::from(""),
        Line::from(Span::styled("General", section_style)),
        Line::from(vec![
            Span::styled("  S ", key_style),
            Span::styled("Stats   ", desc_style),
            Span::styled("? ", key_style),
            Span::styled("Help   ", desc_style),
            Span::styled("Q ", key_style),
            Span::styled("Quit", desc_style),
        ]),
        Line::from(""),
        Line::from(Span::styled("Press ? to close", Style::default().fg(DIM))),
    ];

    let paragraph = Paragraph::new(lines).block(overlay_block("Help"));
    frame.render_widget(paragraph, area);
}

fn render_stats_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(55, 65, frame.area());
    frame.render_widget(Clear, area);

    let stats = &app.game_state;
    let local_stats = stats.seat_stats(LOCAL_SEAT);
    let section_style = Style::default().fg(GOLD).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::Rgb(180, 180, 180));
    let value_style = Style::default()
        .fg(Color::Rgb(255, 255, 255))
        .add_modifier(Modifier::BOLD);

    let win_rate = if stats.hands_played > 0 {
        local_stats.hands_won as f64 / stats.hands_played as f64 * 100.0
    } else {
        0.0
    };
    let profit = stats.session_profit_bb(LOCAL_SEAT);
    let profit_color = if profit > 0.0 {
        ACTION_CHECK
    } else if profit < 0.0 {
        ACTION_FOLD
    } else {
        Color::Rgb(255, 255, 255)
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("Session", section_style)),
        Line::from(vec![
            Span::styled("  Hands: ", label_style),
            Span::styled(format!("{}", stats.hands_played), value_style),
            Span::styled("   Won: ", label_style),
            Span::styled(format!("{}", local_stats.hands_won), value_style),
            Span::styled(format!("  ({:.0}%)", win_rate), label_style),
        ]),
        Line::from(vec![
            Span::styled("  P/L: ", label_style),
            Span::styled(
                format!("{:+.1}BB", profit),
                Style::default()
                    .fg(profit_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
    ];

    lines.push(Line::from(Span::styled("Stat Definitions", section_style)));
    for def in STAT_DEFINITIONS {
        lines.push(Line::from(vec![
            Span::styled(format!("  {} ", def.abbrev), value_style),
            Span::styled(def.explanation, label_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press S to close",
        Style::default().fg(DIM),
    )));

    let paragraph = Paragraph::new(lines)
        .block(overlay_block("Stats"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_session_end_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, frame.area());
    frame.render_widget(Clear, area);

    let winner = if app.game_state.stack(LOCAL_SEAT) == 0 {
        "You busted!"
    } else {
        "Bot busted! You win!"
    };
    let winner_color = if app.game_state.stack(LOCAL_SEAT) == 0 {
        ACTION_FOLD
    } else {
        ACTION_CHECK
    };

    let section_style = Style::default().fg(GOLD).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::Rgb(180, 180, 180));
    let value_style = Style::default()
        .fg(Color::Rgb(255, 255, 255))
        .add_modifier(Modifier::BOLD);

    let local_stats = app.game_state.seat_stats(LOCAL_SEAT);
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "SESSION COMPLETE",
            Style::default()
                .fg(GOLD_BRIGHT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            winner,
            Style::default()
                .fg(winner_color)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("Results", section_style)),
        Line::from(vec![
            Span::styled("  Hands played: ", label_style),
            Span::styled(format!("{}", app.game_state.hands_played), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Hands won: ", label_style),
            Span::styled(format!("{}", local_stats.hands_won), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Biggest pot won: ", label_style),
            Span::styled(format_bb(local_stats.biggest_pot_won), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Biggest pot lost: ", label_style),
            Span::styled(format_bb(local_stats.biggest_pot_lost), value_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " N New Session ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(ACTION_CHECK_BG),
            ),
            Span::raw("   "),
            Span::styled(
                " Q Quit ",
                Style::default()
                    .fg(Color::Rgb(255, 255, 255))
                    .bg(ACTION_FOLD_BG),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(overlay_block("Game Over"))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_summary_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 40, frame.area());
    frame.render_widget(Clear, area);

    let profit = app.game_state.session_profit_bb(LOCAL_SEAT);
    let profit_color = if profit > 0.0 {
        ACTION_CHECK
    } else if profit < 0.0 {
        ACTION_FOLD
    } else {
        Color::Rgb(255, 255, 255)
    };

    let label_style = Style::default().fg(Color::Rgb(180, 180, 180));
    let value_style = Style::default()
        .fg(Color::Rgb(255, 255, 255))
        .add_modifier(Modifier::BOLD);

    let local_stats = app.game_state.seat_stats(LOCAL_SEAT);
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "SESSION SUMMARY",
            Style::default()
                .fg(GOLD_BRIGHT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Hands played: ", label_style),
            Span::styled(format!("{}", app.game_state.hands_played), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Hands won: ", label_style),
            Span::styled(format!("{}", local_stats.hands_won), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Session P/L: ", label_style),
            Span::styled(
                format!("{:+.1}BB", profit),
                Style::default()
                    .fg(profit_color)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to exit",
            Style::default().fg(DIM),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .block(overlay_block("Summary"))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

// ── Utilities ──────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
