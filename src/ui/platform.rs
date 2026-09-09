//! Shared terminal capability, semantic theme, and presentation-effect boundary.

use std::env;
use std::time::Duration as StdDuration;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use tachyonfx::{fx, EffectManager, Interpolation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    Basic,
    Ansi256,
    TrueColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportClass {
    Unsupported,
    Compact,
    Standard,
    Wide,
}

impl ViewportClass {
    pub const fn classify(width: u16, height: u16) -> Self {
        if width < 80 || height < 24 {
            Self::Unsupported
        } else if width < 120 || height < 40 {
            Self::Compact
        } else if width < 160 || height < 50 {
            Self::Standard
        } else {
            Self::Wide
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCapabilities {
    pub color_depth: ColorDepth,
    pub reduced_motion: bool,
}

impl TerminalCapabilities {
    pub fn detect() -> Self {
        let no_color = env::var_os("NO_COLOR").is_some();
        let color_term = env::var("COLORTERM")
            .unwrap_or_default()
            .to_ascii_lowercase();
        let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
        let color_depth = detected_color_depth(no_color, &color_term, &term, cfg!(windows));
        Self {
            color_depth,
            reduced_motion: false,
        }
    }

    pub const fn with_reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = reduced_motion;
        self
    }
}

fn detected_color_depth(
    no_color: bool,
    color_term: &str,
    term: &str,
    modern_windows_console: bool,
) -> ColorDepth {
    if no_color || term == "dumb" {
        ColorDepth::Basic
    } else if color_term.contains("truecolor")
        || color_term.contains("24bit")
        || modern_windows_console
    {
        ColorDepth::TrueColor
    } else if term.contains("256color") {
        ColorDepth::Ansi256
    } else {
        ColorDepth::Basic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Ash,
    HighContrast,
}

#[derive(Debug, Clone, Copy)]
pub struct SemanticTheme {
    pub screen: Color,
    pub panel: Color,
    pub felt: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    pub info: Color,
    pub success: Color,
    pub danger: Color,
}

impl SemanticTheme {
    pub fn resolve(mode: ThemeMode, depth: ColorDepth) -> Self {
        match (mode, depth) {
            (ThemeMode::Ash, ColorDepth::TrueColor) => Self {
                screen: Color::Rgb(5, 11, 19),
                panel: Color::Rgb(10, 22, 34),
                felt: Color::Rgb(7, 42, 31),
                border: Color::Rgb(49, 77, 96),
                text: Color::Rgb(229, 237, 240),
                muted: Color::Rgb(133, 157, 171),
                accent: Color::Rgb(243, 174, 54),
                info: Color::Rgb(36, 206, 220),
                success: Color::Rgb(43, 190, 112),
                danger: Color::Rgb(222, 73, 73),
            },
            (ThemeMode::Ash, ColorDepth::Ansi256) => Self {
                screen: Color::Indexed(233),
                panel: Color::Indexed(234),
                felt: Color::Indexed(22),
                border: Color::Indexed(66),
                text: Color::Indexed(255),
                muted: Color::Indexed(109),
                accent: Color::Indexed(214),
                info: Color::Indexed(44),
                success: Color::Indexed(41),
                danger: Color::Indexed(167),
            },
            (ThemeMode::Ash, ColorDepth::Basic) => Self {
                screen: Color::Black,
                panel: Color::Black,
                felt: Color::Black,
                border: Color::DarkGray,
                text: Color::White,
                muted: Color::Gray,
                accent: Color::Yellow,
                info: Color::Cyan,
                success: Color::Green,
                danger: Color::Red,
            },
            (ThemeMode::HighContrast, _) => Self {
                screen: Color::Black,
                panel: Color::Black,
                felt: Color::Black,
                border: Color::White,
                text: Color::White,
                muted: Color::Gray,
                accent: Color::Yellow,
                info: Color::Cyan,
                success: Color::Green,
                danger: Color::LightRed,
            },
        }
    }
}

/// Constrain production renderer colors to the detected terminal palette.
///
/// This post-render boundary lets the approved table keep its semantic palette
/// while ensuring restricted terminals never receive unsupported RGB output.
pub fn apply_terminal_palette(buffer: &mut Buffer, area: Rect, mode: ThemeMode, depth: ColorDepth) {
    if mode == ThemeMode::Ash && depth == ColorDepth::TrueColor {
        return;
    }
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let Some(cell) = buffer.cell_mut((x, y)) else {
                continue;
            };
            cell.fg = constrain_color(cell.fg, mode, depth);
            cell.bg = constrain_color(cell.bg, mode, depth);
        }
    }
}

fn constrain_color(color: Color, mode: ThemeMode, depth: ColorDepth) -> Color {
    let depth = if mode == ThemeMode::HighContrast {
        ColorDepth::Basic
    } else {
        depth
    };
    match (color, depth) {
        (Color::Rgb(red, green, blue), ColorDepth::Ansi256) => {
            Color::Indexed(rgb_to_ansi256(red, green, blue))
        }
        (Color::Rgb(red, green, blue), ColorDepth::Basic) => rgb_to_basic(red, green, blue),
        (Color::Indexed(index), ColorDepth::Basic) => indexed_to_basic(index),
        (other, _) => other,
    }
}

fn rgb_to_ansi256(red: u8, green: u8, blue: u8) -> u8 {
    if red == green && green == blue {
        if red < 8 {
            return 16;
        }
        if red > 248 {
            return 231;
        }
        return 232 + ((u16::from(red) - 8) * 24 / 247) as u8;
    }
    let r = (u16::from(red) * 5 / 255) as u8;
    let g = (u16::from(green) * 5 / 255) as u8;
    let b = (u16::from(blue) * 5 / 255) as u8;
    16 + 36 * r + 6 * g + b
}

fn rgb_to_basic(red: u8, green: u8, blue: u8) -> Color {
    let brightest = red.max(green).max(blue);
    if brightest < 48 {
        Color::Black
    } else if red > green.saturating_add(45) && red > blue.saturating_add(45) {
        Color::Red
    } else if green > red.saturating_add(35) && green > blue.saturating_add(25) {
        Color::Green
    } else if blue > red.saturating_add(35) && blue > green.saturating_add(20) {
        Color::Blue
    } else if red > 140 && green > 100 && blue < 130 {
        Color::Yellow
    } else if green > 100 && blue > 100 && red < 130 {
        Color::Cyan
    } else if red > 110 && blue > 110 && green < 120 {
        Color::Magenta
    } else if u16::from(red) + u16::from(green) + u16::from(blue) > 560 {
        Color::White
    } else {
        Color::Gray
    }
}

fn indexed_to_basic(index: u8) -> Color {
    const BASIC: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    if index < 16 {
        return BASIC[usize::from(index)];
    }
    if index >= 232 {
        return if index >= 247 {
            Color::White
        } else if index >= 239 {
            Color::Gray
        } else {
            Color::Black
        };
    }
    let cube = index - 16;
    let red = cube / 36;
    let green = (cube % 36) / 6;
    let blue = cube % 6;
    rgb_to_basic(red * 51, green * 51, blue * 51)
}

pub struct PresentationEffects {
    manager: EffectManager<()>,
    reduced_motion: bool,
}

impl PresentationEffects {
    pub fn new(reduced_motion: bool) -> Self {
        Self {
            manager: EffectManager::default(),
            reduced_motion,
        }
    }

    pub fn set_reduced_motion(&mut self, reduced_motion: bool) {
        self.reduced_motion = reduced_motion;
        if reduced_motion {
            self.manager = EffectManager::default();
        }
    }

    pub fn begin_route_transition(&mut self, background: Color) {
        if self.reduced_motion {
            return;
        }
        // Route changes replace rather than queue presentation work. This
        // keeps rapid navigation bounded and prevents visual input lag.
        self.manager = EffectManager::default();
        self.manager.add_effect(fx::fade_from_fg(
            background,
            (180_u32, Interpolation::QuadOut),
        ));
    }

    pub fn process(&mut self, elapsed: StdDuration, buffer: &mut Buffer, area: Rect) {
        if self.reduced_motion {
            return;
        }
        self.manager.process_effects(elapsed.into(), buffer, area);
    }

    pub const fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_contract_matches_the_ui_map() {
        assert_eq!(ViewportClass::classify(79, 24), ViewportClass::Unsupported);
        assert_eq!(ViewportClass::classify(80, 24), ViewportClass::Compact);
        assert_eq!(ViewportClass::classify(120, 40), ViewportClass::Standard);
        assert_eq!(ViewportClass::classify(160, 50), ViewportClass::Wide);
    }

    #[test]
    fn semantic_theme_has_deterministic_basic_and_true_color_fallbacks() {
        let true_color = SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::TrueColor);
        let basic = SemanticTheme::resolve(ThemeMode::Ash, ColorDepth::Basic);
        assert_eq!(true_color.accent, Color::Rgb(243, 174, 54));
        assert_eq!(basic.accent, Color::Yellow);
        assert_eq!(basic.text, Color::White);
        assert_ne!(true_color.panel, true_color.screen);
    }

    #[test]
    fn modern_windows_defaults_to_true_color_without_unix_environment_hints() {
        assert_eq!(
            detected_color_depth(false, "", "", true),
            ColorDepth::TrueColor
        );
        assert_eq!(detected_color_depth(true, "", "", true), ColorDepth::Basic);
        assert_eq!(
            detected_color_depth(false, "", "dumb", true),
            ColorDepth::Basic
        );
    }

    #[test]
    fn reduced_motion_never_changes_the_rendered_buffer() {
        let area = Rect::new(0, 0, 8, 2);
        let mut buffer = Buffer::empty(area);
        buffer.set_string(
            0,
            0,
            "SNEAKY",
            ratatui::style::Style::default().fg(Color::Yellow),
        );
        let before = buffer.clone();
        let mut effects = PresentationEffects::new(true);
        effects.begin_route_transition(Color::Black);
        effects.process(StdDuration::from_millis(90), &mut buffer, area);
        assert_eq!(buffer, before);
    }

    #[test]
    fn repeated_route_changes_replace_effects_instead_of_accumulating_work() {
        let area = Rect::new(0, 0, 12, 2);
        let source = Buffer::filled(area, ratatui::buffer::Cell::new("x"));

        let mut single_buffer = source.clone();
        let mut single = PresentationEffects::new(false);
        single.begin_route_transition(Color::Black);
        single.process(StdDuration::from_millis(90), &mut single_buffer, area);

        let mut repeated_buffer = source;
        let mut repeated = PresentationEffects::new(false);
        for _ in 0..1_000 {
            repeated.begin_route_transition(Color::Black);
        }
        repeated.process(StdDuration::from_millis(90), &mut repeated_buffer, area);

        assert_eq!(repeated_buffer, single_buffer);
    }

    #[test]
    fn palette_boundary_removes_rgb_from_restricted_terminal_buffers() {
        let area = Rect::new(0, 0, 3, 1);
        let mut basic = Buffer::empty(area);
        basic.cell_mut((0, 0)).unwrap().fg = Color::Rgb(220, 40, 40);
        basic.cell_mut((1, 0)).unwrap().fg = Color::Rgb(20, 180, 70);
        basic.cell_mut((2, 0)).unwrap().bg = Color::Rgb(20, 20, 30);
        apply_terminal_palette(&mut basic, area, ThemeMode::Ash, ColorDepth::Basic);
        assert_eq!(basic.cell((0, 0)).unwrap().fg, Color::Red);
        assert_eq!(basic.cell((1, 0)).unwrap().fg, Color::Green);
        assert_eq!(basic.cell((2, 0)).unwrap().bg, Color::Black);

        let mut indexed = Buffer::empty(area);
        indexed.cell_mut((0, 0)).unwrap().fg = Color::Rgb(243, 174, 54);
        apply_terminal_palette(&mut indexed, area, ThemeMode::Ash, ColorDepth::Ansi256);
        assert!(matches!(
            indexed.cell((0, 0)).unwrap().fg,
            Color::Indexed(_)
        ));
    }
}
