//! Minimal installed shell with honest capability states.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::local_profile::LocalProfile;
use crate::ui::platform::{ColorDepth, SemanticTheme, ThemeMode, ViewportClass};

pub const HOME_MIN_WIDTH: u16 = 40;
pub const HOME_MIN_HEIGHT: u16 = 20;
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;
pub const STANDARD_WIDTH: u16 = 120;
pub const STANDARD_HEIGHT: u16 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellRoute {
    Home,
    QuickPractice,
    Settings,
    Help,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    Select,
    InputChar(char),
    Backspace,
    OpenSettings,
    OpenHelp,
    Back,
    Failure(String),
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellEffect {
    None,
    StartQuickPractice,
    StartHostTournament,
    StartJoinTournament,
    SaveProfile,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellApp {
    route: ShellRoute,
    return_route: ShellRoute,
    selected_home_item: usize,
    status: String,
    selected_setting: usize,
    editing_name: bool,
    profile: LocalProfile,
}

impl Default for ShellApp {
    fn default() -> Self {
        Self::new(LocalProfile::default())
    }
}

impl ShellApp {
    pub fn new(profile: LocalProfile) -> Self {
        Self {
            route: ShellRoute::Home,
            return_route: ShellRoute::Home,
            selected_home_item: 0,
            status: "Ready · Quick Practice · Host · Join".to_string(),
            selected_setting: 0,
            editing_name: false,
            profile,
        }
    }

    pub const fn route(&self) -> ShellRoute {
        self.route
    }

    pub const fn selected_home_item(&self) -> usize {
        self.selected_home_item
    }

    pub fn status(&self) -> &str {
        &self.status
    }

    pub const fn selected_setting(&self) -> usize {
        self.selected_setting
    }

    pub const fn editing_name(&self) -> bool {
        self.editing_name
    }

    pub fn profile_mut(&mut self) -> &mut LocalProfile {
        &mut self.profile
    }

    pub const fn profile(&self) -> &LocalProfile {
        &self.profile
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }

    pub fn handle(&mut self, event: ShellEvent) -> ShellEffect {
        match event {
            ShellEvent::Quit => return ShellEffect::Quit,
            ShellEvent::Failure(message) => {
                self.return_route = self.route;
                self.route = ShellRoute::Error;
                self.status = message;
            }
            ShellEvent::OpenHelp => {
                if self.route != ShellRoute::Help {
                    self.return_route = self.route;
                    self.route = ShellRoute::Help;
                }
            }
            ShellEvent::OpenSettings => {
                if self.route != ShellRoute::QuickPractice {
                    self.return_route = self.route;
                    self.route = ShellRoute::Settings;
                    self.editing_name = false;
                }
            }
            ShellEvent::Back => {
                if self.route == ShellRoute::Settings && self.editing_name {
                    self.editing_name = false;
                    return ShellEffect::None;
                }
                self.route = match self.route {
                    ShellRoute::Home => ShellRoute::Home,
                    ShellRoute::Help | ShellRoute::Settings | ShellRoute::Error => {
                        self.return_route
                    }
                    ShellRoute::QuickPractice => ShellRoute::Home,
                };
                if self.route == ShellRoute::Home {
                    self.status = "Ready · Quick Practice · Host · Join".to_string();
                }
            }
            ShellEvent::MoveUp if self.route == ShellRoute::Home => {
                self.selected_home_item = self.selected_home_item.saturating_sub(1);
            }
            ShellEvent::MoveDown if self.route == ShellRoute::Home => {
                self.selected_home_item = (self.selected_home_item + 1).min(HOME_ITEMS.len() - 1);
            }
            ShellEvent::MoveUp if self.route == ShellRoute::Settings => {
                self.selected_setting = self.selected_setting.saturating_sub(1);
            }
            ShellEvent::MoveDown if self.route == ShellRoute::Settings => {
                self.selected_setting = (self.selected_setting + 1).min(1);
            }
            direction @ (ShellEvent::MoveLeft | ShellEvent::MoveRight)
                if self.route == ShellRoute::Settings && !self.editing_name =>
            {
                match self.selected_setting {
                    1 => {
                        const STACKS: [u32; 4] = [100, 250, 500, 1_000];
                        let current = STACKS
                            .iter()
                            .position(|stack| *stack == self.profile.quick_starting_stack)
                            .unwrap_or(0);
                        let next = if direction == ShellEvent::MoveLeft {
                            current.saturating_sub(1)
                        } else {
                            (current + 1).min(STACKS.len() - 1)
                        };
                        self.profile.quick_starting_stack = STACKS[next];
                    }
                    _ => return ShellEffect::None,
                }
                return ShellEffect::SaveProfile;
            }
            ShellEvent::InputChar(character)
                if self.route == ShellRoute::Settings && self.editing_name =>
            {
                if !character.is_control() && self.profile.display_name.chars().count() < 24 {
                    self.profile.display_name.push(character);
                }
            }
            ShellEvent::Backspace if self.route == ShellRoute::Settings && self.editing_name => {
                self.profile.display_name.pop();
            }
            ShellEvent::Select if self.route == ShellRoute::Home => {
                match self.selected_home_item {
                    0 => {
                        self.route = ShellRoute::QuickPractice;
                        return ShellEffect::StartQuickPractice;
                    }
                    1 => return ShellEffect::StartHostTournament,
                    2 => return ShellEffect::StartJoinTournament,
                    4 => return self.handle(ShellEvent::OpenSettings),
                    _ => {}
                }
                if self.selected_home_item == HOME_ITEMS.len() - 1 {
                    return ShellEffect::Quit;
                }
                self.status = format!(
                    "{} is not integrated yet · choose Quick Practice",
                    HOME_ITEMS[self.selected_home_item].0
                );
            }
            ShellEvent::Select if self.route == ShellRoute::Settings => {
                if self.selected_setting == 0 {
                    if self.editing_name {
                        self.editing_name = false;
                        if self.profile.display_name.is_empty() {
                            self.profile.display_name = "Player".to_string();
                        }
                        return ShellEffect::SaveProfile;
                    }
                    self.editing_name = true;
                    return ShellEffect::None;
                }
                return ShellEffect::SaveProfile;
            }
            ShellEvent::MoveUp
            | ShellEvent::MoveDown
            | ShellEvent::MoveLeft
            | ShellEvent::MoveRight
            | ShellEvent::Select
            | ShellEvent::InputChar(_)
            | ShellEvent::Backspace => {}
        }
        ShellEffect::None
    }
}

pub const HOME_ITEMS: [(&str, &str, bool); 6] = [
    (
        "QUICK PRACTICE",
        "Repeat hands against eight projection-bound local bots",
        true,
    ),
    (
        "HOST GAME",
        "Create a private single-table tournament on this computer",
        true,
    ),
    (
        "JOIN GAME",
        "Join a private tournament with one opaque invite",
        true,
    ),
    (
        "STUDY",
        "Replay and ranges are not integrated in this build",
        false,
    ),
    ("SETTINGS", "Profile and presentation preferences", true),
    ("QUIT", "Restore the terminal and exit", true),
];

pub fn render_shell(
    frame: &mut Frame<'_>,
    app: &ShellApp,
    profile_path: &str,
    theme: &SemanticTheme,
) {
    if app.route() != ShellRoute::Home
        && app.route() != ShellRoute::QuickPractice
        && (frame.area().width < MIN_WIDTH || frame.area().height < MIN_HEIGHT)
    {
        frame.render_widget(
            Paragraph::new("This screen needs 80x24. Resize, Esc for Home, or Q to quit.")
                .style(Style::default().fg(theme.text).bg(theme.screen))
                .wrap(Wrap { trim: true }),
            frame.area(),
        );
        return;
    }
    match app.route() {
        ShellRoute::Home | ShellRoute::QuickPractice => render_home_profile(
            frame,
            app.selected_home_item(),
            app.status(),
            &app.profile.display_name,
            theme,
        ),
        ShellRoute::Settings => render_settings(frame, app, profile_path, theme),
        ShellRoute::Help => render_help(frame, app, theme),
        ShellRoute::Error => render_error(frame, app, theme),
    }
}

pub fn render_home(frame: &mut Frame<'_>, selected: usize, status: &str) {
    let theme = SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor);
    render_home_with_theme(frame, selected, status, &theme);
}

pub fn render_tournament_entry(frame: &mut Frame<'_>, title: &str, label: &str, value: &str) {
    let theme = SemanticTheme::resolve(
        ThemeMode::Ash,
        super::platform::TerminalCapabilities::detect().color_depth,
    );
    super::game_lobby::render_lobby_message(
        frame,
        title,
        &format!("{label}\n\n> {value}_"),
        "Enter confirm | Backspace edit\nEsc cancel",
        theme,
    );
}

pub fn render_tournament_result(frame: &mut Frame<'_>, title: &str, lines: &[String]) {
    let theme = SemanticTheme::resolve(
        ThemeMode::Ash,
        super::platform::TerminalCapabilities::detect().color_depth,
    );
    let footer = lines
        .last()
        .filter(|line| line.starts_with("Press any key"))
        .map(String::as_str)
        .unwrap_or("");
    let content = if footer.is_empty() {
        lines
    } else {
        &lines[..lines.len() - 1]
    };
    super::game_lobby::render_lobby_message(frame, title, &content.join("\n"), footer, theme);
}

pub fn render_home_with_theme(
    frame: &mut Frame<'_>,
    selected: usize,
    status: &str,
    theme: &SemanticTheme,
) {
    render_home_profile(frame, selected, status, "Player", theme);
}

fn render_home_profile(
    frame: &mut Frame<'_>,
    selected: usize,
    status: &str,
    display_name: &str,
    theme: &SemanticTheme,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.screen)),
        area,
    );
    if area.width < HOME_MIN_WIDTH || area.height < HOME_MIN_HEIGHT {
        frame.render_widget(
            Paragraph::new(
                "Sneaky Blinders needs at least 40x20 for Home. Resize or Q to quit safely.",
            )
            .style(Style::default().fg(theme.text))
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    if area.width < STANDARD_WIDTH || area.height < STANDARD_HEIGHT {
        render_compact_home(frame, selected, status, theme);
        return;
    }

    let canvas = Rect::new(
        area.x + (area.width - STANDARD_WIDTH) / 2,
        area.y + (area.height - STANDARD_HEIGHT) / 2,
        STANDARD_WIDTH,
        STANDARD_HEIGHT,
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(canvas);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " SNEAKY BLINDERS ",
                Style::default()
                    .fg(theme.screen)
                    .bg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    "  v{}  ·  HOME  ·  {}",
                    env!("CARGO_PKG_VERSION"),
                    display_name
                ),
                Style::default().fg(theme.text),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        ),
        rows[0],
    );

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .margin(2)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "ONE COMMAND. EVERY TABLE.",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "The installed shell owns Quick Practice and the",
                Style::default().fg(theme.text),
            )),
            Line::from(Span::styled(
                "production nine-seat table presentation.",
                Style::default().fg(theme.text),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Play-money No-Limit Texas Hold'em",
                Style::default().fg(theme.info),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Host and Join run private tournaments on this computer.",
                Style::default().fg(theme.muted),
            )),
            Line::from(Span::styled(
                "Study remains disabled until its journey is ready.",
                Style::default().fg(theme.muted),
            )),
        ])
        .block(panel("WELCOME", theme))
        .wrap(Wrap { trim: true }),
        columns[0],
    );

    let mut menu = Vec::new();
    for (index, (label, description, enabled)) in HOME_ITEMS.iter().enumerate() {
        let focused = index == selected;
        let marker = if focused { "▶" } else { " " };
        let label_style = if focused {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if *enabled {
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.muted)
        };
        menu.push(Line::from(vec![
            Span::styled(format!(" {marker} {label:<18}"), label_style),
            Span::styled(
                if *enabled { "READY" } else { "COMING NEXT" },
                Style::default().fg(if *enabled { theme.success } else { theme.muted }),
            ),
        ]));
        menu.push(Line::from(Span::styled(
            format!("     {description}"),
            Style::default().fg(theme.muted),
        )));
        menu.push(Line::from(""));
    }
    frame.render_widget(Paragraph::new(menu).block(panel("PLAY", theme)), columns[1]);

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                status.to_string(),
                Style::default().fg(theme.info),
            )),
            Line::from(Span::styled(
                "↑↓ move   Enter select   S settings   ? help   Q quit",
                Style::default().fg(theme.muted),
            )),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(theme.border)),
        ),
        rows[2],
    );
}

fn render_settings(
    frame: &mut Frame<'_>,
    app: &ShellApp,
    _profile_path: &str,
    theme: &SemanticTheme,
) {
    let profile = app.profile();
    let values = [
        format!(
            "DISPLAY NAME    {}{}",
            profile.display_name,
            if app.editing_name() { "_" } else { "" }
        ),
        format!("QUICK STACK     {}", profile.quick_starting_stack),
    ];
    let mut lines = Vec::new();
    for (index, value) in values.iter().enumerate() {
        lines.push(format!(
            "{} {}",
            if index == app.selected_setting() {
                "▶"
            } else {
                " "
            },
            value
        ));
        lines.push(String::new());
    }
    render_detail_screen(
        frame,
        "SETTINGS",
        "PLAYER SETTINGS",
        &lines,
        if app.editing_name() {
            "Type name   Backspace edit   Enter save   Esc cancel"
        } else {
            "↑↓ choose   ←→ change   Enter edit/save   Esc back"
        },
        theme,
        theme.info,
    );
}

fn render_help(frame: &mut Frame<'_>, app: &ShellApp, theme: &SemanticTheme) {
    let lines = vec![
        "HOME      ↑↓ move · Enter select · S settings · ? help · Q quit".to_string(),
        "TABLE     F fold · C check/call · ↑/↓ size · R raise · A all-in".to_string(),
        "CONSOLE   PgUp/PgDn history · Home/End oldest/latest".to_string(),
        String::new(),
        "Quick Practice is play-money and server-authoritative in process.".to_string(),
        "You see your own cards and legitimately public information only.".to_string(),
        "Host creates a game on your running server; Join browses its lobby. Study is unavailable."
            .to_string(),
        String::new(),
        "80×24 compact · 120×40 standard · 160×50 wide".to_string(),
        "Home fits 40x20; Settings and Help need 80x24. Esc returns Home.".to_string(),
    ];
    render_detail_screen(
        frame,
        "HELP",
        &format!(
            "CONTEXT  {:?}  ·  v{}",
            app.return_route,
            env!("CARGO_PKG_VERSION")
        ),
        &lines,
        "Esc return   Q quit",
        theme,
        theme.info,
    );
}

fn render_error(frame: &mut Frame<'_>, app: &ShellApp, theme: &SemanticTheme) {
    let lines = vec![
        "Sneaky Blinders stopped the requested activity safely.".to_string(),
        String::new(),
        app.status().to_string(),
        String::new(),
        "No replacement authoritative state was accepted.".to_string(),
        "The terminal lifecycle remains owned by the application shell.".to_string(),
    ];
    render_detail_screen(
        frame,
        "RECOVERABLE ERROR",
        "ACTIVITY STOPPED · TERMINAL SAFE",
        &lines,
        "Esc return   ? help   Q quit",
        theme,
        theme.danger,
    );
}

fn render_detail_screen(
    frame: &mut Frame<'_>,
    title: &str,
    subtitle: &str,
    lines: &[String],
    footer: &str,
    theme: &SemanticTheme,
    accent: ratatui::style::Color,
) {
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(theme.screen)),
        area,
    );
    if ViewportClass::classify(area.width, area.height) == ViewportClass::Unsupported {
        frame.render_widget(
            Paragraph::new(format!(
                "Sneaky Blinders needs at least {MIN_WIDTH}×{MIN_HEIGHT}. Current terminal: {}×{}. Resize, press ? for Help, or Q to quit safely.",
                area.width, area.height
            ))
            .style(Style::default().fg(theme.text).bg(theme.screen))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }
    let width = area.width.min(STANDARD_WIDTH);
    let height = area.height.min(STANDARD_HEIGHT);
    let canvas = Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(canvas);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!(" {title} "),
                Style::default()
                    .fg(theme.screen)
                    .bg(accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("  {subtitle}"), Style::default().fg(theme.text)),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(theme.border)),
        ),
        rows[0],
    );
    let body = lines
        .iter()
        .map(|line| Line::from(Span::styled(line.clone(), Style::default().fg(theme.text))))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(body)
            .block(panel(title, theme))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(footer)
            .style(Style::default().fg(theme.muted))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(theme.border)),
            ),
        rows[2],
    );
}

fn render_compact_home(
    frame: &mut Frame<'_>,
    selected: usize,
    _status: &str,
    theme: &SemanticTheme,
) {
    let area = frame.area();
    let width = 36.min(area.width.saturating_sub(4));
    let x = area.x + (area.width - width) / 2;
    let y = area.y + ((area.height - 20) / 2).min(6);
    let mut line = |row, value: &str, style: Style, centered| {
        frame.render_widget(
            Paragraph::new(value.to_string())
                .style(style)
                .alignment(if centered {
                    Alignment::Center
                } else {
                    Alignment::Left
                }),
            Rect::new(x, row, width, 1),
        );
    };
    let normal = Style::default().fg(theme.text).bg(theme.screen);
    let muted = Style::default().fg(theme.muted).bg(theme.screen);
    line(
        y + 1,
        "SNEAKY BLINDERS",
        normal.add_modifier(Modifier::BOLD),
        true,
    );
    line(y + 2, "P O K E R   C L U B", muted, true);
    line(y + 4, "PLAY SMARTER STAY SNEAKY", muted, true);
    for (index, (label, _, enabled)) in HOME_ITEMS.iter().enumerate() {
        let focused = index == selected;
        let value = format!(
            "{} {}{}",
            if focused { ">" } else { " " },
            label,
            if !enabled { " (SOON)" } else { "" }
        );
        let style = if focused {
            Style::default()
                .fg(theme.screen)
                .bg(theme.text)
                .add_modifier(Modifier::BOLD)
        } else if *enabled {
            normal
        } else {
            muted
        };
        line(y + 6 + index as u16 * 2, &value, style, false);
    }
    line(
        area.bottom() - 2,
        "Up/Down select | Enter play | Q quit",
        muted,
        true,
    );
}

pub(super) fn panel<'a>(title: &'a str, theme: &SemanticTheme) -> Block<'a> {
    Block::default()
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(theme.info).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border))
        .style(Style::default().bg(theme.panel).fg(theme.text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn shell_routes_practice_help_error_and_quit_through_one_reducer() {
        let mut app = ShellApp::default();
        assert_eq!(app.route(), ShellRoute::Home);

        assert_eq!(
            app.handle(ShellEvent::Select),
            ShellEffect::StartQuickPractice
        );
        assert_eq!(app.route(), ShellRoute::QuickPractice);

        app.handle(ShellEvent::OpenHelp);
        assert_eq!(app.route(), ShellRoute::Help);
        app.handle(ShellEvent::Back);
        assert_eq!(app.route(), ShellRoute::QuickPractice);

        app.handle(ShellEvent::Failure(
            "practice authority stopped".to_string(),
        ));
        assert_eq!(app.route(), ShellRoute::Error);
        app.handle(ShellEvent::Back);
        assert_eq!(app.route(), ShellRoute::QuickPractice);

        app.handle(ShellEvent::Back);
        assert_eq!(app.route(), ShellRoute::Home);

        assert_eq!(app.handle(ShellEvent::Quit), ShellEffect::Quit);
    }

    #[test]
    fn host_and_join_are_integrated_while_study_remains_honestly_disabled() {
        let mut app = ShellApp::default();
        app.handle(ShellEvent::MoveDown);
        assert_eq!(
            app.handle(ShellEvent::Select),
            ShellEffect::StartHostTournament
        );
        app.handle(ShellEvent::MoveDown);
        assert_eq!(
            app.handle(ShellEvent::Select),
            ShellEffect::StartJoinTournament
        );
        app.handle(ShellEvent::MoveDown);
        assert_eq!(app.handle(ShellEvent::Select), ShellEffect::None);
        assert_eq!(app.route(), ShellRoute::Home);
        assert!(app.status().contains("STUDY is not integrated yet"));
    }

    #[test]
    fn menu_settings_entry_opens_settings_and_returns_to_same_selection() {
        let mut app = ShellApp::default();
        for _ in 0..4 {
            app.handle(ShellEvent::MoveDown);
        }
        assert_eq!(HOME_ITEMS[app.selected_home_item()].0, "SETTINGS");
        assert_eq!(app.handle(ShellEvent::Select), ShellEffect::None);
        assert_eq!(app.route(), ShellRoute::Settings);
        app.handle(ShellEvent::Back);
        assert_eq!(app.route(), ShellRoute::Home);
        assert_eq!(app.selected_home_item(), 4);
        app.handle(ShellEvent::MoveDown);
        assert_eq!(app.handle(ShellEvent::Select), ShellEffect::Quit);
    }

    #[test]
    fn settings_edit_profile_through_the_same_reducer() {
        let mut app = ShellApp::default();
        app.handle(ShellEvent::OpenSettings);
        assert_eq!(app.route(), ShellRoute::Settings);
        assert_eq!(app.handle(ShellEvent::Select), ShellEffect::None);
        assert!(app.editing_name());
        for _ in 0.."Player".len() {
            app.handle(ShellEvent::Backspace);
        }
        for character in "Ada".chars() {
            app.handle(ShellEvent::InputChar(character));
        }
        assert_eq!(app.handle(ShellEvent::Select), ShellEffect::SaveProfile);
        assert_eq!(app.profile().display_name, "Ada");

        app.handle(ShellEvent::MoveDown);
        assert_eq!(app.handle(ShellEvent::MoveRight), ShellEffect::SaveProfile);
        assert_eq!(app.profile().quick_starting_stack, 250);
        app.handle(ShellEvent::MoveDown);
        assert_eq!(app.selected_setting(), 1);
    }

    #[test]
    fn every_non_table_shell_route_renders_at_standard_and_compact_sizes() {
        let theme = SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor);
        let mut app = ShellApp::default();
        for route in [
            ShellRoute::Home,
            ShellRoute::Settings,
            ShellRoute::Help,
            ShellRoute::Error,
        ] {
            app.route = route;
            for (width, height) in [(STANDARD_WIDTH, STANDARD_HEIGHT), (80, 24)] {
                let backend = TestBackend::new(width, height);
                let mut terminal = Terminal::new(backend).unwrap();
                terminal
                    .draw(|frame| render_shell(frame, &app, "C:/profile.json", &theme))
                    .unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(text.contains("SNEAKY BLINDERS") || route != ShellRoute::Home);
                assert!(!text.trim().is_empty());
            }
        }
    }

    #[test]
    fn home_is_honest_about_ready_and_future_routes() {
        let backend = TestBackend::new(STANDARD_WIDTH, STANDARD_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_home(frame, 0, "Ready"))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(text.contains("SNEAKY BLINDERS"));
        assert!(text.contains("QUICK PRACTICE"));
        assert!(text.contains("READY"));
        assert!(text.contains("HOST GAME"));
        assert!(text.contains("COMING NEXT"));
    }

    #[test]
    fn compact_terminal_gets_a_deliberate_single_column_home() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_home(frame, 0, "Ready"))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("P O K E R   C L U B"));
        assert!(text.contains("QUICK PRACTICE"));
    }

    #[test]
    fn compact_menu_keeps_every_entry_and_selection_visible_at_small_sizes() {
        for (width, height) in [(40, 20), (56, 20), (56, 40), (80, 24), (100, 36)] {
            for (selected, item) in HOME_ITEMS.iter().enumerate() {
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| render_home(frame, selected, "Ready"))
                    .unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                for (label, _, _) in HOME_ITEMS {
                    assert!(text.contains(label), "{width}x{height}: {label}");
                }
                assert!(text.contains(&format!("> {}", item.0)));
                assert!(text.contains("Enter play"));
            }
        }
    }

    #[test]
    fn unsupported_terminal_gets_a_clear_requirement_and_safe_keys() {
        let backend = TestBackend::new(39, 19);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render_home(frame, 0, "Ready"))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("at least 40x20"));
        assert!(text.contains("quit safely"));
    }

    #[test]
    fn ten_quick_practice_route_cycles_return_home_without_retained_route_state() {
        let mut app = ShellApp::default();
        for _ in 1..=10 {
            assert_eq!(app.route(), ShellRoute::Home);
            assert_eq!(
                app.handle(ShellEvent::Select),
                ShellEffect::StartQuickPractice
            );
            assert_eq!(app.route(), ShellRoute::QuickPractice);

            app.handle(ShellEvent::Back);
            assert_eq!(app.route(), ShellRoute::Home);
            assert_eq!(app.return_route, ShellRoute::Home);
        }
    }
}
