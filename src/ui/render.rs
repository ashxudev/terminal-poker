use crate::game::deck::Card;
use crate::game::state::{GamePhase, Player, BIG_BLIND};
use crate::stats::models::STAT_DEFINITIONS;
use crate::ui::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub fn render(frame: &mut Frame, app: &App) {
    let size = frame.area();

    // Main layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(3),  // Bot info
            Constraint::Length(5),  // Board
            Constraint::Length(3),  // Player cards
            Constraint::Length(3),  // Player info
            Constraint::Length(5),  // Actions / Pot odds
            Constraint::Min(1),     // Message
        ])
        .split(size);

    // Header
    let header = Paragraph::new("Terminal Poker - Heads-Up No-Limit Hold'em")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(header, chunks[0]);

    // Bot info
    render_bot_info(frame, app, chunks[1]);

    // Board
    render_board(frame, app, chunks[2]);

    // Player cards
    render_player_cards(frame, app, chunks[3]);

    // Player info
    render_player_info(frame, app, chunks[4]);

    // Actions and pot odds
    render_actions(frame, app, chunks[5]);

    // Message
    render_message(frame, app, chunks[6]);

    // Overlays
    if app.show_stats {
        render_stats_overlay(frame, app);
    }

    if app.show_help {
        render_help_overlay(frame);
    }

    // Phase-specific overlays
    match app.game_state.phase {
        GamePhase::Showdown => render_showdown_overlay(frame, app),
        GamePhase::SessionEnd => render_session_end_overlay(frame, app),
        GamePhase::Summary => render_summary_overlay(frame, app),
        _ => {}
    }
}

fn render_bot_info(frame: &mut Frame, app: &App, area: Rect) {
    let bot_stack_bb = app.game_state.bot_stack as f64 / BIG_BLIND as f64;
    let bot_bet = app.game_state.bot_bet;

    let mut spans = vec![
        Span::styled("Bot: ", Style::default().fg(Color::White)),
        Span::styled(
            format!("{:.0}BB", bot_stack_bb),
            Style::default().fg(Color::Yellow),
        ),
    ];

    if bot_bet > 0 {
        spans.push(Span::raw("  |  Bet: "));
        spans.push(Span::styled(
            format!("{}", bot_bet),
            Style::default().fg(Color::Red),
        ));
    }

    if app.game_state.button == Player::Bot {
        spans.push(Span::styled(
            "  [BTN]",
            Style::default().fg(Color::Magenta),
        ));
    }

    // Show bot cards face down or revealed at showdown
    spans.push(Span::raw("  "));
    if matches!(app.game_state.phase, GamePhase::Showdown) {
        for card in &app.game_state.bot_cards {
            spans.push(card_span(card));
            spans.push(Span::raw(" "));
        }
    } else {
        spans.push(Span::styled("[??]", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled("[??]", Style::default().fg(Color::DarkGray)));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_board(frame: &mut Frame, app: &App, area: Rect) {
    let pot = app.game_state.pot;
    let board = &app.game_state.board;

    let mut lines = vec![Line::from(vec![
        Span::raw("Pot: "),
        Span::styled(format!("{}", pot), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ])];

    // Board cards
    let mut card_spans = vec![];
    for i in 0..5 {
        if i < board.len() {
            card_spans.push(card_span(&board[i]));
        } else {
            card_spans.push(Span::styled("[  ]", Style::default().fg(Color::DarkGray)));
        }
        card_spans.push(Span::raw(" "));
    }

    lines.push(Line::from(card_spans));

    let paragraph = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Board"));
    frame.render_widget(paragraph, area);
}

fn render_player_cards(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![Span::raw("Your hand: ")];
    for card in &app.game_state.player_cards {
        spans.push(card_span(card));
        spans.push(Span::raw(" "));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_player_info(frame: &mut Frame, app: &App, area: Rect) {
    let player_stack_bb = app.game_state.player_stack as f64 / BIG_BLIND as f64;
    let player_bet = app.game_state.player_bet;
    let to_call = app.game_state.amount_to_call(Player::Human);

    let mut spans = vec![
        Span::styled("You: ", Style::default().fg(Color::White)),
        Span::styled(
            format!("{:.0}BB", player_stack_bb),
            Style::default().fg(Color::Yellow),
        ),
    ];

    if player_bet > 0 {
        spans.push(Span::raw("  |  Bet: "));
        spans.push(Span::styled(
            format!("{}", player_bet),
            Style::default().fg(Color::Red),
        ));
    }

    if to_call > 0 {
        spans.push(Span::raw("  |  To call: "));
        spans.push(Span::styled(
            format!("{}", to_call),
            Style::default().fg(Color::Cyan),
        ));
    }

    if app.game_state.button == Player::Human {
        spans.push(Span::styled(
            "  [BTN]",
            Style::default().fg(Color::Magenta),
        ));
    }

    let paragraph = Paragraph::new(Line::from(spans)).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_actions(frame: &mut Frame, app: &App, area: Rect) {
    let available = app.game_state.available_actions();
    let is_player_turn = app.game_state.is_player_turn();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Actions
    let mut action_spans = vec![];

    if is_player_turn {
        if available.can_fold {
            action_spans.push(Span::styled("[F]", Style::default().fg(Color::Red)));
            action_spans.push(Span::raw("old  "));
        }
        if available.can_check {
            action_spans.push(Span::styled("[X]", Style::default().fg(Color::Green)));
            action_spans.push(Span::raw("Check  "));
        }
        if available.can_call.is_some() {
            action_spans.push(Span::styled("[C]", Style::default().fg(Color::Cyan)));
            action_spans.push(Span::raw("all  "));
        }
        if available.min_bet.is_some() || available.min_raise.is_some() {
            action_spans.push(Span::styled("[R]", Style::default().fg(Color::Yellow)));
            action_spans.push(Span::raw("aise  "));
        }
        action_spans.push(Span::styled("[A]", Style::default().fg(Color::Magenta)));
        action_spans.push(Span::raw("ll-in  "));

        // Raise input
        if !app.raise_input.is_empty() {
            action_spans.push(Span::raw(" | Amount: "));
            action_spans.push(Span::styled(
                &app.raise_input,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ));
        }
    } else {
        action_spans.push(Span::styled(
            "Waiting for bot...",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let actions = Paragraph::new(Line::from(action_spans))
        .block(Block::default().borders(Borders::ALL).title("Actions"));
    frame.render_widget(actions, chunks[0]);

    // Pot odds
    let mut odds_lines = vec![];
    if let Some((ratio, equity)) = app.game_state.pot_odds() {
        odds_lines.push(Line::from(vec![
            Span::raw("Pot odds: "),
            Span::styled(
                format!("{:.1}:1", ratio - 1.0),
                Style::default().fg(Color::Cyan),
            ),
        ]));
        odds_lines.push(Line::from(vec![
            Span::raw("Need: "),
            Span::styled(
                format!("{:.0}%", equity * 100.0),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" equity"),
        ]));
    } else {
        odds_lines.push(Line::from(Span::styled(
            "No bet to call",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let pot_odds = Paragraph::new(odds_lines)
        .block(Block::default().borders(Borders::ALL).title("Pot Odds"));
    frame.render_widget(pot_odds, chunks[1]);
}

fn render_message(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![];

    if let Some(ref msg) = app.message {
        lines.push(Line::from(Span::styled(
            msg.as_str(),
            Style::default().fg(Color::White),
        )));
    }

    lines.push(Line::from(vec![
        Span::styled("[S]", Style::default().fg(Color::Blue)),
        Span::raw("tats  "),
        Span::styled("[?]", Style::default().fg(Color::Blue)),
        Span::raw("Help  "),
        Span::styled("[Q]", Style::default().fg(Color::Red)),
        Span::raw("uit"),
    ]));

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_stats_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 60, frame.area());
    frame.render_widget(Clear, area);

    let stats = &app.game_state;
    let mut lines = vec![
        Line::from(Span::styled(
            "Session Statistics",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("Hands played: {}", stats.hands_played)),
        Line::from(format!("Hands won: {}", stats.hands_won)),
        Line::from(format!(
            "Win rate: {:.0}%",
            if stats.hands_played > 0 {
                stats.hands_won as f64 / stats.hands_played as f64 * 100.0
            } else {
                0.0
            }
        )),
        Line::from(format!("Session P/L: {:.1}BB", stats.session_profit_bb())),
        Line::from(""),
    ];

    if app.show_help {
        // Show stat explanations
        lines.push(Line::from(Span::styled(
            "Stat Definitions:",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for def in STAT_DEFINITIONS {
            lines.push(Line::from(format!("{}: {}", def.abbrev, def.explanation)));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press [?] to toggle explanations",
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(Span::styled(
        "Press [S] to close",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .title("Stats")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn render_help_overlay(frame: &mut Frame) {
    let area = centered_rect(60, 70, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(Span::styled(
            "Controls",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("[F] - Fold"),
        Line::from("[X] - Check"),
        Line::from("[C] - Call"),
        Line::from("[R] - Raise (type amount, then R or Enter)"),
        Line::from("[A] - All-in"),
        Line::from(""),
        Line::from(Span::styled(
            "Quick Bet Sizes (when no amount typed):",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("[1] - 33% pot"),
        Line::from("[2] - 50% pot"),
        Line::from("[3] - 67% pot"),
        Line::from("[4] - 100% pot"),
        Line::from(""),
        Line::from(Span::styled(
            "Other:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from("[S] - Toggle stats"),
        Line::from("[?] - Toggle this help"),
        Line::from("[Q] - Quit (shows summary)"),
        Line::from("Ctrl+C - Quit immediately"),
        Line::from(""),
        Line::from(Span::styled(
            "Press [?] to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

fn render_showdown_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, frame.area());
    frame.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(Span::styled(
            "SHOWDOWN",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )),
        Line::from(""),
    ];

    if let Some(ref result) = app.game_state.showdown_result {
        // Your hand
        let mut your_hand = vec![Span::raw("Your hand: ")];
        for card in &app.game_state.player_cards {
            your_hand.push(card_span(card));
            your_hand.push(Span::raw(" "));
        }
        your_hand.push(Span::raw(" - "));
        your_hand.push(Span::styled(
            &result.player_hand.description,
            Style::default().fg(Color::Cyan),
        ));
        lines.push(Line::from(your_hand));

        // Bot hand
        let mut bot_hand = vec![Span::raw("Bot hand:  ")];
        for card in &app.game_state.bot_cards {
            bot_hand.push(card_span(card));
            bot_hand.push(Span::raw(" "));
        }
        bot_hand.push(Span::raw(" - "));
        bot_hand.push(Span::styled(
            &result.bot_hand.description,
            Style::default().fg(Color::Cyan),
        ));
        lines.push(Line::from(bot_hand));

        lines.push(Line::from(""));

        // Winner
        let winner_text = match result.winner {
            Some(Player::Human) => format!("You win {} chips!", result.pot_won),
            Some(Player::Bot) => format!("Bot wins {} chips", result.pot_won),
            None => format!("Split pot - {} each", result.pot_won / 2),
        };
        let winner_color = match result.winner {
            Some(Player::Human) => Color::Green,
            Some(Player::Bot) => Color::Red,
            None => Color::Yellow,
        };
        lines.push(Line::from(Span::styled(
            winner_text,
            Style::default().fg(winner_color).add_modifier(Modifier::BOLD),
        )));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Press any key to continue",
        Style::default().fg(Color::DarkGray),
    )));

    let block = Block::default()
        .title("Result")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block).alignment(Alignment::Center);
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

    let lines = vec![
        Line::from(Span::styled(
            "SESSION COMPLETE",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        )),
        Line::from(""),
        Line::from(Span::styled(
            winner,
            Style::default().fg(if app.game_state.player_stack == 0 {
                Color::Red
            } else {
                Color::Green
            }),
        )),
        Line::from(""),
        Line::from(format!("Hands played: {}", app.game_state.hands_played)),
        Line::from(format!("Hands won: {}", app.game_state.hands_won)),
        Line::from(format!("Biggest pot won: {}", app.game_state.biggest_pot_won)),
        Line::from(format!("Biggest pot lost: {}", app.game_state.biggest_pot_lost)),
        Line::from(""),
        Line::from(vec![
            Span::styled("[N]", Style::default().fg(Color::Green)),
            Span::raw("ew session  "),
            Span::styled("[Q]", Style::default().fg(Color::Red)),
            Span::raw("uit"),
        ]),
    ];

    let block = Block::default()
        .title("Game Over")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn render_summary_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, frame.area());
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(Span::styled(
            "SESSION SUMMARY",
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        )),
        Line::from(""),
        Line::from(format!("Hands played: {}", app.game_state.hands_played)),
        Line::from(format!("Hands won: {}", app.game_state.hands_won)),
        Line::from(format!(
            "Session P/L: {:.1}BB",
            app.game_state.session_profit_bb()
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press any key to exit",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title("Summary")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block).alignment(Alignment::Center);
    frame.render_widget(paragraph, area);
}

fn card_span(card: &Card) -> Span<'static> {
    let color = if card.suit.is_red() {
        Color::Red
    } else {
        Color::White
    };

    Span::styled(
        format!("[{}{}]", card.rank.symbol(), card.suit.symbol()),
        Style::default().fg(color),
    )
}

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
