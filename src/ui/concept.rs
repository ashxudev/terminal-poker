//! Deterministic 120x40 concept fixtures for the future Sneaky Blinders shell.
//!
//! These screens are design evidence, not wired production routes. They deliberately
//! use Ratatui widgets and a `TestBackend`, so every character, color, and protected
//! value is reproducible and constrained by the same cell grid as the product.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};

pub const CONCEPT_WIDTH: u16 = 120;
pub const CONCEPT_HEIGHT: u16 = 40;

const SCREEN: Color = Color::Rgb(5, 11, 19);
const PANEL: Color = Color::Rgb(10, 22, 34);
const PANEL_2: Color = Color::Rgb(14, 31, 47);
const FELT: Color = Color::Rgb(7, 43, 45);
const BORDER: Color = Color::Rgb(49, 77, 96);
const TEXT: Color = Color::Rgb(229, 237, 240);
const MUTED: Color = Color::Rgb(133, 157, 171);
const CYAN: Color = Color::Rgb(36, 206, 220);
const BLUE: Color = Color::Rgb(47, 112, 219);
const GREEN: Color = Color::Rgb(43, 190, 112);
const AMBER: Color = Color::Rgb(243, 174, 54);
const GOLD_BRIGHT: Color = Color::LightYellow;
const RED: Color = Color::Rgb(224, 74, 90);
const VIOLET: Color = Color::Rgb(161, 98, 232);
const WHITE: Color = Color::Rgb(248, 249, 245);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConceptScreen {
    Home,
    Practice,
    Host,
    HostLobby,
    Join,
    PublicLobby,
    NineSeatTable,
    TableHud,
    Study,
    RangeExplorer,
    HandReplay,
    Settings,
    Help,
}

impl ConceptScreen {
    pub const ALL: [Self; 13] = [
        Self::Home,
        Self::Practice,
        Self::Host,
        Self::HostLobby,
        Self::Join,
        Self::PublicLobby,
        Self::NineSeatTable,
        Self::TableHud,
        Self::Study,
        Self::RangeExplorer,
        Self::HandReplay,
        Self::Settings,
        Self::Help,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Home => "01-home",
            Self::Practice => "02-practice",
            Self::Host => "03-host",
            Self::HostLobby => "04-host-lobby",
            Self::Join => "05-join",
            Self::PublicLobby => "06-public-lobby",
            Self::NineSeatTable => "07-nine-seat-table",
            Self::TableHud => "08-table-hud",
            Self::Study => "09-study",
            Self::RangeExplorer => "10-range-explorer",
            Self::HandReplay => "11-hand-replay",
            Self::Settings => "12-settings",
            Self::Help => "13-help",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Home => "HOME",
            Self::Practice => "PRACTICE",
            Self::Host => "HOST / CREATE GAME",
            Self::HostLobby => "HOST / WAITING ROOM",
            Self::Join => "JOIN A PRIVATE GAME",
            Self::PublicLobby => "PUBLIC TABLES",
            Self::NineSeatTable => "LIVE TABLE",
            Self::TableHud => "LIVE TABLE / HUD CONCEPT",
            Self::Study => "STUDY",
            Self::RangeExplorer => "STUDY / RANGE EXPLORER",
            Self::HandReplay => "STUDY / HAND REPLAY",
            Self::Settings => "SETTINGS",
            Self::Help => "HELP",
        }
    }

    pub const fn footer(self) -> &'static str {
        match self {
            Self::Home => "↑↓ move   enter select   ? help   q quit",
            Self::Practice | Self::Host | Self::Join => {
                "tab next field   space change   enter continue   esc back"
            }
            Self::HostLobby => "c copy invite   enter start when ready   esc cancel",
            Self::PublicLobby => "/ filter   ↑↓ table   enter inspect   j join   esc back",
            Self::NineSeatTable | Self::TableHud => {
                "f fold   c call   r raise   a all-in   h history   ? help"
            }
            Self::Study => "/ filter   ↑↓ hand   enter replay   r ranges   esc home",
            Self::RangeExplorer => {
                "←→ position   ↑↓ hand   tab scenario   enter detail   esc study"
            }
            Self::HandReplay => "← previous   → next   space play/pause   n note   esc study",
            Self::Settings => "↑↓ setting   ←→ change   space toggle   esc save & back",
            Self::Help => "tab section   / search   esc close   q quit safely",
        }
    }
}

pub fn render_concept(frame: &mut Frame<'_>, screen: ConceptScreen) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(SCREEN)), area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);
    render_header(frame, rows[0], screen.title());
    match screen {
        ConceptScreen::Home => render_home(frame, rows[1]),
        ConceptScreen::Practice => render_practice(frame, rows[1]),
        ConceptScreen::Host => render_host(frame, rows[1]),
        ConceptScreen::HostLobby => render_host_lobby(frame, rows[1]),
        ConceptScreen::Join => render_join(frame, rows[1]),
        ConceptScreen::PublicLobby => render_public_lobby(frame, rows[1]),
        ConceptScreen::NineSeatTable => render_table(frame, rows[1], false),
        ConceptScreen::TableHud => render_table(frame, rows[1], true),
        ConceptScreen::Study => render_study(frame, rows[1]),
        ConceptScreen::RangeExplorer => render_range(frame, rows[1]),
        ConceptScreen::HandReplay => render_replay(frame, rows[1]),
        ConceptScreen::Settings => render_settings(frame, rows[1]),
        ConceptScreen::Help => render_help(frame, rows[1]),
    }
    render_footer(frame, rows[2], screen.footer());
}

/// A table-first visual revision that preserves Ash's original physical-card
/// language while adapting it to a stable nine-seat perimeter.
pub fn render_ash_continuity_mockup(frame: &mut Frame<'_>) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Black)),
        area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " SNEAKY BLINDERS",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  ·  DOCKSIDE NINE  ·  HAND #184  ·  1/2  ·  PLAY MONEY",
                Style::default().fg(MUTED),
            ),
        ])),
        Rect::new(0, 0, 91, 1),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("● ", Style::default().fg(GREEN)),
            Span::styled(
                "CONNECTED",
                Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Right),
        Rect::new(92, 0, 27, 1),
    );
    frame.buffer_mut().set_string(
        0,
        1,
        "─".repeat(120),
        Style::default().fg(Color::Rgb(45, 55, 62)),
    );

    let felt = Rect::new(20, 7, 80, 19);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(48, 128, 75)))
            .style(Style::default().bg(Color::Rgb(0, 78, 38))),
        felt,
    );

    draw_seat_tag(
        frame,
        Rect::new(34, 2, 20, 2),
        "S4 · UTG+1 · IVO",
        "74 BB",
        "",
        Alignment::Center,
        false,
    );
    draw_mini_back_pair(frame, 39, 4, false);
    draw_seat_tag(
        frame,
        Rect::new(65, 2, 20, 2),
        "S5 · LJ · KIT",
        "116 BB",
        "BET 6",
        Alignment::Center,
        true,
    );
    draw_mini_back_pair(frame, 70, 4, false);

    draw_seat_tag(
        frame,
        Rect::new(2, 7, 17, 3),
        "S3 · UTG · NOOR",
        "132 BB",
        "FOLD",
        Alignment::Left,
        false,
    );
    draw_mini_back_pair(frame, 5, 10, true);
    draw_seat_tag(
        frame,
        Rect::new(101, 7, 18, 3),
        "S6 · HJ · RUE",
        "64 BB",
        "",
        Alignment::Right,
        false,
    );
    draw_mini_back_pair(frame, 106, 10, false);

    draw_seat_tag(
        frame,
        Rect::new(1, 15, 18, 3),
        "S2 · BB · ELI",
        "88 BB",
        "CALL 6",
        Alignment::Left,
        true,
    );
    draw_mini_back_pair(frame, 4, 18, false);
    draw_seat_tag(
        frame,
        Rect::new(101, 15, 18, 3),
        "S7 · CO · ASH",
        "143 BB",
        "FOLD",
        Alignment::Right,
        false,
    );
    draw_mini_back_pair(frame, 106, 18, true);

    draw_seat_tag(
        frame,
        Rect::new(5, 23, 18, 3),
        "S1 · SB · MARA",
        "101 BB",
        "",
        Alignment::Left,
        false,
    );
    draw_mini_back_pair(frame, 9, 26, false);
    draw_seat_tag(
        frame,
        Rect::new(99, 23, 20, 3),
        "S8 · OPEN",
        "—",
        "[ SIT HERE ]",
        Alignment::Right,
        false,
    );

    frame.render_widget(
        Paragraph::new(Span::styled(
            "POT  18 BB",
            Style::default()
                .fg(GOLD_BRIGHT)
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Rect::new(45, 9, 30, 1),
    );
    draw_big_card(frame, 40, 11, "A", "♠", false);
    draw_big_card(frame, 48, 11, "J", "♥", true);
    draw_big_card(frame, 56, 11, "7", "♣", false);
    draw_empty_big_card(frame, 64, 11);
    draw_empty_big_card(frame, 72, 11);
    frame.render_widget(
        Paragraph::new(Span::styled(
            "FLOP  ·  TO CALL 6 BB",
            Style::default().fg(TEXT),
        ))
        .alignment(Alignment::Center),
        Rect::new(42, 17, 36, 1),
    );

    draw_chip(frame, 71, 8, "6 BB");
    draw_chip(frame, 22, 17, "6 BB");
    draw_chip(frame, 91, 23, "1 BB");

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "S0 · YOU  ",
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "96 BB  [D]",
                Style::default()
                    .fg(GOLD_BRIGHT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Center),
        Rect::new(45, 20, 30, 1),
    );
    draw_big_card(frame, 52, 22, "Q", "♠", false);
    draw_big_card(frame, 61, 22, "Q", "♦", true);

    let actions = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(21),
            Constraint::Percentage(25),
            Constraint::Percentage(32),
            Constraint::Percentage(22),
        ])
        .split(Rect::new(16, 29, 88, 3));
    let action_area = |area: Rect| Rect::new(area.x + 1, area.y, area.width - 2, area.height);
    draw_luminous_action(frame, action_area(actions[0]), "[ F ]  FOLD", RED, false);
    draw_luminous_action(
        frame,
        action_area(actions[1]),
        "[ C ]  CALL 6",
        Color::Rgb(55, 170, 215),
        false,
    );
    draw_luminous_action(
        frame,
        action_area(actions[2]),
        "[ R ]  RAISE TO 18",
        AMBER,
        true,
    );
    draw_luminous_action(
        frame,
        action_area(actions[3]),
        "[ A ]  ALL-IN",
        VIOLET,
        false,
    );

    frame.buffer_mut().set_string(
        16,
        33,
        "─".repeat(88),
        Style::default().fg(Color::Rgb(45, 55, 62)),
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("FLOP", Style::default().fg(MUTED)),
                Span::styled("  KIT bets 6 BB", Style::default().fg(TEXT)),
                Span::styled("  ·  ELI calls 6 BB", Style::default().fg(TEXT)),
            ]),
            Line::from(vec![
                Span::styled(
                    "NOW ",
                    Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "YOU to act  ·  22s",
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
                Span::styled("      h history   ? help", Style::default().fg(MUTED)),
            ]),
        ])
        .alignment(Alignment::Center),
        Rect::new(14, 35, 92, 2),
    );
}

fn draw_big_card(frame: &mut Frame<'_>, x: u16, y: u16, rank: &str, suit: &str, red: bool) {
    let face = Style::default().bg(Color::Rgb(214, 213, 209));
    let ink = Style::default()
        .fg(if red {
            Color::Rgb(200, 40, 40)
        } else {
            Color::Rgb(30, 30, 30)
        })
        .bg(Color::Rgb(214, 213, 209))
        .add_modifier(Modifier::BOLD);
    for row in 0..5 {
        frame.buffer_mut().set_string(x, y + row, "       ", face);
    }
    frame.buffer_mut().set_string(x + 1, y + 1, rank, ink);
    frame.buffer_mut().set_string(x + 3, y + 2, suit, ink);
    frame.buffer_mut().set_string(x + 5, y + 3, rank, ink);
}

fn draw_empty_big_card(frame: &mut Frame<'_>, x: u16, y: u16) {
    let style = Style::default()
        .fg(Color::Rgb(110, 125, 115))
        .bg(Color::Rgb(0, 78, 38));
    for (row, line) in ["┌╌╌╌╌╌┐", "╎     ╎", "╎     ╎", "╎     ╎", "└╌╌╌╌╌┘"]
        .iter()
        .enumerate()
    {
        frame
            .buffer_mut()
            .set_string(x, y + row as u16, *line, style);
    }
}

fn draw_mini_back_pair(frame: &mut Frame<'_>, x: u16, y: u16, folded: bool) {
    let bg = if folded {
        Color::Rgb(37, 37, 69)
    } else {
        Color::Rgb(60, 60, 120)
    };
    let fg = if folded {
        Color::Rgb(70, 70, 105)
    } else {
        Color::Rgb(120, 120, 185)
    };
    for offset in [0, 6] {
        for row in 0..3 {
            frame
                .buffer_mut()
                .set_string(x + offset, y + row, "     ", Style::default().bg(bg));
        }
        frame
            .buffer_mut()
            .set_string(x + offset, y + 1, " ✦ ✦", Style::default().fg(fg).bg(bg));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_seat_tag(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    stack: &str,
    status: &str,
    alignment: Alignment,
    active: bool,
) {
    let mut lines = vec![
        Line::from(Span::styled(
            label.to_string(),
            Style::default()
                .fg(if active { WHITE } else { TEXT })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            stack.to_string(),
            Style::default()
                .fg(GOLD_BRIGHT)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    if area.height > 2 && !status.is_empty() {
        lines.push(Line::from(Span::styled(
            status.to_string(),
            Style::default().fg(if active { CYAN } else { MUTED }),
        )));
    }
    frame.render_widget(Paragraph::new(lines).alignment(alignment), area);
}

fn draw_chip(frame: &mut Frame<'_>, x: u16, y: u16, amount: &str) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("●", Style::default().fg(WHITE).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" {amount}"), Style::default().fg(GOLD_BRIGHT)),
        ]))
        .style(Style::default().bg(Color::Rgb(0, 78, 38))),
        Rect::new(x, y, 9, 1),
    );
}

fn draw_luminous_action(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    color: Color,
    selected: bool,
) {
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(if selected { Color::Black } else { color })
                    .bg(if selected { color } else { Color::Black })
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color)),
            ),
        area,
    );
}

fn render_header(frame: &mut Frame<'_>, area: Rect, breadcrumb: &str) {
    frame.render_widget(Block::default().style(Style::default().bg(PANEL_2)), area);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " SNEAKY BLINDERS ",
                Style::default()
                    .fg(SCREEN)
                    .bg(AMBER)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  /  {breadcrumb}"), Style::default().fg(TEXT)),
        ])),
        Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(35), 1),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("● ", Style::default().fg(GREEN)),
            Span::styled("LOCAL / PLAY MONEY", Style::default().fg(MUTED)),
        ]))
        .alignment(Alignment::Right),
        Rect::new(area.right().saturating_sub(30), area.y + 1, 28, 1),
    );
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, hints: &str) {
    frame.render_widget(Block::default().style(Style::default().bg(PANEL_2)), area);
    frame.render_widget(
        Paragraph::new(hints)
            .style(Style::default().fg(MUTED))
            .alignment(Alignment::Center),
        Rect::new(area.x, area.y, area.width, 1),
    );
    frame.render_widget(
        Paragraph::new("concept fixture · 120×40 · keyboard first")
            .style(Style::default().fg(BORDER))
            .alignment(Alignment::Center),
        Rect::new(area.x, area.y + 1, area.width, 1),
    );
}

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL).fg(TEXT))
}

fn selected_panel(title: &str) -> Block<'_> {
    panel(title).border_style(Style::default().fg(CYAN))
}

fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    Rect::new(
        area.x + horizontal,
        area.y + vertical,
        area.width.saturating_sub(horizontal * 2),
        area.height.saturating_sub(vertical * 2),
    )
}

fn render_home(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(47), Constraint::Percentage(53)])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "  S N E A K Y",
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "  B L I N D E R S",
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  ONE COMMAND. EVERY TABLE.",
                Style::default().fg(CYAN),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Private ring poker for 2–9 players",
                Style::default().fg(TEXT),
            )),
            Line::from(Span::styled(
                "  with practice and study built in.",
                Style::default().fg(MUTED),
            )),
        ])
        .block(panel("WELCOME"))
        .wrap(Wrap { trim: false }),
        cols[0],
    );
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(21), Constraint::Min(8)])
        .split(cols[1]);
    let menu = [
        ("▶  PRACTICE", "Play immediately against configurable bots"),
        ("   HOST", "Create a private or public ring table"),
        ("   JOIN", "Paste an invite or browse public tables"),
        ("   STUDY", "Review hands, ranges, and decisions"),
        ("   SETTINGS", "Theme, motion, controls, and privacy"),
    ];
    let mut lines = Vec::new();
    for (index, (label, description)) in menu.iter().enumerate() {
        let color = if index == 0 { AMBER } else { TEXT };
        lines.push(Line::from(Span::styled(
            format!(" {label:<18}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(
            format!("    {description}"),
            Style::default().fg(MUTED),
        )));
        if index != menu.len() - 1 {
            lines.push(Line::from(""));
        }
    }
    frame.render_widget(
        Paragraph::new(lines).block(selected_panel("PLAY")),
        right[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "CONTINUE  ",
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                ),
                Span::raw("Dockside Nine"),
            ]),
            Line::from(Span::styled(
                "9-max · 100 BB · waiting for next hand",
                Style::default().fg(MUTED),
            )),
            Line::from(Span::styled(
                "No active hand is abandoned when opening Help or Settings.",
                Style::default().fg(TEXT),
            )),
        ])
        .block(panel("ACTIVE SESSION"))
        .wrap(Wrap { trim: true }),
        right[1],
    );
}

fn field(label: &str, value: &str, focused: bool) -> Line<'static> {
    let marker = if focused { "▶" } else { " " };
    let style = if focused {
        Style::default().fg(AMBER).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT)
    };
    Line::from(vec![
        Span::styled(format!("{marker} {label:<19}"), style),
        Span::styled(
            format!(" {value:<24} "),
            Style::default().fg(WHITE).bg(PANEL_2),
        ),
    ])
}

fn render_practice(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            field("Preset", "QUICK PRACTICE", true),
            Line::from(""),
            field("Players", "6-max", false),
            Line::from(""),
            field("Starting stack", "100 BB", false),
            Line::from(""),
            field("Blinds", "1 / 2 chips", false),
            Line::from(""),
            field("Bot profile", "Balanced · Medium", false),
            Line::from(""),
            field("Seat", "Random", false),
            Line::from(""),
            Line::from(Span::styled(
                "  [ ENTER ]  START PRACTICE",
                Style::default()
                    .fg(SCREEN)
                    .bg(GREEN)
                    .add_modifier(Modifier::BOLD),
            )),
        ])
        .block(selected_panel("SETUP")),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "NO ACCOUNT. NO NETWORK.",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("• Same authoritative rules as ring play"),
            Line::from("• Pause between hands"),
            Line::from("• Reveal bot reasoning after the hand"),
            Line::from("• Send any hand directly to Study"),
            Line::from(""),
            Line::from(Span::styled("TABLE PREVIEW", Style::default().fg(AMBER))),
            Line::from(""),
            Line::from("      BOT 2       BOT 3"),
            Line::from("  BOT 1    [ FLOP ]    BOT 4"),
            Line::from("        YOU · 100 BB"),
            Line::from(""),
            Line::from(Span::styled(
                "All chips are play money.",
                Style::default().fg(MUTED),
            )),
        ])
        .block(panel("QUICK PRACTICE")),
        cols[1],
    );
}

fn render_host(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            field("Table name", "Dockside Nine", true),
            Line::from(""),
            field("Visibility", "PRIVATE / INVITE", false),
            Line::from(""),
            field("Seats", "9-max", false),
            Line::from(""),
            field("Starting stack", "100 BB", false),
            Line::from(""),
            field("Blinds", "1 / 2 chips", false),
            Line::from(""),
            field("Minimum to start", "2 players", false),
            Line::from(""),
            field("Idle expiry", "30 minutes", false),
            Line::from(""),
            Line::from(Span::styled(
                "  [ ENTER ]  CREATE TABLE",
                Style::default()
                    .fg(SCREEN)
                    .bg(AMBER)
                    .add_modifier(Modifier::BOLD),
            )),
        ])
        .block(selected_panel("CREATE A RING GAME")),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "REACH",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from("This build listens on this computer only."),
            Line::from(Span::styled(
                "Internet hosting is not enabled.",
                Style::default().fg(AMBER),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "SAFE DEFAULTS",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from("✓ Private and unlisted by default"),
            Line::from("✓ Invite can be rotated or revoked"),
            Line::from("✓ Waiting list is bounded"),
            Line::from("✓ Host remains server authority"),
            Line::from(""),
            Line::from(Span::styled(
                "No cash value · no deposits · no rake",
                Style::default().fg(MUTED),
            )),
        ])
        .block(panel("BEFORE YOU HOST"))
        .wrap(Wrap { trim: true }),
        cols[1],
    );
}

fn render_host_lobby(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(44), Constraint::Percentage(56)])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "DOCKSIDE NINE",
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "PRIVATE · 9-max · 1/2 · 100 BB",
                Style::default().fg(MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "INVITE READY",
                Style::default().fg(GREEN).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "  ••••–••••  ",
                Style::default()
                    .fg(WHITE)
                    .bg(PANEL_2)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Masked in design evidence",
                Style::default().fg(MUTED),
            )),
            Line::from(""),
            Line::from("[ c ] Copy privately"),
            Line::from("[ r ] Rotate invite"),
            Line::from("[ x ] Revoke invite"),
            Line::from(""),
            Line::from(Span::styled(
                "● HOST AUTHORITY HEALTHY",
                Style::default().fg(GREEN),
            )),
        ])
        .block(selected_panel("SHARE INVITE")),
        cols[0],
    );
    let seats = [
        ("1", "YOU", "HOST · READY"),
        ("2", "Mara", "READY"),
        ("3", "Eli", "READY"),
        ("4", "—", "OPEN"),
        ("5", "—", "OPEN"),
        ("6", "—", "OPEN"),
        ("7", "—", "OPEN"),
        ("8", "—", "OPEN"),
        ("9", "—", "OPEN"),
    ];
    let mut lines = vec![
        Line::from(Span::styled(
            "3 seated · minimum reached",
            Style::default().fg(GREEN),
        )),
        Line::from(""),
    ];
    for (seat, name, state) in seats {
        lines.push(Line::from(vec![
            Span::styled(format!(" {seat} "), Style::default().fg(SCREEN).bg(BORDER)),
            Span::styled(format!(" {name:<16}"), Style::default().fg(TEXT)),
            Span::styled(
                state,
                Style::default().fg(if state == "OPEN" { MUTED } else { CYAN }),
            ),
        ]));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "  [ ENTER ]  START TABLE",
        Style::default()
            .fg(SCREEN)
            .bg(GREEN)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(Paragraph::new(lines).block(panel("SEATS")), cols[1]);
}

fn render_join(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "PASTE AN INVITE",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  SB–••••–••••                              ",
                Style::default().fg(WHITE).bg(PANEL_2),
            )),
            Line::from(Span::styled(
                "  Invite values remain masked in screenshots and logs.",
                Style::default().fg(MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  [ ENTER ]  INSPECT TABLE",
                Style::default()
                    .fg(SCREEN)
                    .bg(GREEN)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("──────────────────── or ────────────────────"),
            Line::from(""),
            Line::from(Span::styled(
                "  [ p ]  BROWSE PUBLIC TABLES",
                Style::default().fg(CYAN),
            )),
        ])
        .block(selected_panel("JOIN")),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "RECENT",
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from("Dockside Nine"),
            Line::from(Span::styled(
                "Private · last played today",
                Style::default().fg(MUTED),
            )),
            Line::from(""),
            Line::from("Workshop Table"),
            Line::from(Span::styled(
                "Unlisted · last played yesterday",
                Style::default().fg(MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Reconnect credentials are stored locally and never shown here.",
                Style::default().fg(MUTED),
            )),
        ])
        .block(panel("RECENT TABLES"))
        .wrap(Wrap { trim: true }),
        cols[1],
    );
}

fn render_public_lobby(frame: &mut Frame<'_>, area: Rect) {
    let inner = inset(area, 2, 1);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(7),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " / ",
                Style::default()
                    .fg(SCREEN)
                    .bg(CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " 9-max · seats available · any stakes",
                Style::default().fg(TEXT).bg(PANEL_2),
            ),
        ]))
        .block(panel("FILTER")),
        rows[0],
    );
    let header = Row::new(["TABLE", "GAME", "PLAYERS", "WAIT", "STATUS"])
        .style(Style::default().fg(CYAN).add_modifier(Modifier::BOLD))
        .bottom_margin(1);
    let table_rows = [
        Row::new(["▶ Blue Room", "9-max · 1/2", "6 / 9", "0", "OPEN"])
            .style(Style::default().fg(WHITE).bg(Color::Rgb(18, 54, 70))),
        Row::new(["  Lamplight", "6-max · 1/2", "6 / 6", "2", "WAIT"]),
        Row::new(["  First Orbit", "9-max · 2/4", "3 / 9", "0", "OPEN"]),
        Row::new(["  Short Deck? No", "6-max · 1/2", "2 / 6", "0", "OPEN"]),
        Row::new(["  Night Shift", "9-max · 5/10", "8 / 9", "0", "OPEN"]),
    ];
    frame.render_widget(
        Table::new(
            table_rows,
            [
                Constraint::Length(28),
                Constraint::Length(19),
                Constraint::Length(14),
                Constraint::Length(9),
                Constraint::Min(10),
            ],
        )
        .header(header)
        .column_spacing(1)
        .block(selected_panel("PUBLIC DIRECTORY · REVISION 42")),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "BLUE ROOM  ",
                    Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
                ),
                Span::styled("OPEN · 3 seats", Style::default().fg(GREEN)),
            ]),
            Line::from("9-max · 1/2 chips · 100 BB · minimum 2 · hand in progress"),
            Line::from(Span::styled(
                "[ j ] Join at the next hand boundary     [ enter ] Full details",
                Style::default().fg(AMBER),
            )),
        ])
        .block(panel("INSPECT")),
        rows[2],
    );
}

#[derive(Clone, Copy)]
struct SeatFixture {
    label: &'static str,
    name: &'static str,
    stack: &'static str,
    status: &'static str,
    hud: &'static str,
}

const SEATS: [SeatFixture; 9] = [
    SeatFixture {
        label: "S0 · BTN",
        name: "YOU",
        stack: "96 BB",
        status: "ACTING",
        hud: "18/14/5 · 42",
    },
    SeatFixture {
        label: "S1 · SB",
        name: "Mara",
        stack: "101 BB",
        status: "",
        hud: "24/18/7 · 81",
    },
    SeatFixture {
        label: "S2 · BB",
        name: "Eli",
        stack: "88 BB",
        status: "CALL 6",
        hud: "29/11/2 · 55",
    },
    SeatFixture {
        label: "S3 · UTG",
        name: "Noor",
        stack: "132 BB",
        status: "FOLD",
        hud: "17/13/4 · 96",
    },
    SeatFixture {
        label: "S4 · UTG+1",
        name: "Ivo",
        stack: "74 BB",
        status: "",
        hud: "31/22/9 · 38",
    },
    SeatFixture {
        label: "S5 · LJ",
        name: "Kit",
        stack: "116 BB",
        status: "BET 6",
        hud: "22/17/6 · 73",
    },
    SeatFixture {
        label: "S6 · HJ",
        name: "Rue",
        stack: "64 BB",
        status: "",
        hud: "15/12/3 · 61",
    },
    SeatFixture {
        label: "S7 · CO",
        name: "Ash",
        stack: "143 BB",
        status: "FOLD",
        hud: "27/21/8 · 104",
    },
    SeatFixture {
        label: "S8",
        name: "OPEN",
        stack: "—",
        status: "",
        hud: "—",
    },
];

fn render_table(frame: &mut Frame<'_>, area: Rect, hud: bool) {
    let table_area = Rect::new(area.x + 1, area.y, area.width - 2, area.height - 7);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(25, 105, 103)))
            .style(Style::default().bg(FELT)),
        Rect::new(
            table_area.x + 21,
            table_area.y + 3,
            table_area.width - 42,
            table_area.height - 7,
        ),
    );
    let positions = [
        Rect::new(47, area.y + 21, 26, 5),
        Rect::new(15, area.y + 19, 20, 4),
        Rect::new(2, area.y + 12, 20, 4),
        Rect::new(10, area.y + 5, 20, 4),
        Rect::new(38, area.y + 1, 20, 4),
        Rect::new(62, area.y + 1, 20, 4),
        Rect::new(90, area.y + 5, 20, 4),
        Rect::new(98, area.y + 12, 20, 4),
        Rect::new(85, area.y + 19, 20, 4),
    ];
    for (index, seat) in SEATS.iter().enumerate() {
        render_seat(frame, positions[index], *seat, index == 0, hud);
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "POT  18 BB",
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                card_span("A♠", false),
                Span::raw(" "),
                card_span("J♥", true),
                Span::raw(" "),
                card_span("7♣", false),
            ]),
            Line::from(Span::styled(
                "FLOP · TO CALL 6 BB",
                Style::default().fg(MUTED),
            )),
        ])
        .alignment(Alignment::Center),
        Rect::new(39, area.y + 10, 42, 6),
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            card_span("Q♠", false),
            Span::raw(" "),
            card_span("Q♦", true),
        ]))
        .alignment(Alignment::Center),
        Rect::new(48, area.y + 24, 24, 1),
    );
    let actions = Rect::new(area.x + 14, area.bottom() - 6, area.width - 28, 5);
    frame.render_widget(Clear, actions);
    frame.render_widget(Block::default().style(Style::default().bg(SCREEN)), actions);
    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([
            Constraint::Percentage(22),
            Constraint::Percentage(25),
            Constraint::Percentage(31),
            Constraint::Percentage(22),
        ])
        .split(actions);
    action_button(frame, buttons[0], "[ F ]  FOLD", RED, false);
    action_button(frame, buttons[1], "[ C ]  CALL 6", GREEN, false);
    action_button(frame, buttons[2], "[ R ]  RAISE TO 18", AMBER, true);
    action_button(frame, buttons[3], "[ A ]  ALL-IN", VIOLET, false);
    frame.render_widget(
        Paragraph::new("● CONNECTED · authoritative revision 184 · 22s to act")
            .style(Style::default().fg(GREEN))
            .alignment(Alignment::Center),
        Rect::new(area.x + 20, area.bottom() - 1, area.width - 40, 1),
    );
    if hud {
        frame.render_widget(
            Paragraph::new(
                "SYNTHETIC HUD · locally observed public actions only · VPIP/PFR/3B · sample N",
            )
            .style(Style::default().fg(AMBER))
            .alignment(Alignment::Center),
            Rect::new(area.x + 18, area.y, area.width - 36, 1),
        );
    }
}

fn render_seat(frame: &mut Frame<'_>, area: Rect, seat: SeatFixture, hero: bool, hud: bool) {
    let border = if hero {
        AMBER
    } else if seat.status == "BET 6" {
        CYAN
    } else {
        BORDER
    };
    let title = format!(" {} ", seat.label);
    let mut lines = vec![Line::from(vec![
        Span::styled(
            format!("{:<9}", seat.name),
            Style::default()
                .fg(if hero { WHITE } else { TEXT })
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(seat.stack, Style::default().fg(AMBER)),
    ])];
    let second = if hud {
        seat.hud
    } else if seat.name == "OPEN" {
        "[ sit here ]"
    } else if seat.status.is_empty() {
        "░░ ░░"
    } else {
        seat.status
    };
    lines.push(Line::from(Span::styled(
        second,
        Style::default().fg(if seat.status == "FOLD" { MUTED } else { CYAN }),
    )));
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border))
                .style(Style::default().bg(PANEL)),
        ),
        area,
    );
}

fn card_span(text: &'static str, red: bool) -> Span<'static> {
    Span::styled(
        format!(" {text} "),
        Style::default()
            .fg(if red { RED } else { Color::Rgb(34, 39, 44) })
            .bg(WHITE)
            .add_modifier(Modifier::BOLD),
    )
}

fn action_button(frame: &mut Frame<'_>, area: Rect, label: &str, color: Color, selected: bool) {
    frame.render_widget(
        Paragraph::new(label)
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(if selected { SCREEN } else { color })
                    .bg(if selected { color } else { PANEL })
                    .add_modifier(Modifier::BOLD),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(color)),
            ),
        area,
    );
}

fn render_study(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let hands = [
        ("▶ #184  Dockside Nine", "BTN · QQ · +18 BB · today 21:44"),
        ("  #183  Dockside Nine", "CO  · AJs · −6 BB  · today 21:41"),
        ("  #182  Practice", "BB  · 88 · +11 BB · today 20:12"),
        ("  #181  Blue Room", "HJ  · KQs · −2 BB  · yesterday"),
        ("  #180  Practice", "UTG · AKs · +27 BB · yesterday"),
    ];
    let mut lines = vec![
        Line::from(Span::styled(
            "/ all tables · last 30 days",
            Style::default().fg(MUTED),
        )),
        Line::from(""),
    ];
    for (index, (name, meta)) in hands.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            *name,
            Style::default()
                .fg(if index == 0 { AMBER } else { TEXT })
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(Span::styled(*meta, Style::default().fg(MUTED))));
        lines.push(Line::from(""));
    }
    frame.render_widget(
        Paragraph::new(lines).block(selected_panel("RECENT HANDS")),
        cols[0],
    );
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(13), Constraint::Min(10)])
        .split(cols[1]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "1,284",
                    Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" hands observed locally", Style::default().fg(MUTED)),
            ]),
            Line::from(""),
            Line::from("VPIP      22%      PFR       17%"),
            Line::from("3-BET      6%      SHOWDOWN  31%"),
            Line::from("NET       +84 BB   BIGGEST   +42 BB"),
            Line::from(""),
            Line::from(Span::styled(
                "Statistics never include hidden or cross-table data.",
                Style::default().fg(MUTED),
            )),
        ])
        .block(panel("YOUR OVERVIEW")),
        right[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "[ ENTER ]  Replay selected hand",
                Style::default().fg(CYAN),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "[ R ]      Range Explorer",
                Style::default().fg(VIOLET),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "[ N ]      Notes and tags",
                Style::default().fg(AMBER),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "[ E ]      Export public-safe hand",
                Style::default().fg(GREEN),
            )),
        ])
        .block(panel("LEARN")),
        right[1],
    );
}

fn render_range(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(1)
        .constraints([Constraint::Length(65), Constraint::Min(40)])
        .split(area);
    frame.render_widget(panel("13×13 STARTING-HAND MATRIX"), cols[0]);
    let ranks = [
        "A", "K", "Q", "J", "T", "9", "8", "7", "6", "5", "4", "3", "2",
    ];
    let matrix = inset(cols[0], 2, 2);
    frame.buffer_mut().set_string(
        matrix.x + 4,
        matrix.y,
        ranks.iter().map(|r| format!(" {r} ")).collect::<String>(),
        Style::default().fg(MUTED),
    );
    for row in 0..13u16 {
        frame.buffer_mut().set_string(
            matrix.x,
            matrix.y + row + 1,
            ranks[row as usize],
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        );
        for col in 0..13u16 {
            let (symbol, color) = range_category(row as usize, col as usize);
            let focused = row == 0 && col == 10;
            frame.buffer_mut().set_string(
                matrix.x + 4 + col * 3,
                matrix.y + row + 1,
                format!("{symbol:^3}"),
                Style::default()
                    .fg(if focused { SCREEN } else { WHITE })
                    .bg(if focused { WHITE } else { color })
                    .add_modifier(Modifier::BOLD),
            );
        }
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "SYNTHETIC UI FIXTURE",
                Style::default().fg(RED).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Not poker advice or an approved strategy pack.",
                Style::default().fg(MUTED),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "SCENARIO",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from("9-max · BTN · 100 BB"),
            Line::from("Unopened pot · 1/2 blinds"),
            Line::from(""),
            Line::from(Span::styled(
                "SELECTED  A4s",
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            )),
            Line::from("Raise 55% · Fold 45%"),
            Line::from("4 suited combinations"),
            Line::from(""),
            Line::from(Span::styled(
                "LEGEND",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled(" R ", Style::default().fg(WHITE).bg(BLUE)),
                Span::raw(" Raise   "),
                Span::styled(" J ", Style::default().fg(WHITE).bg(VIOLET)),
                Span::raw(" Jam"),
            ]),
            Line::from(vec![
                Span::styled(" C ", Style::default().fg(WHITE).bg(GREEN)),
                Span::raw(" Call    "),
                Span::styled(" F ", Style::default().fg(WHITE).bg(Color::Rgb(52, 61, 69))),
                Span::raw(" Fold"),
            ]),
            Line::from(vec![
                Span::styled(" M ", Style::default().fg(WHITE).bg(AMBER)),
                Span::raw(" Mixed"),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "SOURCE",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from("Fixture pack · schema v1"),
            Line::from("Purpose: layout and interaction QA"),
        ])
        .block(selected_panel("RANGE EXPLORER"))
        .wrap(Wrap { trim: true }),
        cols[1],
    );
}

fn range_category(row: usize, col: usize) -> (&'static str, Color) {
    if row == col {
        ("J", VIOLET)
    } else if row < col && row < 4 {
        ("R", BLUE)
    } else if row < col && (row + col).is_multiple_of(4) {
        ("M", AMBER)
    } else if row > col && row < 5 && col < 4 {
        ("C", GREEN)
    } else {
        ("F", Color::Rgb(52, 61, 69))
    }
}

fn render_replay(frame: &mut Frame<'_>, area: Rect) {
    let inner = inset(area, 2, 1);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(20), Constraint::Length(10)])
        .split(inner);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(25, 105, 103)))
            .style(Style::default().bg(FELT))
            .title(" READ-ONLY TABLE · HAND #184 "),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "POT 18 BB",
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                card_span("A♠", false),
                Span::raw(" "),
                card_span("J♥", true),
                Span::raw(" "),
                card_span("7♣", false),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "FLOP · action 7 of 14",
                Style::default().fg(MUTED),
            )),
        ])
        .alignment(Alignment::Center),
        Rect::new(rows[0].x + 35, rows[0].y + 6, rows[0].width - 70, 7),
    );
    for (x, y, text) in [
        (5, 5, "Noor 132 BB · FOLD"),
        (5, 13, "Mara 101 BB · CALL 6"),
        (85, 5, "Kit 116 BB · BET 6"),
        (85, 13, "Eli 88 BB · ░░ ░░"),
        (43, 17, "YOU 96 BB · Q♠ Q♦"),
    ] {
        frame.render_widget(
            Paragraph::new(text)
                .style(Style::default().fg(if text.starts_with("YOU") { AMBER } else { TEXT }))
                .block(panel("SEAT")),
            Rect::new(rows[0].x + x, rows[0].y + y, 28, 4),
        );
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("01 DEAL", Style::default().fg(MUTED)),
                Span::raw(" ─ "),
                Span::styled("04 PREFLOP", Style::default().fg(MUTED)),
                Span::raw(" ─ "),
                Span::styled(
                    "▶ 07 FLOP",
                    Style::default()
                        .fg(SCREEN)
                        .bg(AMBER)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" ─ 10 TURN ─ 12 RIVER ─ 14 AWARD"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "KIT",
                    Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" bets 6 BB  ·  "),
                Span::styled("YOU", Style::default().fg(AMBER)),
                Span::raw(" to act with Q♠ Q♦"),
            ]),
            Line::from(Span::styled(
                "NOTE  ‘Raise or control the pot here?’",
                Style::default().fg(MUTED),
            )),
        ])
        .block(selected_panel("TIMELINE")),
        rows[1],
    );
}

fn render_settings(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            field("Theme", "MIDNIGHT FELT", true),
            Line::from(""),
            field("Color mode", "TRUE COLOR", false),
            Line::from(""),
            field("Reduced motion", "OFF", false),
            Line::from(""),
            field("Live HUD", "OFF", false),
            Line::from(""),
            field("Card notation", "RANK + SUIT", false),
            Line::from(""),
            field("Confirm all-in", "ON", false),
            Line::from(""),
            field("History panel", "COLLAPSED", false),
            Line::from(""),
            field("Sound", "NOT AVAILABLE", false),
        ])
        .block(selected_panel("PREFERENCES")),
        cols[0],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "THEME PREVIEW",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled(" BRAND ", Style::default().fg(SCREEN).bg(AMBER)),
                Span::raw(" "),
                Span::styled(" INFO ", Style::default().fg(SCREEN).bg(CYAN)),
                Span::raw(" "),
                Span::styled(" OK ", Style::default().fg(SCREEN).bg(GREEN)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(" FOLD ", Style::default().fg(WHITE).bg(RED)),
                Span::raw(" "),
                Span::styled(" RAISE ", Style::default().fg(SCREEN).bg(AMBER)),
                Span::raw(" "),
                Span::styled(" JAM ", Style::default().fg(WHITE).bg(VIOLET)),
            ]),
            Line::from(""),
            Line::from(vec![
                card_span("A♠", false),
                Span::raw(" "),
                card_span("J♥", true),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "ACCESSIBILITY",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from("Every state retains text, glyph, or border cues."),
            Line::from("NO_COLOR selects a deliberate monochrome hierarchy."),
            Line::from("Reduced motion never changes game timing."),
            Line::from(""),
            Line::from(Span::styled(
                "Settings are local to this device.",
                Style::default().fg(MUTED),
            )),
        ])
        .block(panel("PREVIEW"))
        .wrap(Wrap { trim: true }),
        cols[1],
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(51), Constraint::Percentage(49)])
        .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "GLOBAL",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from("  ↑ ↓ ← →     Move focus or selection"),
            Line::from("  ENTER       Choose / confirm"),
            Line::from("  ESC         Back / close overlay"),
            Line::from("  ?           Contextual help"),
            Line::from("  q           Quit safely"),
            Line::from(""),
            Line::from(Span::styled(
                "AT THE TABLE",
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            )),
            Line::from("  f           Fold"),
            Line::from("  x / c       Check / call"),
            Line::from("  r           Raise or bet"),
            Line::from("  a           All-in (confirmation on)"),
            Line::from("  h           Toggle hand history"),
            Line::from("  s           Sit out next hand"),
        ])
        .block(selected_panel("KEYBOARD")),
        cols[0],
    );
    frame.render_widget(Paragraph::new(vec![
        Line::from(Span::styled("CURRENT CONTEXT · LIVE TABLE", Style::default().fg(AMBER).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("Only legal actions are enabled."),
        Line::from("Actions wait for server confirmation."),
        Line::from("Disconnected controls are disabled immediately."),
        Line::from(""),
        Line::from(Span::styled("CARD PRIVACY", Style::default().fg(CYAN).add_modifier(Modifier::BOLD))),
        Line::from("You see your own cards and legitimately public reveals only."),
        Line::from("Mucked and opponent cards remain hidden."),
        Line::from(""),
        Line::from(Span::styled("CONNECTION", Style::default().fg(CYAN).add_modifier(Modifier::BOLD))),
        Line::from("A reconnect overlay preserves the table route while a fresh authorized snapshot is requested."),
        Line::from(""),
        Line::from(Span::styled("All play is play-money only.", Style::default().fg(MUTED))),
    ]).block(panel("HOW THIS SCREEN WORKS")).wrap(Wrap { trim: true }), cols[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render(screen: ConceptScreen) -> String {
        let backend = TestBackend::new(CONCEPT_WIDTH, CONCEPT_HEIGHT);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render_concept(frame, screen))
            .expect("concept render");
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    fn render_ash_continuity() -> String {
        let backend = TestBackend::new(CONCEPT_WIDTH, CONCEPT_HEIGHT);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(render_ash_continuity_mockup)
            .expect("Ash Continuity render");
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn every_feature_renders_at_the_standard_viewport() {
        for screen in ConceptScreen::ALL {
            let text = render(screen);
            assert!(
                text.contains("SNEAKY BLINDERS"),
                "missing brand in {:?}",
                screen
            );
            assert!(
                text.contains(screen.title()),
                "missing title in {:?}",
                screen
            );
        }
    }

    #[test]
    fn fixtures_preserve_play_money_and_private_information_boundaries() {
        for screen in ConceptScreen::ALL {
            let text = render(screen);
            assert!(!text.contains('$'), "cash symbol in {:?}", screen);
            for forbidden in [
                "join_code",
                "bearer",
                "reconnect token",
                "deck order",
                "session token",
            ] {
                assert!(
                    !text.to_lowercase().contains(forbidden),
                    "private term {forbidden:?} in {:?}",
                    screen
                );
            }
        }
        let range = render(ConceptScreen::RangeExplorer);
        assert!(range.contains("SYNTHETIC UI FIXTURE"));
        let hud = render(ConceptScreen::TableHud);
        assert!(hud.contains("SYNTHETIC HUD"));
    }

    #[test]
    fn ash_continuity_restores_table_theatre_without_losing_multiplayer_state() {
        let text = render_ash_continuity();
        for expected in [
            "DOCKSIDE NINE",
            "S8 · OPEN",
            "POT  18 BB",
            "FOLD",
            "CALL 6",
            "RAISE TO 18",
            "ALL-IN",
            "YOU to act",
        ] {
            assert!(text.contains(expected), "missing {expected:?}");
        }
        assert!(!text.contains('$'));
        assert!(!text.contains("LOCAL / PLAY MONEY"));
    }
}
