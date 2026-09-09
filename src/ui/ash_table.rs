//! Unified portrait-first production table renderer.
//!
//! The renderer consumes only an authorized player/spectator projection. Resize
//! changes geometry and label density only; it never owns or mutates game state.

use crate::game::deck::Card;
use crate::game::multiway::MultiwayPhase;
use crate::ui::multiway_review::{MultiwayReviewSeatView, MultiwayReviewView, ShowdownStage};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub const STANDARD_WIDTH: u16 = 80;
pub const STANDARD_HEIGHT: u16 = 30;
pub const MINIMUM_WIDTH: u16 = 56;
pub const MINIMUM_HEIGHT: u16 = 30;

const FELT: Color = Color::Rgb(11, 87, 56);
const FELT_BORDER: Color = Color::Rgb(79, 138, 108);
const TEXT: Color = Color::Rgb(226, 226, 226);
const MUTED: Color = Color::Rgb(145, 145, 145);
const RULE: Color = Color::Rgb(64, 64, 64);
const PANEL: Color = Color::Rgb(16, 16, 16);
const CARD: Color = Color::Rgb(22, 22, 22);
const RED: Color = Color::Rgb(255, 74, 74);
const RED_DARK: Color = Color::Rgb(111, 23, 23);
const WINNER_GREEN: Color = Color::Rgb(57, 255, 20);

#[derive(Clone, Copy)]
enum SeatAnchor {
    BottomLeft,
    MidLeft,
    UpperLeft,
    TopLeft,
    TopRight,
    UpperRight,
    MidRight,
    BottomRight,
}

pub const fn supports_viewport(width: u16, height: u16) -> bool {
    match width {
        80.. => height >= 30,
        72..=79 => height >= 32,
        64..=71 => height >= 36,
        56..=63 => height >= 40,
        _ => false,
    }
}

pub fn render(frame: &mut Frame<'_>, view: &MultiwayReviewView) {
    render_with_console_scroll(frame, view, 0);
}

pub fn render_with_console_scroll(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    console_scroll: usize,
) {
    render_with_raise(frame, view, console_scroll, None);
}

pub fn render_with_raise(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    console_scroll: usize,
    raise: Option<(Option<usize>, u32)>,
) {
    let showdown = matches!(
        view.phase,
        MultiwayPhase::Showdown | MultiwayPhase::HandComplete
    )
    .then_some(ShowdownStage::Award);
    render_with_state(frame, view, console_scroll, raise, showdown);
}

pub fn render_with_state(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    console_scroll: usize,
    raise: Option<(Option<usize>, u32)>,
    showdown: Option<ShowdownStage>,
) {
    let viewport = frame.area();
    let showdown = if view.phase == MultiwayPhase::HandComplete {
        Some(ShowdownStage::Award)
    } else if view.showdown_progress.is_some() {
        Some(ShowdownStage::Reveal)
    } else {
        showdown
    };
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        viewport,
    );
    if !supports_viewport(viewport.width, viewport.height) {
        render_minimum_notice(frame, viewport);
        return;
    }

    let console_height = if viewport.height >= 36 { 5 } else { 4 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(14),
            Constraint::Length(console_height),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(viewport);

    render_header(frame, view, rows[0]);
    render_status(frame, view, rows[1]);
    render_table(frame, view, rows[2], showdown);
    if let Some(stage) = showdown {
        let bottom = Rect::new(
            rows[3].x,
            rows[3].y,
            rows[3].width,
            rows[5]
                .y
                .saturating_add(rows[5].height)
                .saturating_sub(rows[3].y),
        );
        render_showdown_panel(frame, view, bottom, stage);
    } else {
        render_console(frame, view, rows[3], console_scroll);
        render_actions(frame, view, rows[4], raise);
        render_footer(frame, rows[5], raise);
    }
}

fn render_minimum_notice(frame: &mut Frame<'_>, area: Rect) {
    let message = if area.width < MINIMUM_WIDTH {
        format!(
            "Terminal too narrow: {} cols; need 56x40 or wider",
            area.width
        )
    } else {
        format!(
            "Add height for {} columns: use 56x40, 64x36, 72x32, or 80x30",
            area.width
        )
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                " SNEAKY BLINDERS ",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(message, Style::default().fg(RED))),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(RULE)),
        ),
        area,
    );
}

fn render_header(frame: &mut Frame<'_>, view: &MultiwayReviewView, area: Rect) {
    let table = view
        .protocol
        .as_ref()
        .map_or_else(|| "-".to_string(), |protocol| protocol.table_id.to_string());
    let hand = view.protocol.as_ref().map_or_else(
        || view.hand_id.clone(),
        |protocol| protocol.hand_id.to_string(),
    );
    let connection = view
        .client
        .as_ref()
        .map_or("LOCAL", |client| client.connection.as_str());
    let connection_style = if matches!(connection, "DISCONNECTED" | "AWAITING RESYNC") {
        Style::default().fg(RED).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
    };
    let compact = area.width < 72;
    let middle = if compact {
        format!(" T{table} · H{hand}")
    } else {
        format!(" TABLE {table} · HAND {hand}")
    };
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(if compact { 19 } else { 22 }),
            Constraint::Min(10),
            Constraint::Length(if compact { 8 } else { 18 }),
        ])
        .split(Rect::new(area.x, area.y, area.width, 1));
    frame.render_widget(
        Paragraph::new(Span::styled(
            " ▥ SNEAKY BLINDERS",
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        columns[0],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(middle, Style::default().fg(MUTED)))
            .alignment(Alignment::Center),
        columns[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("● ", connection_style),
            Span::styled(if compact { "LIVE" } else { connection }, connection_style),
        ]))
        .alignment(Alignment::Right),
        columns[2],
    );
    frame.buffer_mut().set_string(
        area.x,
        area.y + 1,
        "─".repeat(area.width as usize),
        Style::default().fg(RULE),
    );
}

fn render_status(frame: &mut Frame<'_>, view: &MultiwayReviewView, area: Rect) {
    let occupied = view.seats.len();
    let blind = if view.ante_amount > 0 {
        format!(
            "{}/{} A{}",
            view.small_blind_amount, view.big_blind_amount, view.ante_amount
        )
    } else {
        format!("{}/{}", view.small_blind_amount, view.big_blind_amount)
    };
    let checkpoint = if let Some(progress) = &view.showdown_progress {
        if progress.all_in {
            "ALL-IN RUNOUT".to_string()
        } else {
            "SHOWDOWN".to_string()
        }
    } else if view.checkpoint.starts_with("LEVEL ") {
        view.checkpoint.clone()
    } else {
        view.phase.name().to_uppercase()
    };
    let mut text = if area.width < 64 {
        format!(" {checkpoint} · {blind} · {occupied}P")
    } else {
        format!(" {checkpoint} · {blind} · {occupied} PLAYERS")
    };
    if view.showdown_progress.is_none()
        && !matches!(
            view.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        )
    {
        text.push_str(if view.always_show {
            " · H SHOW"
        } else {
            " · H AUTO-MUCK"
        });
    }
    frame.render_widget(
        Paragraph::new(Span::styled(text, Style::default().fg(MUTED)))
            .style(Style::default().bg(Color::Black)),
        area,
    );
}

fn render_table(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    area: Rect,
    showdown: Option<ShowdownStage>,
) {
    let seat_width: u16 = if area.width < 64 {
        9
    } else if area.width < 80 {
        11
    } else {
        14
    };
    let felt_width = area
        .width
        .saturating_sub(seat_width.saturating_mul(2).saturating_add(2))
        .clamp(26, 48);
    let felt_height = area.height.saturating_sub(2).max(12);
    let felt = Rect::new(
        area.x + (area.width.saturating_sub(felt_width)) / 2,
        area.y + 1,
        felt_width,
        felt_height.min(area.height.saturating_sub(1)),
    );
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(FELT_BORDER))
            .style(Style::default().bg(FELT)),
        felt,
    );

    render_board(frame, view, felt, showdown);
    render_opponents(frame, view, area, felt, seat_width, showdown);
    render_local_seat(frame, view, felt, seat_width, showdown);
}

fn render_board(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    felt: Rect,
    showdown: Option<ShowdownStage>,
) {
    let center_y = felt.y + felt.height / 2;
    frame.render_widget(
        Paragraph::new(Span::styled(
            format!("POT {}", view.pot_total),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center)
        .style(Style::default().bg(FELT)),
        Rect::new(
            felt.x + 1,
            center_y.saturating_sub(3),
            felt.width.saturating_sub(2),
            1,
        ),
    );

    let shown_slots = if felt.width < 30 && view.board.len() < 4 {
        3
    } else {
        5
    };
    let row_width = shown_slots * 4;
    let start_x = felt.x + (felt.width.saturating_sub(row_width)) / 2;
    for index in 0..shown_slots {
        let area = Rect::new(start_x + index * 4, center_y.saturating_sub(1), 4, 1);
        if let Some(card) = view.board.get(index as usize) {
            let plays = matches!(
                showdown,
                Some(ShowdownStage::Winners | ShowdownStage::Award)
            ) && view
                .seats
                .iter()
                .filter(|seat| seat.winner)
                .filter_map(|seat| seat.showdown_hand.as_ref())
                .any(|hand| hand.best_five.contains(card));
            draw_card(frame, area, card, plays);
        } else {
            frame.render_widget(
                Paragraph::new(Span::styled("[░]", Style::default().fg(MUTED).bg(CARD)))
                    .alignment(Alignment::Center),
                area,
            );
        }
    }
    let detail = showdown.map_or_else(
        || {
            view.legal_actions.as_ref().map_or_else(
                || view.phase.name().to_uppercase(),
                |legal| {
                    if legal.can_check {
                        format!("{} · CHECK", view.phase.name().to_uppercase())
                    } else if let Some(amount) = legal.call_amount {
                        format!("{} · CALL {amount}", view.phase.name().to_uppercase())
                    } else {
                        view.phase.name().to_uppercase()
                    }
                },
            )
        },
        |stage| match stage {
            ShowdownStage::Reveal if view.showdown_progress.as_ref().is_some_and(|p| p.all_in) => {
                "ALL-IN · RUNOUT".to_string()
            }
            ShowdownStage::Reveal => "SHOWDOWN · CARDS UP".to_string(),
            ShowdownStage::Winners => "WINNING FIVE HIGHLIGHTED".to_string(),
            ShowdownStage::Award => "POT AWARDED".to_string(),
        },
    );
    frame.render_widget(
        Paragraph::new(Span::styled(detail, Style::default().fg(TEXT)))
            .alignment(Alignment::Center)
            .style(Style::default().bg(FELT)),
        Rect::new(felt.x + 1, center_y + 1, felt.width.saturating_sub(2), 1),
    );
}

fn render_opponents(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    stage: Rect,
    felt: Rect,
    seat_width: u16,
    showdown: Option<ShowdownStage>,
) {
    let table_size = view.table_size.clamp(2, 9);
    for (index, anchor) in anchors_for(table_size).iter().copied().enumerate() {
        let delta = index as u8 + 1;
        let physical = (view.local_seat.as_u8() + delta) % table_size;
        let seat = view
            .seats
            .iter()
            .find(|candidate| candidate.seat.as_u8() == physical);
        let rect = seat_rect(anchor, stage, felt, seat_width);
        render_opponent(frame, seat, physical, rect, anchor, showdown);
    }
}

fn seat_rect(anchor: SeatAnchor, stage: Rect, felt: Rect, width: u16) -> Rect {
    let left = stage.x;
    let right = stage.x + stage.width.saturating_sub(width);
    let top_left = felt.x.saturating_sub(width / 2).max(stage.x);
    let top_right = (felt.x + felt.width).saturating_sub(width / 2).min(right);
    let upper = felt.y + felt.height / 4;
    let middle = felt.y + felt.height / 2;
    let lower = felt.y + felt.height.saturating_sub(4);
    let (x, y) = match anchor {
        SeatAnchor::BottomLeft => (left, lower),
        SeatAnchor::MidLeft => (left, middle),
        SeatAnchor::UpperLeft => (left, upper),
        SeatAnchor::TopLeft => (top_left, stage.y),
        SeatAnchor::TopRight => (top_right, stage.y),
        SeatAnchor::UpperRight => (right, upper),
        SeatAnchor::MidRight => (right, middle),
        SeatAnchor::BottomRight => (right, lower),
    };
    Rect::new(x, y, width, 3)
}

fn render_opponent(
    frame: &mut Frame<'_>,
    seat: Option<&MultiwayReviewSeatView>,
    physical: u8,
    area: Rect,
    anchor: SeatAnchor,
    showdown: Option<ShowdownStage>,
) {
    let alignment = if matches!(
        anchor,
        SeatAnchor::UpperRight
            | SeatAnchor::MidRight
            | SeatAnchor::BottomRight
            | SeatAnchor::TopRight
    ) {
        Alignment::Right
    } else {
        Alignment::Left
    };
    let Some(seat) = seat else {
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(format!("S{physical} OPEN")),
                Line::from("-"),
            ])
            .alignment(alignment)
            .style(Style::default().fg(MUTED).bg(PANEL)),
            area,
        );
        return;
    };
    let (label, status, highlighted) = showdown.map_or_else(
        || {
            let marker = if seat.position.is_empty() {
                String::new()
            } else {
                format!(" {}", seat.position)
            };
            let status = if seat.to_act || seat.contribution > 0 {
                format!("{} / {}", seat.stack, seat.contribution)
            } else {
                seat.stack.to_string()
            };
            (
                format!("S{}{marker}", seat.seat.as_u8()),
                status,
                seat.to_act,
            )
        },
        |stage| {
            let highlighted =
                seat.winner && matches!(stage, ShowdownStage::Winners | ShowdownStage::Award);
            let label = match stage {
                ShowdownStage::Reveal if seat.folded => {
                    format!("S{} FOLD", seat.seat.as_u8())
                }
                _ if seat.status == "MUCKED" => format!("S{} MUCK", seat.seat.as_u8()),
                ShowdownStage::Reveal if !seat.cards_visible => {
                    format!("S{} HOLD", seat.seat.as_u8())
                }
                ShowdownStage::Reveal => format!("S{} SHOW", seat.seat.as_u8()),
                ShowdownStage::Winners if seat.winner => {
                    format!("S{} WIN", seat.seat.as_u8())
                }
                ShowdownStage::Award if seat.winner => {
                    format!("S{} +{}", seat.seat.as_u8(), seat.awarded)
                }
                _ if seat.folded => format!("S{} FOLD", seat.seat.as_u8()),
                _ if !seat.cards_visible => format!("S{} HOLD", seat.seat.as_u8()),
                _ => format!("S{} SHOW", seat.seat.as_u8()),
            };
            let status = if seat.folded {
                "FOLDED".to_string()
            } else if seat.status == "MUCKED" {
                "MUCKED".to_string()
            } else if seat.cards_visible {
                cards_compact(&seat.cards)
            } else {
                "[░] [░]".to_string()
            };
            (label, status, highlighted)
        },
    );
    let color = if seat.folded {
        MUTED
    } else if highlighted || seat.to_act {
        RED
    } else {
        TEXT
    };
    let mut lines = vec![
        Line::from(Span::styled(
            label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
        if highlighted && seat.cards_visible {
            let mut spans = Vec::new();
            for card in &seat.cards {
                spans.extend(styled_brackets(
                    compact_card(card),
                    Style::default().fg(TEXT),
                    seat.showdown_hand
                        .as_ref()
                        .is_some_and(|hand| hand.best_five.contains(card)),
                ));
            }
            Line::from(spans)
        } else {
            Line::from(styled_brackets(status, Style::default().fg(MUTED), false))
        },
    ];
    if showdown.is_none() {
        let holding = if seat.folded {
            "FOLDED".to_string()
        } else if seat.status == "NOTDEALT" {
            "WAITING".to_string()
        } else if seat.cards_visible {
            cards_compact(&seat.cards)
        } else {
            "[░] [░]".to_string()
        };
        lines.push(Line::from(Span::styled(
            holding,
            Style::default()
                .fg(if seat.folded || seat.status == "NOTDEALT" {
                    MUTED
                } else {
                    TEXT
                })
                .add_modifier(Modifier::BOLD),
        )));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(alignment)
            .style(Style::default().bg(if highlighted || seat.to_act {
                RED_DARK
            } else {
                PANEL
            }))
            .block(
                Block::default()
                    .borders(Borders::LEFT)
                    .border_style(Style::default().fg(if highlighted || seat.to_act {
                        RED
                    } else {
                        RULE
                    })),
            ),
        area,
    );
}

fn render_local_seat(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    felt: Rect,
    seat_width: u16,
    showdown: Option<ShowdownStage>,
) {
    let width = (seat_width.saturating_mul(2)).clamp(18, 28).min(felt.width);
    let area = Rect::new(
        felt.x + (felt.width.saturating_sub(width)) / 2,
        felt.y + felt.height.saturating_sub(4),
        width,
        4,
    );
    let local = view.seats.iter().find(|seat| seat.seat == view.local_seat);
    let mucked = view.mucked.contains(&view.local_seat);
    let shown = view.publicly_shown.contains(&view.local_seat);
    let (label, stack, cards, folded, winner, awarded) = local.map_or_else(
        || {
            (
                format!("S{} OPEN", view.local_seat.as_u8()),
                "-".to_string(),
                Vec::new(),
                false,
                false,
                0,
            )
        },
        |seat| {
            (
                if view.highlight_local_seat {
                    format!("YOU · S{}", seat.seat.as_u8())
                } else {
                    format!("WATCH · S{}", seat.seat.as_u8())
                },
                seat.stack.to_string(),
                seat.cards.clone(),
                seat.folded,
                seat.winner,
                seat.awarded,
            )
        },
    );
    let state_label = if folded {
        "FOLDED".to_string()
    } else if mucked {
        "MUCKED".to_string()
    } else {
        match showdown {
            Some(ShowdownStage::Reveal) if !shown => "PRIVATE".to_string(),
            Some(ShowdownStage::Reveal) => "SHOW".to_string(),
            Some(ShowdownStage::Winners) if winner => "WINNER".to_string(),
            Some(ShowdownStage::Award) if winner => format!("+{awarded}"),
            _ => String::new(),
        }
    };
    let header = if width < 28 {
        format!(
            "{} S{} {stack}",
            if view.highlight_local_seat {
                "YOU"
            } else {
                "WATCH"
            },
            view.local_seat.as_u8()
        )
    } else {
        format!("{label} · {stack}")
    };
    let highlight_winner = winner
        && matches!(
            showdown,
            Some(ShowdownStage::Winners | ShowdownStage::Award)
        );
    let mut card_spans = Vec::new();
    for card in cards.iter().take(
        if folded || mucked || local.is_none_or(|seat| !seat.cards_visible) {
            0
        } else {
            2
        },
    ) {
        if !card_spans.is_empty() {
            card_spans.push(Span::raw(" "));
        }
        card_spans.extend(styled_brackets(
            format!("[ {}{} ]", card.rank.symbol(), card.suit.symbol()),
            Style::default()
                .fg(if card.suit.is_red() { RED } else { TEXT })
                .bg(CARD)
                .add_modifier(Modifier::BOLD),
            highlight_winner
                && local
                    .and_then(|seat| seat.showdown_hand.as_ref())
                    .is_some_and(|hand| hand.best_five.contains(card)),
        ));
    }
    if card_spans.is_empty() {
        let holding = if folded {
            "FOLDED"
        } else if mucked {
            "MUCKED"
        } else if local.is_none_or(|seat| seat.status == "NOTDEALT") {
            "WAITING"
        } else {
            "[░] [░]"
        };
        card_spans.push(Span::styled(holding, Style::default().fg(MUTED).bg(CARD)));
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                header,
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(card_spans),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().bg(PANEL))
        .block(
            Block::default()
                .title(Span::styled(state_label, Style::default().fg(MUTED)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(if highlight_winner { RED } else { RULE })),
        ),
        area,
    );
}

fn draw_card(frame: &mut Frame<'_>, area: Rect, card: &Card, highlighted: bool) {
    frame.render_widget(
        Paragraph::new(Line::from(styled_brackets(
            compact_card(card),
            Style::default()
                .fg(if card.suit.is_red() { RED } else { TEXT })
                .bg(CARD)
                .add_modifier(Modifier::BOLD),
            highlighted,
        )))
        .alignment(Alignment::Center),
        area,
    );
}

fn styled_brackets(text: String, style: Style, highlighted: bool) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut content = String::new();
    for ch in text.chars() {
        if matches!(ch, '[' | ']') {
            if !content.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut content), style));
            }
            spans.push(Span::styled(
                ch.to_string(),
                if highlighted {
                    style.fg(WINNER_GREEN).add_modifier(Modifier::BOLD)
                } else {
                    style
                },
            ));
        } else {
            content.push(ch);
        }
    }
    if !content.is_empty() {
        spans.push(Span::styled(content, style));
    }
    spans
}

fn compact_card(card: &Card) -> String {
    // Four-column board cells and eight-column opponent holdings must retain
    // both brackets even for tens.
    let rank = if card.rank == crate::game::deck::Rank::Ten {
        "T"
    } else {
        card.rank.symbol()
    };
    format!("[{rank}{}]", card.suit.symbol())
}

fn cards_compact(cards: &[Card]) -> String {
    cards.iter().take(2).map(compact_card).collect::<String>()
}

fn winning_five(cards: &[Card]) -> String {
    cards.iter().map(compact_card).collect::<Vec<_>>().join(" ")
}

fn short_hand(description: &str) -> &'static str {
    let lower = description.to_ascii_lowercase();
    if lower.contains("straight flush") {
        "STRAIGHT FLUSH"
    } else if lower.contains("four of a kind") {
        "QUADS"
    } else if lower.contains("full house") {
        "FULL HOUSE"
    } else if lower.contains("flush") {
        "FLUSH"
    } else if lower.contains("straight") {
        "STRAIGHT"
    } else if lower.contains("three of a kind") {
        "TRIPS"
    } else if lower.contains("two pair") {
        "TWO PAIR"
    } else if lower.contains("pair") {
        "PAIR"
    } else {
        "HIGH CARD"
    }
}

fn render_showdown_panel(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    area: Rect,
    stage: ShowdownStage,
) {
    let (title, entries) = match stage {
        ShowdownStage::Reveal => (
            if view.showdown_progress.as_ref().is_some_and(|p| p.all_in) {
                " ALL-IN · CARDS UP / RUNOUT "
            } else {
                " SHOWDOWN · REVEALING CARDS "
            },
            view.seats
                .iter()
                .map(|seat| {
                    if seat.folded {
                        format!("S{} · FOLDED", seat.seat.as_u8())
                    } else if view.mucked.contains(&seat.seat) {
                        format!("S{} · MUCKED", seat.seat.as_u8())
                    } else if view.publicly_shown.contains(&seat.seat) {
                        format!(
                            "S{} · SHOW {}",
                            seat.seat.as_u8(),
                            cards_compact(&seat.cards)
                        )
                    } else {
                        format!("S{} · REVEALING", seat.seat.as_u8())
                    }
                })
                .collect::<Vec<_>>(),
        ),
        ShowdownStage::Winners => (
            " SHOWDOWN · WINNING HAND ",
            view.seats
                .iter()
                .filter(|seat| seat.winner)
                .map(|seat| {
                    seat.showdown_hand.as_ref().map_or_else(
                        || format!("S{} · WINS WITHOUT SHOWDOWN", seat.seat.as_u8()),
                        |hand| {
                            format!(
                                "S{} · {} · {}",
                                seat.seat.as_u8(),
                                short_hand(&hand.description),
                                winning_five(&hand.best_five)
                            )
                        },
                    )
                })
                .collect::<Vec<_>>(),
        ),
        ShowdownStage::Award => (
            if view.phase == MultiwayPhase::HandComplete {
                " UNCONTESTED · POT AWARDED "
            } else {
                " SHOWDOWN · POT AWARDED "
            },
            view.seats
                .iter()
                .filter(|seat| seat.winner)
                .map(|seat| {
                    format!(
                        "S{} · +{} CHIPS · STACK {}",
                        seat.seat.as_u8(),
                        seat.awarded,
                        seat.stack
                    )
                })
                .collect::<Vec<_>>(),
        ),
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(RED));
    let inner = block.inner(area);
    frame.render_widget(block.style(Style::default().bg(PANEL)), area);
    let rows_per_column = usize::from(inner.height).max(1);
    let column_count = entries.len().div_ceil(rows_per_column).clamp(1, 3);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![
            Constraint::Ratio(
                1,
                u32::try_from(column_count).unwrap_or(1)
            );
            column_count
        ])
        .split(inner);
    for (column, area) in columns.iter().enumerate() {
        let start = column * rows_per_column;
        let lines = entries
            .iter()
            .skip(start)
            .take(rows_per_column)
            .map(|entry| {
                Line::from(styled_brackets(
                    format!(" {entry}"),
                    Style::default()
                        .fg(if stage == ShowdownStage::Reveal {
                            TEXT
                        } else {
                            RED
                        })
                        .add_modifier(Modifier::BOLD),
                    stage == ShowdownStage::Winners,
                ))
            })
            .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(lines).style(Style::default().bg(PANEL)),
            *area,
        );
    }
}

fn render_console(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    area: Rect,
    console_scroll: usize,
) {
    let capacity = usize::from(area.height.saturating_sub(3)).max(1);
    let scroll = console_scroll.min(view.action_log.len().saturating_sub(1));
    let end = view.action_log.len().saturating_sub(scroll);
    let start = end.saturating_sub(capacity);
    let mut lines = view.action_log[start..end]
        .iter()
        .map(|message| {
            Line::from(Span::styled(
                format!(" {message}"),
                Style::default().fg(TEXT),
            ))
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            " Waiting for dealer",
            Style::default().fg(MUTED),
        )));
    }
    let actor = view
        .seats
        .iter()
        .find(|seat| seat.to_act)
        .map(|seat| seat.seat);
    let prompt = view.client.as_ref().map_or_else(
        || {
            if actor == Some(view.local_seat) && view.legal_actions.is_some() {
                " YOU TO ACT".to_string()
            } else {
                " Waiting for action".to_string()
            }
        },
        |client| {
            if client.pending_command != "none" {
                " AWAITING AUTHORITY · ACTIONS DISABLED".to_string()
            } else if client.connection != "CONNECTED" {
                format!(" {} · ACTIONS DISABLED", client.connection)
            } else if actor == Some(view.local_seat) && client.controls == "ENABLED" {
                " YOU TO ACT".to_string()
            } else {
                " Waiting for action".to_string()
            }
        },
    );
    lines.push(Line::from(Span::styled(
        prompt,
        Style::default().fg(
            if view.client.as_ref().is_some_and(|client| {
                client.pending_command != "none" || client.connection != "CONNECTED"
            }) {
                RED
            } else {
                MUTED
            },
        ),
    )));
    let title = if scroll == 0 {
        " TABLE CONSOLE ".to_string()
    } else {
        format!(" TABLE CONSOLE · {scroll} BACK ")
    };
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(PANEL))
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(RULE)),
            ),
        area,
    );
}

fn render_actions(
    frame: &mut Frame<'_>,
    view: &MultiwayReviewView,
    area: Rect,
    raise: Option<(Option<usize>, u32)>,
) {
    let controls = view
        .client
        .as_ref()
        .map_or(view.legal_actions.is_some(), |client| {
            client.controls == "ENABLED"
        });
    let legal = view.legal_actions.as_ref();
    let passive = legal.map_or_else(
        || "C CHECK".to_string(),
        |item| {
            if item.can_check {
                "C CHECK".to_string()
            } else {
                item.call_amount
                    .map_or_else(|| "C CALL".to_string(), |amount| format!("C {amount}"))
            }
        },
    );
    let raise_label = raise.map_or_else(
        || "R RAISE".to_string(),
        |(_, target)| {
            if area.width < 72 {
                format!("R {target}")
            } else if legal.is_some_and(|item| item.min_bet_to.is_some()) {
                format!("R BET {target}")
            } else {
                format!("R RAISE {target}")
            }
        },
    );
    let labels = [
        "F FOLD".to_string(),
        passive,
        raise_label,
        "A ALL-IN".to_string(),
    ];
    let enabled = [
        controls && legal.is_some_and(|item| item.can_fold),
        controls && legal.is_some_and(|item| item.can_check || item.call_amount.is_some()),
        controls
            && legal.is_some_and(|item| item.min_raise_to.is_some() || item.min_bet_to.is_some()),
        controls && legal.is_some(),
    ];
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(area);
    for index in 0..4 {
        draw_action(
            frame,
            columns[index],
            &labels[index],
            enabled[index],
            index == 1,
        );
    }
}

fn draw_action(frame: &mut Frame<'_>, area: Rect, label: &str, enabled: bool, primary: bool) {
    let style = if enabled && primary {
        Style::default()
            .fg(Color::White)
            .bg(RED_DARK)
            .add_modifier(Modifier::BOLD)
    } else if enabled {
        Style::default()
            .fg(if label.starts_with('F') { RED } else { TEXT })
            .bg(PANEL)
    } else {
        Style::default().fg(MUTED).bg(Color::Black)
    };
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(style)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(if enabled && primary { RED } else { RULE })),
            ),
        area,
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, raise: Option<(Option<usize>, u32)>) {
    let Some((selected, _)) = raise else {
        let hint = if area.width < 72 {
            " Pg log · ↑/↓ chips · 1-5 presets · R raise · Esc home"
        } else {
            " PgUp/PgDn console · ↑/↓ chips · 1-5 presets · R raise · Esc home"
        };
        frame.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(MUTED).bg(PANEL))),
            area,
        );
        return;
    };

    let labels = if area.width < 72 {
        ["1:25", "2:50", "3:75", "4:P", "5:1.5P"]
    } else {
        ["1:25%", "2:50%", "3:75%", "4:POT", "5:1.5P"]
    };
    let mut spans = vec![Span::styled(
        " ↑/↓ ±1 ",
        Style::default().fg(MUTED).bg(PANEL),
    )];
    for (index, label) in labels.iter().enumerate() {
        spans.push(Span::styled(
            format!(" {label} "),
            if Some(index) == selected {
                Style::default()
                    .fg(Color::White)
                    .bg(RED_DARK)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(MUTED).bg(PANEL)
            },
        ));
    }
    spans.push(Span::styled(
        if area.width < 72 {
            " · R raise"
        } else {
            " · R raise · Pg log"
        },
        Style::default().fg(MUTED).bg(PANEL),
    ));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(PANEL)),
        area,
    );
}

fn anchors_for(table_size: u8) -> &'static [SeatAnchor] {
    use SeatAnchor::*;
    match table_size {
        2 => &[TopLeft],
        3 => &[UpperLeft, UpperRight],
        4 => &[UpperLeft, TopLeft, UpperRight],
        5 => &[MidLeft, TopLeft, TopRight, MidRight],
        6 => &[BottomLeft, UpperLeft, TopLeft, UpperRight, BottomRight],
        7 => &[
            BottomLeft,
            MidLeft,
            TopLeft,
            TopRight,
            MidRight,
            BottomRight,
        ],
        8 => &[
            BottomLeft,
            MidLeft,
            UpperLeft,
            TopLeft,
            UpperRight,
            MidRight,
            BottomRight,
        ],
        _ => &[
            BottomLeft,
            MidLeft,
            UpperLeft,
            TopLeft,
            TopRight,
            UpperRight,
            MidRight,
            BottomRight,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::actions::Action;
    use crate::game::command::SeatCommand;
    use crate::game::multiway::MultiwayHand;
    use crate::game::seat::{SeatId, TableSize};
    use crate::local_practice::LocalPractice;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn every_supported_table_size_has_one_anchor_per_opponent() {
        for table_size in 2..=9 {
            assert_eq!(anchors_for(table_size).len(), usize::from(table_size - 1));
        }
    }

    #[test]
    fn approved_width_height_envelope_is_explicit() {
        for viewport in [(80, 30), (72, 32), (64, 36), (56, 40), (120, 40), (160, 50)] {
            assert!(supports_viewport(viewport.0, viewport.1), "{viewport:?}");
        }
        for viewport in [(55, 50), (56, 39), (64, 35), (72, 31), (80, 29)] {
            assert!(!supports_viewport(viewport.0, viewport.1), "{viewport:?}");
        }
    }

    #[test]
    fn one_renderer_preserves_nine_seats_and_landmarks_at_every_approved_size() {
        let practice = LocalPractice::nine_handed_seeded_for_review(100, 14_001).unwrap();
        let view = practice.view();
        for (width, height) in [(80, 30), (72, 32), (64, 36), (56, 40), (120, 40)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|frame| render(frame, &view)).unwrap();
            let text = buffer_text(terminal.backend().buffer());
            for seat in 0..9 {
                assert!(
                    text.contains(&format!("S{seat}")),
                    "missing S{seat} at {width}x{height}"
                );
            }
            for landmark in ["POT", "TABLE CONSOLE", "F FOLD", "R RAISE", "Esc home"] {
                assert!(
                    text.contains(landmark),
                    "missing {landmark} at {width}x{height}"
                );
            }
        }
    }

    #[test]
    fn console_scroll_remains_available_in_the_unified_renderer() {
        let practice = LocalPractice::nine_handed_seeded_for_review(100, 14_001).unwrap();
        let mut view = practice.view();
        view.action_log = (1..=9)
            .map(|index| format!("Dealer · message {index}"))
            .collect();
        let backend = TestBackend::new(STANDARD_WIDTH, STANDARD_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_with_console_scroll(frame, &view, 4))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert!(text.contains("4 BACK"));
        assert!(text.contains("Dealer · message 5"));
        assert!(!text.contains("Dealer · message 9"));
    }

    #[test]
    fn holdings_have_inner_padding_and_raise_presets_share_the_table_chrome() {
        let practice = LocalPractice::nine_handed_seeded_for_review(100, 14_001).unwrap();
        let view = practice.view();
        for (width, height) in [(80, 30), (72, 32), (64, 36), (56, 40), (120, 40)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal
                .draw(|frame| render_with_raise(frame, &view, 0, Some((Some(1), 34))))
                .unwrap();
            let text = buffer_text(terminal.backend().buffer());

            assert!(
                text.contains(" ] [ "),
                "holdings overlap at {width}x{height}"
            );
            assert!(
                text.contains("1.5P"),
                "preset row clips at {width}x{height}"
            );
            assert!(
                text.contains("R raise"),
                "submit hint clips at {width}x{height}"
            );
            if width < 72 {
                assert!(text.contains("R 34"));
            } else {
                for label in ["25%", "50%", "75%", "POT", "1.5P"] {
                    assert!(text.contains(label), "missing raise preset {label}");
                }
                assert!(text.contains("R RAISE 34"));
            }
            assert!(!text.contains("Enter confirm"));
        }
    }

    #[test]
    fn card_presence_distinguishes_live_folded_waiting_and_open_seats_without_leaking_faces() {
        use crate::game::deck::{Rank, Suit};
        let practice = LocalPractice::nine_handed_seeded_for_review(100, 14_001).unwrap();
        let mut view = practice.view();
        for seat in &mut view.seats {
            if seat.seat.as_u8() == 1 || seat.seat.as_u8() == 5 {
                seat.folded = true;
                seat.status = "FOLDED".into();
            }
            if seat.seat.as_u8() == 2 {
                seat.status = "NOTDEALT".into();
            }
            if seat.seat.as_u8() == 3 {
                seat.status = "ALLIN".into();
            }
            if seat.seat.as_u8() == 4 {
                // A hidden hand must remain hidden even if a review fixture has faces.
                seat.cards = vec![
                    Card::new(Rank::Ace, Suit::Clubs),
                    Card::new(Rank::King, Suit::Clubs),
                ];
                seat.cards_visible = false;
            }
        }
        view.seats.retain(|seat| seat.seat.as_u8() != 8);
        for (width, height) in [(80, 30), (72, 32), (64, 36), (56, 40), (120, 40)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| render_with_state(frame, &view, 0, None, None))
                .unwrap();
            let text = buffer_text(terminal.backend().buffer());
            // Four opponent holdings plus two pairs among the five board placeholders.
            assert_eq!(text.matches("[░] [░]").count(), 6, "{width}x{height}");
            assert_eq!(text.matches("FOLDED").count(), 2, "{width}x{height}");
            assert!(text.contains("WAITING"));
            assert!(text.contains("S8 OPEN"));
            assert!(!text.contains(&cards_compact(&view.seats[4].cards)));
        }
        view.seats[0].folded = true;
        let mut terminal = Terminal::new(TestBackend::new(56, 40)).unwrap();
        terminal
            .draw(|frame| render_with_state(frame, &view, 0, None, None))
            .unwrap();
        let text = buffer_text(terminal.backend().buffer());
        assert_eq!(text.matches("[░] [░]").count(), 6);
        assert!(text.contains("FOLDED"));
        assert!(
            !text.contains(" ] [ "),
            "folded hero must not look dealt in"
        );
    }

    #[test]
    fn showdown_sequence_marks_folds_winning_five_chops_and_exact_awards() {
        let mut view = terminal_showdown_view();
        let folded = view
            .seats
            .iter_mut()
            .find(|seat| !seat.winner)
            .expect("three-handed showdown has a displayable non-winner");
        folded.folded = true;
        folded.cards_visible = false;
        folded.showdown_hand = None;
        let folded_cards = cards_compact(&folded.cards);

        let winner = view
            .seats
            .iter()
            .find(|seat| seat.winner)
            .expect("showdown has a winner");
        let best_five = winning_five(
            &winner
                .showdown_hand
                .as_ref()
                .expect("shown winner has an evaluated hand")
                .best_five,
        );
        let awarded = winner.awarded;

        let reveal = render_stage(&view, ShowdownStage::Reveal, 80, 30);
        assert!(reveal.0.contains("REVEALING CARDS"));
        assert!(reveal.0.contains("FOLDED"));
        assert!(!reveal.0.contains(&folded_cards));

        let winners = render_stage(&view, ShowdownStage::Winners, 80, 30);
        assert!(winners.0.contains("WINNING HAND"));
        assert!(winners.0.contains("WINNING HAND"));
        assert!(winners.0.contains(&best_five));
        assert!(
            winners.1 > reveal.1,
            "winner and playing cards are highlighted"
        );

        let award = render_stage(&view, ShowdownStage::Award, 80, 30);
        assert!(award.0.contains("POT AWARDED"));
        assert!(award.0.contains(&format!("+{awarded} CHIPS")));

        let first_hand = view.seats[0].showdown_hand.clone();
        for (index, seat) in view.seats.iter_mut().take(2).enumerate() {
            seat.folded = false;
            seat.cards_visible = true;
            seat.winner = true;
            seat.awarded = 3 + index as u32;
            if seat.showdown_hand.is_none() {
                seat.showdown_hand.clone_from(&first_hand);
            }
        }
        let chop = render_stage(&view, ShowdownStage::Award, 56, 40).0;
        assert!(chop.contains("S0 · +3 CHIPS"));
        assert!(chop.contains("S1 · +4 CHIPS"));
    }

    fn terminal_showdown_view() -> MultiwayReviewView {
        let table_size = TableSize::new(3).unwrap();
        let mut hand = MultiwayHand::new_seeded_for_review(
            table_size,
            SeatId::new(0).unwrap(),
            &[
                (SeatId::new(0).unwrap(), 100),
                (SeatId::new(1).unwrap(), 100),
                (SeatId::new(2).unwrap(), 100),
            ],
            31_415,
        )
        .unwrap();
        while !matches!(
            hand.phase,
            MultiwayPhase::Showdown | MultiwayPhase::HandComplete
        ) {
            let actor = hand.to_act.unwrap();
            let legal = hand.legal_actions_for(actor).unwrap();
            let action = if legal.can_check {
                Action::Check
            } else {
                Action::Call(legal.call_amount.unwrap())
            };
            hand.apply_command(SeatCommand::new(actor, action)).unwrap();
        }
        MultiwayReviewView::from_hand(
            &hand,
            "test",
            "showdown",
            31_415,
            "SHOWDOWN",
            SeatId::new(0).unwrap(),
            Vec::new(),
        )
    }

    #[test]
    fn fold_awards_skip_reveal_and_pending_showdowns_keep_private_labels_readable() {
        let mut hand = MultiwayHand::new_seeded_for_review(
            TableSize::new(3).unwrap(),
            SeatId::new(0).unwrap(),
            &[
                (SeatId::new(0).unwrap(), 100),
                (SeatId::new(1).unwrap(), 100),
                (SeatId::new(2).unwrap(), 100),
            ],
            31_415,
        )
        .unwrap();
        hand.enable_paced_showdown();
        while let Some(actor) = hand.to_act {
            let action =
                crate::network_client::passive_action(&hand.legal_actions_for(actor).unwrap());
            hand.apply_command(SeatCommand::new(actor, action)).unwrap();
        }
        let mut view = MultiwayReviewView::from_hand(
            &hand,
            "test",
            "pending",
            31_415,
            "SHOWDOWN",
            SeatId::new(0).unwrap(),
            Vec::new(),
        );
        for (width, height) in [(80, 30), (72, 32), (64, 36), (56, 40), (120, 40)] {
            let (text, green) = render_stage(&view, ShowdownStage::Winners, width, height);
            assert!(
                text.contains("PRIVATE"),
                "private hero label at {width}x{height}"
            );
            assert!(text.contains("S2 · REVEALING"));
            assert_eq!(green, 0, "pending authority cannot highlight an outcome");
        }
        while hand.advance_showdown() {}
        view = MultiwayReviewView::from_hand(
            &hand,
            "test",
            "mucked",
            31_415,
            "SHOWDOWN",
            SeatId::new(0).unwrap(),
            Vec::new(),
        );
        assert!(render_stage(&view, ShowdownStage::Winners, 56, 40)
            .0
            .contains("MUCKED"));
        view.phase = MultiwayPhase::HandComplete;
        view.showdown_progress = None;
        for stage in [
            ShowdownStage::Reveal,
            ShowdownStage::Winners,
            ShowdownStage::Award,
        ] {
            let text = render_stage(&view, stage, 56, 40).0;
            assert!(text.contains("UNCONTESTED"));
            assert!(!text.contains("SHOWDOWN"));
            assert!(!text.contains("WINNING HAND"));
        }
    }

    #[test]
    fn winning_brackets_are_green_only_after_reveal_for_both_chop_holdings() {
        use crate::game::deck::{Rank, Suit};
        let mut view = terminal_showdown_view();
        view.mucked.clear();
        // Exercise two winning opponents and the narrowest opponent panel,
        // including tens whose closing brackets used to exceed four columns.
        for seat in view.seats.iter_mut().take(2) {
            seat.winner = true;
            seat.status = "LIVE".to_string();
            seat.folded = false;
            seat.cards_visible = true;
            seat.cards = vec![
                Card::new(Rank::Ten, Suit::Spades),
                Card::new(Rank::Ten, Suit::Hearts),
            ];
            seat.showdown_hand = Some(crate::ui::multiway_review::ShowdownHandView {
                description: "Pair of tens".into(),
                best_five: seat
                    .cards
                    .iter()
                    .chain(view.board.iter().take(3))
                    .copied()
                    .collect(),
            });
            for stage in [
                ShowdownStage::Reveal,
                ShowdownStage::Winners,
                ShowdownStage::Award,
            ] {
                let mut terminal = Terminal::new(TestBackend::new(9, 3)).unwrap();
                terminal
                    .draw(|frame| {
                        render_opponent(
                            frame,
                            Some(seat),
                            seat.seat.as_u8(),
                            Rect::new(0, 0, 9, 3),
                            SeatAnchor::BottomLeft,
                            Some(stage),
                        )
                    })
                    .unwrap();
                let green: Vec<_> = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .filter(|cell| cell.fg == WINNER_GREEN)
                    .collect();
                assert_eq!(
                    green.len(),
                    if stage == ShowdownStage::Reveal { 0 } else { 4 }
                );
                assert!(green.iter().all(|cell| matches!(cell.symbol(), "[" | "]")));
            }
        }
        for (width, height) in [(80, 30), (56, 40), (120, 40)] {
            let reveal = render_stage(&view, ShowdownStage::Reveal, width, height);
            let winners = render_stage(&view, ShowdownStage::Winners, width, height);
            assert_eq!(reveal.1, 0);
            assert!(winners.1 >= 8, "both winning holdings at {width}x{height}");
        }
    }

    #[test]
    fn only_best_five_brackets_are_green_for_zero_one_or_two_playing_hole_cards() {
        use crate::game::deck::{Rank::*, Suit::*};
        for local_winner in [false, true] {
            for playing in 0..=2 {
                let mut view = terminal_showdown_view();
                let winner_id = SeatId::new(if local_winner { 0 } else { 1 }).unwrap();
                let (board, holes) = match playing {
                    0 => (
                        vec![
                            Card::new(Ten, Clubs),
                            Card::new(Jack, Clubs),
                            Card::new(Queen, Clubs),
                            Card::new(King, Clubs),
                            Card::new(Ace, Clubs),
                        ],
                        vec![Card::new(Two, Hearts), Card::new(Three, Spades)],
                    ),
                    1 => (
                        vec![
                            Card::new(Nine, Clubs),
                            Card::new(Nine, Spades),
                            Card::new(King, Clubs),
                            Card::new(Queen, Diamonds),
                            Card::new(Two, Hearts),
                        ],
                        vec![Card::new(Ace, Hearts), Card::new(Four, Spades)],
                    ),
                    _ => (
                        vec![
                            Card::new(Two, Clubs),
                            Card::new(Five, Diamonds),
                            Card::new(Seven, Spades),
                            Card::new(Nine, Hearts),
                            Card::new(Jack, Clubs),
                        ],
                        vec![Card::new(Ace, Hearts), Card::new(Ace, Spades)],
                    ),
                };
                let (evaluation, best_five) =
                    crate::game::hand::evaluate_best_five(&holes, &board).unwrap();
                assert_eq!(
                    holes.iter().filter(|c| best_five.contains(c)).count(),
                    playing
                );
                view.board = board;
                view.mucked.clear();
                for seat in &mut view.seats {
                    seat.winner = seat.seat == winner_id;
                    if seat.winner {
                        seat.cards = holes.clone();
                        seat.cards_visible = true;
                        seat.folded = false;
                        seat.status = "SHOW".into();
                        seat.showdown_hand = Some(crate::ui::multiway_review::ShowdownHandView {
                            description: evaluation.description.clone(),
                            best_five: best_five.clone(),
                        });
                    }
                }
                for (width, height) in [(56, 40), (80, 30)] {
                    let (_, green) = render_stage(&view, ShowdownStage::Winners, width, height);
                    assert_eq!(green, 20, "five table cards plus the five-card summary, local={local_winner}, holes={playing}");
                }
            }
        }
    }

    fn render_stage(
        view: &MultiwayReviewView,
        stage: ShowdownStage,
        width: u16,
        height: u16,
    ) -> (String, usize) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_with_state(frame, view, 0, None, Some(stage)))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let highlighted = buffer
            .content
            .iter()
            .filter(|cell| cell.fg == WINNER_GREEN)
            .count();
        (buffer_text(buffer), highlighted)
    }

    fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
        let mut output = String::new();
        for row in 0..buffer.area.height {
            for column in 0..buffer.area.width {
                output.push_str(buffer[(column, row)].symbol());
            }
            output.push('\n');
        }
        output
    }
}
