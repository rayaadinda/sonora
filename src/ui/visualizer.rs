use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Borders, Padding};
use ratatui::Frame;

use crate::audio::spectrum::SpectrumAnalyzer;
use crate::config::VisualizerMode;
use crate::ui::theme::Theme;

const BLOCK_GLYPHS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];

pub fn band_count_for_width(area_width: u16) -> usize {
    let inner = area_width.saturating_sub(4);
    usize::from((inner / 2).max(8))
}

pub fn render(
    frame: &mut Frame,
    area: Rect,
    analyzer: &SpectrumAnalyzer,
    mode: VisualizerMode,
    theme: &Theme,
) {
    let title = Span::styled(
        format!(" ∿ {} ", mode.label()),
        Style::default().fg(Theme::color(theme.accent_primary)),
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.border)))
        .style(Style::default().bg(Theme::color(theme.surface)))
        .padding(Padding::horizontal(1))
        .title(title);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 4 || inner.height < 2 {
        return;
    }

    match mode {
        VisualizerMode::Bars => render_bars(frame, inner, analyzer, theme),
        VisualizerMode::Mirror => render_mirror(frame, inner, analyzer, theme),
        VisualizerMode::Wave => render_wave(frame, inner, analyzer, theme),
    }
}

fn render_bars(frame: &mut Frame, area: Rect, analyzer: &SpectrumAnalyzer, theme: &Theme) {
    let bars = analyzer.bars();
    let peaks = analyzer.peaks();
    if bars.is_empty() {
        return;
    }

    let step = if usize::from(area.width) >= bars.len() * 2 {
        2u16
    } else {
        1u16
    };
    let max_cols = usize::from(area.width / step).min(bars.len());
    let offset_x = (area.width.saturating_sub(max_cols as u16 * step)) / 2;
    let total_units = f32::from(area.height) * 8.0;
    let bg = Theme::color(theme.surface);
    let buf = frame.buffer_mut();

    for (col_idx, &bar) in bars.iter().enumerate().take(max_cols) {
        let value = bar.clamp(0.0, 1.0);
        let peak = peaks.get(col_idx).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let filled_units = (value * total_units).round() as usize;
        let full_rows = filled_units / 8;
        let rem = filled_units % 8;
        let x = area.x + offset_x + (col_idx as u16) * step;

        for row_from_bottom in 0..usize::from(area.height) {
            let y = area.y + area.height - 1 - row_from_bottom as u16;
            let height_t = (row_from_bottom as f32 + 0.5) / f32::from(area.height).max(1.0);
            let fg = Theme::gradient_three(theme.bar_low, theme.bar_mid, theme.bar_high, height_t);

            let symbol = if row_from_bottom < full_rows {
                Some("█")
            } else if row_from_bottom == full_rows && rem > 0 {
                Some(BLOCK_GLYPHS[rem - 1])
            } else {
                None
            };

            if let Some(glyph) = symbol {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(glyph).set_fg(fg).set_bg(bg);
                }
            }
        }

        if peak > 0.04 {
            let peak_row = ((peak * f32::from(area.height.saturating_sub(1))).round() as u16)
                .min(area.height.saturating_sub(1));
            let peak_y = area.y + area.height - 1 - peak_row;
            if peak_row as usize >= full_rows {
                if let Some(cell) = buf.cell_mut((x, peak_y)) {
                    cell.set_symbol("━")
                        .set_fg(Theme::color(theme.peak))
                        .set_bg(bg);
                }
            }
        }
    }
}

fn render_mirror(frame: &mut Frame, area: Rect, analyzer: &SpectrumAnalyzer, theme: &Theme) {
    let bars = analyzer.bars();
    if bars.is_empty() {
        return;
    }

    let step = if usize::from(area.width) >= bars.len() * 2 {
        2u16
    } else {
        1u16
    };
    let max_cols = usize::from(area.width / step).min(bars.len());
    let offset_x = (area.width.saturating_sub(max_cols as u16 * step)) / 2;
    let center = area.y + area.height / 2;
    let half_height = (area.height / 2).max(1);
    let bg = Theme::color(theme.surface);
    let buf = frame.buffer_mut();

    for x in area.x..(area.x + area.width) {
        if let Some(cell) = buf.cell_mut((x, center)) {
            cell.set_symbol("─")
                .set_fg(Theme::color(theme.border))
                .set_bg(bg);
        }
    }

    for (col_idx, &bar) in bars.iter().enumerate().take(max_cols) {
        let value = bar.clamp(0.0, 1.0);
        let reach = (value * f32::from(half_height)).round() as u16;
        let x = area.x + offset_x + (col_idx as u16) * step;

        for dist in 0..=reach {
            let t = f32::from(dist) / f32::from(half_height).max(1.0);
            let color = Theme::gradient_three(theme.bar_low, theme.bar_mid, theme.bar_high, t);

            if let Some(up_y) = center.checked_sub(dist) {
                if up_y >= area.y {
                    if let Some(cell) = buf.cell_mut((x, up_y)) {
                        let glyph = if dist == reach && reach > 0 {
                            "▀"
                        } else {
                            "█"
                        };
                        cell.set_symbol(glyph).set_fg(color).set_bg(bg);
                    }
                }
            }

            let down_y = center.saturating_add(dist);
            if down_y < area.y + area.height {
                if let Some(cell) = buf.cell_mut((x, down_y)) {
                    let glyph = if dist == reach && reach > 0 {
                        "▄"
                    } else {
                        "█"
                    };
                    cell.set_symbol(glyph).set_fg(color).set_bg(bg);
                }
            }
        }
    }
}

fn render_wave(frame: &mut Frame, area: Rect, analyzer: &SpectrumAnalyzer, theme: &Theme) {
    let samples = analyzer.waveform();
    if samples.is_empty() {
        return;
    }

    let bg = Theme::color(theme.surface);
    let mid_y = f32::from(area.height.saturating_sub(1)) / 2.0;
    let buf = frame.buffer_mut();

    for col in 0..area.width {
        let x = area.x + col;
        let mid_row = area.y + area.height / 2;
        if let Some(cell) = buf.cell_mut((x, mid_row)) {
            cell.set_symbol("·")
                .set_fg(Theme::color(theme.border))
                .set_bg(bg);
        }

        let frac = f32::from(col) / f32::from(area.width.saturating_sub(1).max(1));
        let idx = ((frac * (samples.len().saturating_sub(1)) as f32).round() as usize)
            .min(samples.len() - 1);
        let amp = (samples[idx] * 1.45).clamp(-1.0, 1.0);
        let y_offset = (mid_y - amp * mid_y).round() as u16;
        let target_y = (area.y + y_offset).min(area.y + area.height.saturating_sub(1));
        let color = Theme::gradient_three(
            theme.accent_primary,
            theme.accent_secondary,
            theme.accent_tertiary,
            frac,
        );

        let (start_y, end_y) = if target_y <= mid_row {
            (target_y, mid_row)
        } else {
            (mid_row, target_y)
        };

        for y in start_y..=end_y {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let glyph = if y == target_y { "●" } else { "│" };
                let cell_color = if y == target_y {
                    color
                } else {
                    Theme::lerp(theme.surface, theme.accent_primary, 0.45)
                };
                cell.set_symbol(glyph).set_fg(cell_color).set_bg(bg);
            }
        }
    }
}
