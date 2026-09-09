//! Embedded branding from the approved standalone menu. ShellApp owns navigation.
use super::shell::HOME_ITEMS;
use image::{imageops, DynamicImage, Rgba, RgbaImage};
use ratatui::{
    layout::{Rect, Size},
    style::{Color, Style},
    widgets::Paragraph,
};
use ratatui_image::{
    picker::{Picker, ProtocolType},
    protocol::Protocol,
    Image, Resize,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const BG: Color = Color::Rgb(12, 13, 12);
const IVORY: Color = Color::Rgb(231, 224, 208);
const MUTED: Color = Color::Rgb(159, 154, 141);
const RULE: Color = Color::Rgb(66, 65, 59);
const RED: Color = Color::Rgb(190, 30, 39);

pub struct BrandedMenu {
    picker: Picker,
    logo: DynamicImage,
    portrait: DynamicImage,
    cached: Option<(Size, Layout, Protocol, Protocol)>,
}
impl BrandedMenu {
    /// Call once in alternate-screen mode, before the shell event reader starts.
    /// Unsupported graphics and failed queries retain the ordinary accessible menu.
    pub fn detect() -> Option<Self> {
        let picker = Picker::from_query_stdio().ok()?;
        if picker.protocol_type() == ProtocolType::Halfblocks {
            return None;
        }
        Some(Self {
            picker,
            logo: image::load_from_memory(include_bytes!("../../assets/branding/wordmark.png"))
                .ok()?,
            portrait: image::load_from_memory(include_bytes!("../../assets/branding/portrait.png"))
                .ok()?,
            cached: None,
        })
    }
    pub fn prepare(&mut self, size: Size) -> bool {
        let Some(positions) = layout(size) else {
            self.cached = None;
            return false;
        };
        if self
            .cached
            .as_ref()
            .is_some_and(|(cached, ..)| *cached == size)
        {
            return true;
        }
        match (
            encode(&self.picker, &self.logo, positions.logo, true),
            encode(&self.picker, &self.portrait, positions.portrait, false),
        ) {
            (Ok(logo), Ok(portrait)) => {
                self.cached = Some((size, positions, logo, portrait));
                true
            }
            _ => {
                self.cached = None;
                false
            }
        }
    }
    pub fn render(&self, frame: &mut ratatui::Frame, selected: usize) {
        if let Some((_, positions, logo, portrait)) = &self.cached {
            paint(frame, *positions, logo, portrait, selected);
        }
    }
}

#[derive(Clone, Copy)]
struct Layout {
    logo: Rect,
    portrait: Rect,
    divider: u16,
    menu: Rect,
    brand_x: u16,
}

fn layout(size: Size) -> Option<Layout> {
    if size.width < 100 || size.height < 36 {
        return None;
    }
    let left = size.width * 60 / 100;
    let brand_width = (left - 12).min(66);
    let brand_x = (left - brand_width) / 2;
    Some(Layout {
        logo: Rect::new(brand_x, 2, brand_width, 10),
        portrait: Rect::new(4, 17, left - 8, size.height - 19),
        divider: left + 2,
        menu: Rect::new(
            left + 8,
            ((size.height - 24) / 2).max(7),
            size.width - left - 15,
            24,
        ),
        brand_x,
    })
}

fn encode(picker: &Picker, image: &DynamicImage, area: Rect, align_left: bool) -> Result<Protocol> {
    let font = picker.font_size();
    let width = u32::from(area.width) * u32::from(font.width);
    let height = u32::from(area.height) * u32::from(font.height);
    let resized = image
        .resize(width, height, imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut canvas = RgbaImage::from_pixel(width, height, Rgba([12, 13, 12, 255]));
    imageops::overlay(
        &mut canvas,
        &resized,
        if align_left {
            0
        } else {
            i64::from((width - resized.width()) / 2)
        },
        0,
    );
    Ok(picker.new_protocol(
        DynamicImage::ImageRgba8(canvas),
        Size::new(area.width, area.height),
        Resize::Fit(None),
    )?)
}

fn paint(
    frame: &mut ratatui::Frame,
    positions: Layout,
    logo: &Protocol,
    portrait: &Protocol,
    selected: usize,
) {
    let area = frame.area();
    frame.render_widget(Paragraph::new("").style(Style::default().bg(BG)), area);
    frame.render_widget(Image::new(logo), positions.logo);
    frame.render_widget(Image::new(portrait), positions.portrait);
    let mut text = |x, y, width, value: String, fg, bg| {
        frame.render_widget(
            Paragraph::new(value).style(Style::default().fg(fg).bg(bg)),
            Rect::new(x, y, width, 1),
        );
    };
    text(
        positions.brand_x,
        13,
        25,
        "P O K E R   C L U B".into(),
        IVORY,
        BG,
    );
    text(
        positions.brand_x,
        15,
        25,
        "PLAY SMARTER STAY SNEAKY".into(),
        MUTED,
        BG,
    );
    for y in 3..area.height - 3 {
        text(positions.divider, y, 1, "│".into(), RULE, BG);
    }
    let m = positions.menu;
    text(m.x, m.y, m.width, "M A I N   M E N U".into(), MUTED, BG);
    for y in [m.y + 2, m.y + 23] {
        text(m.x, y, m.width, "─".repeat(m.width.into()), RULE, BG);
    }
    for (i, (label, _, _)) in HOME_ITEMS.iter().enumerate() {
        let y = m.y + 5 + i as u16 * 3;
        if i == selected {
            for row in y - 1..=y + 1 {
                text(m.x, row, m.width, " ".repeat(m.width.into()), BG, IVORY);
                text(m.x, row, 1, "▏".into(), RED, IVORY);
            }
            text(m.x + 3, y, m.width - 3, format!("> {label}"), BG, IVORY);
        } else {
            text(
                m.x + 5,
                y,
                m.width - 5,
                label.to_string(),
                if i == 3 { MUTED } else { IVORY },
                BG,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn images_and_menu_do_not_overlap_or_touch_bottom_row() {
        for (w, h) in [(100, 36), (120, 40), (160, 50), (200, 60)] {
            let l = layout(Size::new(w, h)).unwrap();
            assert!(l.logo.right() < l.divider);
            assert!(l.portrait.right() < l.divider);
            assert!(l.portrait.bottom() < h - 1);
            assert!(l.menu.right() <= w && l.menu.bottom() < h);
        }
        assert!(layout(Size::new(80, 24)).is_none());
    }
    #[test]
    fn selection_and_resize_use_shared_menu_items_without_clipping() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut menu = BrandedMenu {
            picker: Picker::halfblocks(),
            logo: image::load_from_memory(include_bytes!("../../assets/branding/wordmark.png"))
                .unwrap(),
            portrait: image::load_from_memory(include_bytes!("../../assets/branding/portrait.png"))
                .unwrap(),
            cached: None,
        };
        assert!(!menu.prepare(Size::new(80, 24)));
        assert!(menu.prepare(Size::new(100, 36)));
        for selected in 0..HOME_ITEMS.len() {
            let mut terminal = Terminal::new(TestBackend::new(100, 36)).unwrap();
            terminal.draw(|frame| menu.render(frame, selected)).unwrap();
            let buffer = terminal.backend().buffer();
            let text = buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            for (label, _, _) in HOME_ITEMS {
                assert!(text.contains(label));
            }
            let positions = layout(Size::new(100, 36)).unwrap();
            let selected_y = positions.menu.y + 5 + selected as u16 * 3;
            assert_eq!(buffer[(positions.menu.x + 3, selected_y)].symbol(), ">");
            assert_eq!(buffer[(positions.menu.x + 3, selected_y)].bg, IVORY);
        }
        assert!(!menu.prepare(Size::new(99, 36)));
        assert!(menu.cached.is_none());
        assert!(menu.prepare(Size::new(120, 40)));
    }

    #[test]
    fn embedded_art_retains_bitmap_resolution() {
        let portrait =
            image::load_from_memory(include_bytes!("../../assets/branding/portrait.png")).unwrap();
        assert_eq!((portrait.width(), portrait.height()), (748, 850));
    }
}
