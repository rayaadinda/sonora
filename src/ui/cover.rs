use std::path::Path;

use image::imageops::FilterType;
use image::RgbImage;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::library::track::Track;
use crate::ui::theme::Theme;

pub struct CoverArt {
    source: Option<RgbImage>,
    cached_size: (u32, u32),
    cached_scaled: Option<RgbImage>,
}

impl Default for CoverArt {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverArt {
    pub fn new() -> Self {
        Self {
            source: None,
            cached_size: (0, 0),
            cached_scaled: None,
        }
    }

    pub fn load_for_track(&mut self, path: &Path) {
        self.cached_size = (0, 0);
        self.cached_scaled = None;
        self.source = Track::load_cover_bytes(path)
            .and_then(|bytes| image::load_from_memory(&bytes).ok())
            .map(|img| img.grayscale().to_rgb8());
    }

    pub fn clear(&mut self) {
        self.source = None;
        self.cached_size = (0, 0);
        self.cached_scaled = None;
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &Theme,
        tick: u64,
        playing: bool,
    ) {
        if area.width < 4 || area.height < 2 {
            return;
        }

        if self.source.is_none() {
            render_vinyl_fallback(frame, area, theme, tick, playing);
            return;
        }

        let target_w = u32::from(area.width);
        let target_h = u32::from(area.height) * 2;

        if self.cached_size != (target_w, target_h) || self.cached_scaled.is_none() {
            if let Some(src) = &self.source {
                let resized =
                    image::imageops::resize(src, target_w, target_h, FilterType::Triangle);
                self.cached_scaled = Some(resized);
                self.cached_size = (target_w, target_h);
            }
        }

        let Some(img) = &self.cached_scaled else {
            return;
        };

        let buf = frame.buffer_mut();
        for row in 0..area.height {
            let py_top = u32::from(row) * 2;
            let py_bot = (py_top + 1).min(img.height().saturating_sub(1));
            for col in 0..area.width {
                let px = u32::from(col).min(img.width().saturating_sub(1));
                let top = img.get_pixel(px, py_top);
                let bot = img.get_pixel(px, py_bot);
                if let Some(cell) = buf.cell_mut((area.x + col, area.y + row)) {
                    cell.set_symbol("▀")
                        .set_fg(Color::Rgb(top[0], top[1], top[2]))
                        .set_bg(Color::Rgb(bot[0], bot[1], bot[2]));
                }
            }
        }
    }
}

fn render_vinyl_fallback(frame: &mut Frame, area: Rect, theme: &Theme, tick: u64, playing: bool) {
    let phase = if playing { (tick / 6) % 4 } else { 0 };
    let glint = match phase {
        0 => "╱",
        1 => "─",
        2 => "╲",
        _ => "│",
    };

    let outer = Style::default().fg(Theme::color(theme.accent_primary));
    let groove = Style::default().fg(Theme::color(theme.muted));
    let label = Style::default()
        .fg(Theme::color(theme.accent_secondary))
        .add_modifier(Modifier::BOLD);
    let hub = Style::default()
        .fg(Theme::color(theme.accent_tertiary))
        .add_modifier(Modifier::BOLD);

    let art = vec![
        Line::from(Span::styled("  ╭──────────╮  ", outer)),
        Line::from(vec![
            Span::styled(" ╭╯ ", outer),
            Span::styled("░▒▓██▓▒░", groove),
            Span::styled(" ╰╮ ", outer),
        ]),
        Line::from(vec![
            Span::styled(" │ ", outer),
            Span::styled("▒▓", groove),
            Span::styled(" ╭──╮ ", label),
            Span::styled("▓▒", groove),
            Span::styled(" │ ", outer),
        ]),
        Line::from(vec![
            Span::styled(" │ ", outer),
            Span::styled("▓█", groove),
            Span::styled(" │", label),
            Span::styled(format!("{glint}◉"), hub),
            Span::styled("│ ", label),
            Span::styled("█▓", groove),
            Span::styled(" │ ", outer),
        ]),
        Line::from(vec![
            Span::styled(" │ ", outer),
            Span::styled("▒▓", groove),
            Span::styled(" ╰──╯ ", label),
            Span::styled("▓▒", groove),
            Span::styled(" │ ", outer),
        ]),
        Line::from(vec![
            Span::styled(" ╰╮ ", outer),
            Span::styled("░▒▓██▓▒░", groove),
            Span::styled(" ╭╯ ", outer),
        ]),
        Line::from(Span::styled("  ╰──────────╯  ", outer)),
    ];

    let top_pad = area.height.saturating_sub(art.len() as u16) / 2;
    let centered = Rect {
        x: area.x,
        y: area.y + top_pad,
        width: area.width,
        height: area.height.saturating_sub(top_pad),
    };

    frame.render_widget(
        Paragraph::new(art)
            .alignment(Alignment::Center)
            .style(Style::default().bg(Theme::color(theme.surface))),
        centered,
    );
}
