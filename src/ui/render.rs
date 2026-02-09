use crate::game::deck::Card;
use crate::game::state::{GamePhase, Player, BIG_BLIND};
use crate::stats::models::STAT_DEFINITIONS;
use crate::ui::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
    Frame,
};

// ── Color Palette ──────────────────────────────────────────
const FELT_GREEN: Color = Color::Rgb(0, 80, 40);
const CARD_BG: Color = Color::Rgb(200, 198, 193);
const CARD_RED: Color = Color::Rgb(200, 40, 40);
const CARD_BORDER: Color = Color::Rgb(130, 130, 130);
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
const ACTION_ALLIN: Color = Color::Rgb(200, 100, 220);
const DIM: Color = Color::DarkGray;
const BTN_COLOR: Color = Color::Rgb(220, 160, 40);
const OVERLAY_BG: Color = Color::Rgb(20, 20, 30);
const OVERLAY_BORDER: Color = Color::Rgb(100, 100, 140);

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

// ── Card Rendering (7-wide × 5-tall) ──────────────────────

fn render_card_lines(card: &Card) -> [Line<'static>; 5] {
    let suit_color = if card.suit.is_red() {
        CARD_RED
    } else {
        Color::Rgb(30, 30, 30)
    };
    let border_style = Style::default().fg(CARD_BORDER).bg(CARD_BG);
    let face_style = Style::default()
        .fg(suit_color)
        .bg(CARD_BG)
        .add_modifier(Modifier::BOLD);
    let bg_style = Style::default().bg(CARD_BG);

    let rank = card.rank.symbol();
    let suit = card.suit.symbol();

    [
        Line::from(Span::styled("┌─────┐", border_style)),
        Line::from(vec![
            Span::styled("│", border_style),
            Span::styled("  ", bg_style),
            Span::styled(rank.to_string(), face_style),
            Span::styled("  ", bg_style),
            Span::styled("│", border_style),
        ]),
        Line::from(vec![
            Span::styled("│", border_style),
            Span::styled("     ", bg_style),
            Span::styled("│", border_style),
        ]),
        Line::from(vec![
            Span::styled("│", border_style),
            Span::styled(" ", bg_style),
            Span::styled(format!("{}{}{}", suit, suit, suit), face_style),
            Span::styled(" ", bg_style),
            Span::styled("│", border_style),
        ]),
        Line::from(Span::styled("└─────┘", border_style)),
    ]
}

fn render_facedown_lines() -> [Line<'static>; 5] {
    let border_style = Style::default().fg(CARD_BORDER);
    let back_style = Style::default().fg(CARD_BACK).add_modifier(Modifier::DIM);

    [
        Line::from(Span::styled("┌─────┐", border_style)),
        Line::from(vec![
            Span::styled("│", border_style),
            Span::styled("░░░░░", back_style),
            Span::styled("│", border_style),
        ]),
        Line::from(vec![
            Span::styled("│", border_style),
            Span::styled("░░░░░", back_style),
            Span::styled("│", border_style),
        ]),
        Line::from(vec![
            Span::styled("│", border_style),
            Span::styled("░░░░░", back_style),
            Span::styled("│", border_style),
        ]),
        Line::from(Span::styled("└─────┘", border_style)),
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
            Constraint::Length(1),  // [0]  Status bar
            Constraint::Fill(1),   // [1]  Spacer
            Constraint::Length(1),  // [2]  Opponent label
            Constraint::Fill(1),   // [3]  Spacer
            Constraint::Length(1),  // [4]  Opponent stack
            Constraint::Length(5),  // [5]  Opponent cards
            Constraint::Fill(1),   // [6]  Spacer (opponent cards → table)
            Constraint::Min(12),   // [7]  Board box (protected)
            Constraint::Fill(1),   // [8]  Spacer (table → player label)
            Constraint::Length(1),  // [9]  Player label
            Constraint::Fill(1),   // [10] Spacer
            Constraint::Length(5),  // [11] Player cards
            Constraint::Fill(1),   // [12] Spacer
            Constraint::Length(1),  // [13] Player stack
            Constraint::Fill(1),   // [14] Spacer
            Constraint::Length(1),  // [15] Action bar
            Constraint::Length(1),  // [16] Quick bets / raise input
            Constraint::Min(5),    // [17] Action log (bordered box)
        ])
        .split(inner_area);

    render_status_bar(frame, app, chunks[0]);
    // chunks[1] = spacer
    render_opponent_label(frame, app, chunks[2]);
    // chunks[3] = spacer
    render_opponent_stack(frame, app, chunks[4]);
    render_opponent_cards(frame, app, chunks[5]);
    // chunks[6] = spacer (opponent cards → table)
    render_board_box(frame, app, chunks[7]);
    // chunks[8] = spacer (table → player label)
    render_player_label(frame, app, chunks[9]);
    // chunks[10] = spacer
    render_player_cards(frame, app, chunks[11]);
    // chunks[12] = spacer
    render_player_stack(frame, app, chunks[13]);
    // chunks[14] = spacer
    render_action_bar(frame, app, chunks[15]);
    render_raise_row(frame, app, chunks[16]);
    render_action_log(frame, app, chunks[17]);

    // Overlays (mutually exclusive — stats/help take priority over phase overlays)
    if app.show_stats {
        render_stats_overlay(frame, app);
    } else if app.show_help {
        render_help_overlay(frame);
    } else {
        match app.game_state.phase {
            GamePhase::Showdown => render_showdown_overlay(frame, app),
            GamePhase::SessionEnd => render_session_end_overlay(frame, app),
            GamePhase::Summary => render_summary_overlay(frame, app),
            _ => {}
        }
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
        Style::default()
            .fg(LABEL)
            .add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_opponent_stack(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        format_bb(app.game_state.bot_stack),
        Style::default().fg(GOLD),
    )];

    if app.game_state.button == Player::Bot {
        spans.push(Span::styled(" [D]", Style::default().fg(BTN_COLOR)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

// ── Opponent Cards ─────────────────────────────────────────

fn render_opponent_cards(frame: &mut Frame, app: &App, area: Rect) {
    let card_data: Vec<[Line<'static>; 5]> =
        if matches!(app.game_state.phase, GamePhase::Showdown) {
            app.game_state
                .bot_cards
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
    if bet > 0 {
        let line = Line::from(vec![
            Span::styled("● ", Style::default().fg(GOLD_BRIGHT).bg(FELT_GREEN)),
            Span::styled(format_bb(bet), Style::default().fg(GOLD_BRIGHT).bg(FELT_GREEN)),
        ]);
        frame.render_widget(
            Paragraph::new(line)
                .alignment(Alignment::Center)
                .style(Style::default().bg(FELT_GREEN)),
            area,
        );
    }
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

    // Bet chips (only during active betting phases)
    let show_bets = matches!(
        app.game_state.phase,
        GamePhase::Preflop | GamePhase::Flop | GamePhase::Turn | GamePhase::River
    );
    if show_bets {
        render_bet_chips(frame, app.game_state.bot_bet, inner_chunks[0]);
    }

    // Pot + To Call info line
    let pot_style = Style::default()
        .fg(GOLD_BRIGHT)
        .add_modifier(Modifier::BOLD);

    let mut info_spans: Vec<Span<'static>> = vec![
        Span::styled("POT: ", pot_style),
        Span::styled(format_bb(app.game_state.pot), pot_style),
    ];

    let to_call = app.game_state.amount_to_call(Player::Human);
    if to_call > 0 {
        info_spans.push(Span::raw("          "));
        info_spans.push(Span::styled("To call: ", Style::default().fg(LABEL)));
        info_spans.push(Span::styled(
            format_bb(to_call),
            Style::default().fg(ACTION_CALL).add_modifier(Modifier::BOLD),
        ));
    }

    let info_line = Paragraph::new(Line::from(info_spans)).alignment(Alignment::Center);
    frame.render_widget(info_line, inner_chunks[2]);

    // Community cards
    let board = &app.game_state.board;
    let card_data: Vec<[Line<'static>; 5]> = (0..5)
        .map(|i| {
            if i < board.len() {
                render_card_lines(&board[i])
            } else {
                render_empty_slot_lines()
            }
        })
        .collect();

    let card_lines = compose_card_row(&card_data, " ");
    let paragraph = Paragraph::new(card_lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, inner_chunks[3]);

    if show_bets {
        render_bet_chips(frame, app.game_state.player_bet, inner_chunks[5]);
    }
}

// ── Player Info ────────────────────────────────────────────

fn render_player_label(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        "YOU ",
        Style::default()
            .fg(LABEL)
            .add_modifier(Modifier::BOLD),
    )];

    if app.game_state.is_player_turn() {
        spans.push(Span::styled(
            "★ YOUR TURN ★",
            Style::default()
                .fg(GOLD_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if let Some((ratio, equity)) = app.game_state.pot_odds() {
        spans.push(Span::styled(
            format!("    odds {:.1}:1 need {:.0}%", ratio - 1.0, equity * 100.0),
            Style::default().fg(DIM),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_player_stack(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(
        format_bb(app.game_state.player_stack),
        Style::default().fg(GOLD),
    )];

    if app.game_state.button == Player::Human {
        spans.push(Span::styled(" [D]", Style::default().fg(BTN_COLOR)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

// ── Player Cards ───────────────────────────────────────────

fn render_player_cards(frame: &mut Frame, app: &App, area: Rect) {
    let card_data: Vec<[Line<'static>; 5]> = app
        .game_state
        .player_cards
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
    let is_player_turn = app.game_state.is_player_turn();

    let mut spans: Vec<Span<'static>> = Vec::new();

    if is_player_turn && app.raise_mode {
        // Raise mode replaces the action bar
        render_raise_bar(&mut spans, app, &available);
    } else if is_player_turn {
        if available.can_fold {
            spans.push(Span::styled(
                " F Fold ",
                Style::default().fg(Color::White).bg(ACTION_FOLD),
            ));
            spans.push(Span::raw("   "));
        }
        if available.can_check {
            spans.push(Span::styled(
                " X Check ",
                Style::default().fg(Color::White).bg(ACTION_CHECK),
            ));
            spans.push(Span::raw("   "));
        }
        if let Some(amount) = available.can_call {
            spans.push(Span::styled(
                format!(" C Call {} ", format_bb(amount)),
                Style::default().fg(Color::White).bg(ACTION_CALL),
            ));
            spans.push(Span::raw("   "));
        }
        if available.min_bet.is_some() || available.min_raise.is_some() {
            spans.push(Span::styled(
                " R Raise ",
                Style::default().fg(Color::White).bg(ACTION_RAISE),
            ));
            spans.push(Span::raw("   "));
        }
        spans.push(Span::styled(
            " A All-in ",
            Style::default().fg(Color::White).bg(ACTION_ALLIN),
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
    let min_bb = (min_chips + 1) / 2;
    let pot_bb = app.game_state.pot / 2;
    let stack_bb = (app.game_state.player_bet + app.game_state.player_stack) / 2;

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
            && typed_bb * 2 >= app.game_state.player_bet + app.game_state.player_stack
        {
            spans.push(Span::styled(" (all-in)", Style::default().fg(GOLD)));
        } else if typed_bb > 0 && typed_bb < min_bb {
            spans.push(Span::styled(
                format!(" (min {}BB)", min_bb),
                Style::default().fg(DIM),
            ));
        }
    }

    spans.push(Span::styled(
        "BB",
        Style::default().fg(BRIGHT_WHITE),
    ));

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
                        format!("{:^width$}", entry.text, width = inner.width.saturating_sub(4) as usize),
                        Style::default().fg(LOG_SEPARATOR),
                    ),
                ])
            } else {
                Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("{:>9}", entry.street), Style::default().fg(LOG_STREET)),
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
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::Rgb(180, 180, 180));

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("Actions", section_style)),
        Line::from(vec![
            Span::styled("  F ", key_style),
            Span::styled("Fold   ", desc_style),
            Span::styled("X ", key_style),
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
        Line::from(Span::styled(
            "Press ? to close",
            Style::default().fg(DIM),
        )),
    ];

    let paragraph = Paragraph::new(lines).block(overlay_block("Help"));
    frame.render_widget(paragraph, area);
}

fn render_stats_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(55, 65, frame.area());
    frame.render_widget(Clear, area);

    let stats = &app.game_state;
    let section_style = Style::default().fg(GOLD).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::Rgb(180, 180, 180));
    let value_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

    let win_rate = if stats.hands_played > 0 {
        stats.hands_won as f64 / stats.hands_played as f64 * 100.0
    } else {
        0.0
    };
    let profit = stats.session_profit_bb();
    let profit_color = if profit > 0.0 {
        ACTION_CHECK
    } else if profit < 0.0 {
        ACTION_FOLD
    } else {
        Color::White
    };

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled("Session", section_style)),
        Line::from(vec![
            Span::styled("  Hands: ", label_style),
            Span::styled(format!("{}", stats.hands_played), value_style),
            Span::styled("   Won: ", label_style),
            Span::styled(format!("{}", stats.hands_won), value_style),
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

fn render_showdown_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(55, 65, frame.area());
    frame.render_widget(Clear, area);

    let mut lines: Vec<Line<'static>> = Vec::new();

    if let Some(ref result) = app.game_state.showdown_result {
        // Result line
        let (result_text, result_color) = match result.winner {
            Some(Player::Human) => (
                format!("You win {}!", format_bb(result.pot_won)),
                ACTION_CHECK,
            ),
            Some(Player::Bot) => (
                format!("Bot wins {}", format_bb(result.pot_won)),
                ACTION_FOLD,
            ),
            None => (
                format!("Split pot — {}", format_bb(result.pot_won)),
                GOLD,
            ),
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            result_text,
            Style::default()
                .fg(result_color)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));

        // Your hand
        lines.push(Line::from(vec![
            Span::styled(
                "Your hand: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                result.player_hand.description.clone(),
                Style::default().fg(ACTION_CALL),
            ),
        ]));

        let player_card_data: Vec<[Line<'static>; 5]> = app
            .game_state
            .player_cards
            .iter()
            .map(|c| render_card_lines(c))
            .collect();
        lines.extend(compose_card_row(&player_card_data, " "));
        lines.push(Line::from(""));

        // Bot hand
        lines.push(Line::from(vec![
            Span::styled(
                "Bot's hand: ",
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                result.bot_hand.description.clone(),
                Style::default().fg(ACTION_CALL),
            ),
        ]));

        let bot_card_data: Vec<[Line<'static>; 5]> = app
            .game_state
            .bot_cards
            .iter()
            .map(|c| render_card_lines(c))
            .collect();
        lines.extend(compose_card_row(&bot_card_data, " "));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "[Space/Enter] Continue",
        Style::default().fg(DIM),
    )));

    let paragraph = Paragraph::new(lines)
        .block(overlay_block("Showdown"))
        .alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_session_end_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, frame.area());
    frame.render_widget(Clear, area);

    let winner = if app.game_state.player_stack == 0 {
        "You busted!"
    } else {
        "Bot busted! You win!"
    };
    let winner_color = if app.game_state.player_stack == 0 {
        ACTION_FOLD
    } else {
        ACTION_CHECK
    };

    let section_style = Style::default().fg(GOLD).add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(Color::Rgb(180, 180, 180));
    let value_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

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
            Span::styled(format!("{}", app.game_state.hands_won), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Biggest pot won: ", label_style),
            Span::styled(format_bb(app.game_state.biggest_pot_won), value_style),
        ]),
        Line::from(vec![
            Span::styled("  Biggest pot lost: ", label_style),
            Span::styled(format_bb(app.game_state.biggest_pot_lost), value_style),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                " N New Session ",
                Style::default().fg(Color::White).bg(ACTION_CHECK),
            ),
            Span::raw("   "),
            Span::styled(
                " Q Quit ",
                Style::default().fg(Color::White).bg(ACTION_FOLD),
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

    let profit = app.game_state.session_profit_bb();
    let profit_color = if profit > 0.0 {
        ACTION_CHECK
    } else if profit < 0.0 {
        ACTION_FOLD
    } else {
        Color::White
    };

    let label_style = Style::default().fg(Color::Rgb(180, 180, 180));
    let value_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);

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
            Span::styled(format!("{}", app.game_state.hands_won), value_style),
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
