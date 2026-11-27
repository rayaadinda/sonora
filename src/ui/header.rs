use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph};
use ratatui::Frame;

use crate::library::track::Track;
use crate::ui::theme::{Theme, ThemeKind};

const ASCII_BANNER: [&str; 6] = [
    "███████╗ ██████╗ ███╗   ██╗ ██████╗ ██████╗  █████╗ ",
    "██╔════╝██╔═══██╗████╗  ██║██╔═══██╗██╔══██╗██╔══██╗",
    "███████╗██║   ██║██╔██╗ ██║██║   ██║██████╔╝███████║",
    "╚════██║██║   ██║██║╚██╗██║██║   ██║██╔══██╗██╔══██║",
    "███████║╚██████╔╝██║ ╚████║╚██████╔╝██║  ██║██║  ██║",
    "╚══════╝ ╚═════╝ ╚═╝  ╚═══╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝",
];

pub fn desired_height(terminal_height: u16, terminal_width: u16) -> u16 {
    if terminal_height >= 26 && terminal_width >= 74 {
        8
    } else {
        3
    }
}

pub struct HeaderView<'a> {
    pub theme_kind: ThemeKind,
    pub theme: &'a Theme,
    pub active_track: Option<&'a Track>,
    pub total_tracks: usize,
    pub scanning: bool,
    pub discord_rpc: bool,
}

pub fn render(frame: &mut Frame, area: Rect, view: &HeaderView<'_>) {
    let theme = view.theme;
    let theme_kind = view.theme_kind;
    let active_track = view.active_track;
    let total_tracks = view.total_tracks;
    let scanning = view.scanning;
    let discord_rpc = view.discord_rpc;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.border)))
        .style(Style::default().bg(Theme::color(theme.surface)))
        .padding(Padding::horizontal(1));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height < 5 || inner.width < 60 {
        render_compact(
            frame,
            inner,
            theme_kind,
            theme,
            active_track,
            total_tracks,
            scanning,
        );
        return;
    }

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(54), Constraint::Min(18)])
        .split(inner);

    let banner_lines: Vec<Line> = ASCII_BANNER
        .iter()
        .map(|row| gradient_line(row, theme))
        .collect();

    frame.render_widget(Paragraph::new(banner_lines), columns[0]);

    let (quality_line, detail_line) = match active_track {
        Some(track) => (
            Line::from(vec![
                Span::styled(
                    "◆ FLAC ",
                    Style::default()
                        .fg(Theme::color(theme.accent_primary))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    track.quality_label(),
                    Style::default()
                        .fg(Theme::color(theme.text))
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    track.bitrate_label(),
                    Style::default().fg(Theme::color(theme.accent_secondary)),
                ),
                Span::styled(" • ", Style::default().fg(Theme::color(theme.muted))),
                Span::styled(
                    track.channel_label(),
                    Style::default().fg(Theme::color(theme.accent_tertiary)),
                ),
            ]),
        ),
        None => (
            Line::from(vec![
                Span::styled(
                    "◆ LOSSLESS ",
                    Style::default()
                        .fg(Theme::color(theme.accent_primary))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("FLAC ENGINE", Style::default().fg(Theme::color(theme.text))),
            ]),
            Line::from(Span::styled(
                "Select a track and press Enter",
                Style::default().fg(Theme::color(theme.muted)),
            )),
        ),
    };

    let library_status = if scanning {
        format!("Scanning... {total_tracks} tracks")
    } else if total_tracks == 1 {
        String::from("1 FLAC track")
    } else {
        format!("{total_tracks} FLAC tracks")
    };

    let info_lines = vec![
        Line::from(Span::styled(
            "AUDIOPHILE TUI PLAYER",
            Style::default()
                .fg(Theme::color(theme.muted))
                .add_modifier(Modifier::BOLD),
        )),
        quality_line,
        detail_line,
        Line::from(""),
        Line::from(vec![
            Span::styled("Theme: ", Style::default().fg(Theme::color(theme.muted))),
            Span::styled(
                theme_kind.name(),
                Style::default().fg(Theme::color(theme.accent_secondary)),
            ),
            Span::styled(" • RPC: ", Style::default().fg(Theme::color(theme.muted))),
            Span::styled(
                if discord_rpc { "ON" } else { "OFF" },
                Style::default().fg(if discord_rpc {
                    Theme::color(theme.accent_primary)
                } else {
                    Theme::color(theme.muted)
                }),
            ),
        ]),
        Line::from(vec![
            Span::styled("Library: ", Style::default().fg(Theme::color(theme.muted))),
            Span::styled(
                library_status,
                Style::default().fg(Theme::color(theme.success)),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(info_lines).alignment(Alignment::Right),
        columns[1],
    );
}

fn render_compact(
    frame: &mut Frame,
    area: Rect,
    theme_kind: ThemeKind,
    theme: &Theme,
    active_track: Option<&Track>,
    total_tracks: usize,
    scanning: bool,
) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let title = Line::from(vec![
        Span::styled(
            "◆ SONORA ",
            Style::default()
                .fg(Theme::color(theme.accent_primary))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "FLAC",
            Style::default()
                .fg(Theme::color(theme.accent_secondary))
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let status = match active_track {
        Some(track) => format!(
            "{} • {} • {}",
            track.quality_label(),
            track.channel_label(),
            theme_kind.name()
        ),
        None if scanning => format!("Scanning... {total_tracks} tracks"),
        None => format!("{total_tracks} FLAC • {}", theme_kind.name()),
    };

    frame.render_widget(Paragraph::new(title), columns[0]);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            status,
            Style::default().fg(Theme::color(theme.muted)),
        )))
        .alignment(Alignment::Right),
        columns[1],
    );
}

fn gradient_line(row: &str, theme: &Theme) -> Line<'static> {
    let total = row.chars().count().max(1);
    let spans: Vec<Span<'static>> = row
        .chars()
        .enumerate()
        .map(|(idx, ch)| {
            let t = idx as f32 / (total.saturating_sub(1).max(1)) as f32;
            let color = Theme::gradient_three(
                theme.accent_primary,
                theme.accent_secondary,
                theme.accent_tertiary,
                t,
            );
            Span::styled(
                ch.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    Line::from(spans)
}
