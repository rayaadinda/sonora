use std::borrow::Cow;
use std::path::PathBuf;
use std::time::Duration;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, List, ListItem, ListState, Padding, Paragraph, Wrap,
};
use ratatui::Frame;

use crate::audio::spectrum::SpectrumAnalyzer;
use crate::config::{RepeatMode, VisualizerMode};
use crate::library::track::{format_duration, Track};
use crate::ui::cover::CoverArt;
use crate::ui::header;
use crate::ui::theme::{Theme, ThemeKind};
use crate::ui::visualizer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search,
    FolderModal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderPreview {
    Empty,
    ValidDir,
    ValidFile,
    NotFound,
}

#[derive(Debug, Clone, Copy)]
pub struct ToastView<'a> {
    pub message: &'a str,
    pub is_error: bool,
}

pub struct UiSnapshot<'a> {
    pub theme_kind: ThemeKind,
    pub tracks: &'a [Track],
    pub visible_indices: &'a [usize],
    pub playing_index: Option<usize>,
    pub is_playing: bool,
    pub is_paused: bool,
    pub scanning: bool,
    pub position: Duration,
    pub duration: Duration,
    pub volume: f32,
    pub muted: bool,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub visualizer_mode: VisualizerMode,
    pub discord_rpc: bool,
    pub input_mode: InputMode,
    pub search_query: &'a str,
    pub folders: &'a [PathBuf],
    pub folder_input: &'a str,
    pub folder_selected: Option<usize>,
    pub folder_preview: FolderPreview,
    pub show_help: bool,
    pub toast: Option<ToastView<'a>>,
    pub tick: u64,
    pub root_label: &'a str,
}

pub fn render(
    frame: &mut Frame,
    snapshot: &UiSnapshot<'_>,
    list_state: &mut ListState,
    cover: &mut CoverArt,
    analyzer: &SpectrumAnalyzer,
) -> u16 {
    let theme = snapshot.theme_kind.palette();
    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(Theme::color(theme.bg))),
        area,
    );

    let header_h = header::desired_height(area.height, area.width);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Min(10),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    let active_track = snapshot
        .playing_index
        .and_then(|idx| snapshot.tracks.get(idx));

    header::render(
        frame,
        rows[0],
        &header::HeaderView {
            theme_kind: snapshot.theme_kind,
            theme: &theme,
            active_track,
            total_tracks: snapshot.tracks.len(),
            scanning: snapshot.scanning,
            discord_rpc: snapshot.discord_rpc,
        },
    );

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(rows[1]);

    render_library(frame, body[0], snapshot, &theme, list_state);

    let right_panes = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(11), Constraint::Min(6)])
        .split(body[1]);

    render_now_playing(frame, right_panes[0], snapshot, active_track, &theme, cover);
    visualizer::render(
        frame,
        right_panes[1],
        analyzer,
        snapshot.visualizer_mode,
        &theme,
    );

    render_transport(frame, rows[2], snapshot, &theme);
    render_footer(frame, rows[3], snapshot, &theme);

    if snapshot.input_mode == InputMode::FolderModal {
        render_folder_modal(frame, area, snapshot, &theme);
    } else if snapshot.show_help {
        render_help_modal(frame, area, &theme);
    }

    right_panes[1].width
}

fn render_library(
    frame: &mut Frame,
    area: Rect,
    snapshot: &UiSnapshot<'_>,
    theme: &Theme,
    list_state: &mut ListState,
) {
    let is_searching = snapshot.input_mode == InputMode::Search;
    let border_color = if is_searching {
        Theme::color(theme.accent_primary)
    } else {
        Theme::color(theme.border_focus)
    };

    let count_badge = if snapshot.search_query.is_empty() {
        format!(" ♫ Library [{}] ", snapshot.tracks.len())
    } else {
        format!(
            " ♫ Library [{}/{}] ",
            snapshot.visible_indices.len(),
            snapshot.tracks.len()
        )
    };

    let selected_pos = list_state
        .selected()
        .map(|i| format!(" {}/{} ", i + 1, snapshot.visible_indices.len().max(1)))
        .unwrap_or_default();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(Theme::color(theme.surface)))
        .title(Span::styled(
            count_badge,
            Style::default()
                .fg(Theme::color(theme.accent_primary))
                .add_modifier(Modifier::BOLD),
        ))
        .title(
            Line::from(Span::styled(
                selected_pos,
                Style::default().fg(Theme::color(theme.muted)),
            ))
            .alignment(Alignment::Right),
        );

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let show_search_bar = is_searching || !snapshot.search_query.is_empty();
    let (search_area, list_area) = if show_search_bar && inner.height > 2 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);
        (Some(chunks[0]), chunks[1])
    } else {
        (None, inner)
    };

    if let Some(s_area) = search_area {
        let cursor = if is_searching && ((snapshot.tick / 12) & 1) == 0 {
            "▏"
        } else {
            ""
        };
        let line = Line::from(vec![
            Span::styled(
                " / ",
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}{cursor}", snapshot.search_query),
                Style::default().fg(Theme::color(theme.text)),
            ),
        ]);
        frame.render_widget(
            Paragraph::new(line).style(Style::default().bg(Theme::color(theme.selection_bg))),
            s_area,
        );
    }

    if snapshot.visible_indices.is_empty() {
        let empty_msg = if snapshot.scanning {
            format!("Scanning FLAC files in:\n{}", snapshot.root_label)
        } else if !snapshot.search_query.is_empty() {
            format!("No tracks matching \"{}\"", snapshot.search_query)
        } else {
            format!(
                "No .flac files found in:\n{}\n\nPress 'o' to add a FLAC folder path.",
                snapshot.root_label
            )
        };
        frame.render_widget(
            Paragraph::new(empty_msg)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(Theme::color(theme.muted))),
            Rect {
                x: list_area.x.saturating_add(1),
                y: list_area.y.saturating_add(list_area.height / 4),
                width: list_area.width.saturating_sub(2),
                height: list_area.height.saturating_sub(list_area.height / 4),
            },
        );
        return;
    }

    let row_width = usize::from(list_area.width.saturating_sub(2));
    let items: Vec<ListItem> = snapshot
        .visible_indices
        .iter()
        .enumerate()
        .filter_map(|(pos, &track_idx)| {
            let track = snapshot.tracks.get(track_idx)?;
            let is_active = snapshot.playing_index == Some(track_idx);
            let indicator = if is_active && snapshot.is_playing {
                " ▶ "
            } else if is_active && snapshot.is_paused {
                " ❚❚"
            } else if is_active {
                " ● "
            } else {
                "   "
            };

            let num_str = format!("{:02} ", pos + 1);
            let duration_str = format_duration(track.duration);
            let prefix_len = 3 + num_str.len();
            let suffix_len = duration_str.len() + 2;
            let text_budget = row_width.saturating_sub(prefix_len + suffix_len);

            let label: Cow<'_, str> = if text_budget > 26 {
                Cow::Owned(format!("{} — {}", track.title, track.artist))
            } else {
                Cow::Borrowed(&track.title)
            };
            let truncated = truncate_str(&label, text_budget);
            let pad = text_budget.saturating_sub(truncated.chars().count());

            let num_style = if is_active {
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::color(theme.muted))
            };

            let title_style = if is_active {
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Theme::color(theme.text))
            };

            let line = Line::from(vec![
                Span::styled(indicator, num_style),
                Span::styled(num_str, num_style),
                Span::styled(truncated, title_style),
                Span::raw(" ".repeat(pad + 1)),
                Span::styled(duration_str, Style::default().fg(Theme::color(theme.muted))),
            ]);

            Some(ListItem::new(line))
        })
        .collect();

    let list = List::new(items)
        .highlight_style(
            Style::default()
                .bg(Theme::color(theme.selection_bg))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▎");

    frame.render_stateful_widget(list, list_area, list_state);
}

fn render_now_playing(
    frame: &mut Frame,
    area: Rect,
    snapshot: &UiSnapshot<'_>,
    active_track: Option<&Track>,
    theme: &Theme,
    cover: &mut CoverArt,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.border)))
        .style(Style::default().bg(Theme::color(theme.surface)))
        .padding(Padding::horizontal(1))
        .title(Span::styled(
            " ◉ Now Playing ",
            Style::default()
                .fg(Theme::color(theme.accent_secondary))
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let show_cover = inner.width >= 38 && inner.height >= 7;
    let info_area = if show_cover {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(18),
                Constraint::Length(2),
                Constraint::Min(16),
            ])
            .split(inner);

        cover.render(frame, cols[0], theme, snapshot.tick, snapshot.is_playing);
        cols[2]
    } else {
        inner
    };

    let lines = match active_track {
        Some(track) => {
            let album_year = match &track.year {
                Some(year) => format!("{} ({year})", track.album),
                None => track.album.clone(),
            };
            let track_tag = track
                .track_number
                .map(|n| format!("Track #{n:02}"))
                .unwrap_or_else(|| String::from("Lossless Track"));

            let shuffle_style = if snapshot.shuffle {
                Style::default()
                    .fg(Theme::color(theme.bg))
                    .bg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Theme::color(theme.muted))
                    .bg(Theme::color(theme.selection_bg))
            };

            let repeat_style = if snapshot.repeat != RepeatMode::Off {
                Style::default()
                    .fg(Theme::color(theme.bg))
                    .bg(Theme::color(theme.accent_secondary))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Theme::color(theme.muted))
                    .bg(Theme::color(theme.selection_bg))
            };

            vec![
                Line::from(Span::styled(
                    truncate_str(&track.title, usize::from(info_area.width)),
                    Style::default()
                        .fg(Theme::color(theme.text))
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    truncate_str(&track.artist, usize::from(info_area.width)),
                    Style::default()
                        .fg(Theme::color(theme.accent_secondary))
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    truncate_str(&album_year, usize::from(info_area.width)),
                    Style::default().fg(Theme::color(theme.muted)),
                )),
                Line::from(Span::styled(
                    track_tag,
                    Style::default().fg(Theme::color(theme.muted)),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        " FLAC ",
                        Style::default()
                            .fg(Theme::color(theme.bg))
                            .bg(Theme::color(theme.accent_primary))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!(" {} ", track.quality_label()),
                        Style::default()
                            .fg(Theme::color(theme.text))
                            .bg(Theme::color(theme.selection_bg)),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!(" {} ", track.bitrate_label()),
                        Style::default()
                            .fg(Theme::color(theme.accent_tertiary))
                            .bg(Theme::color(theme.selection_bg)),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        if snapshot.shuffle {
                            " ⇄ Shuffle: ON "
                        } else {
                            " ⇄ Shuffle: OFF "
                        },
                        shuffle_style,
                    ),
                    Span::raw(" "),
                    Span::styled(
                        format!(" ↻ Repeat: {} ", snapshot.repeat.label()),
                        repeat_style,
                    ),
                ]),
            ]
        }
        None => vec![
            Line::from(""),
            Line::from(Span::styled(
                "Ready for Lossless Playback",
                Style::default()
                    .fg(Theme::color(theme.text))
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Choose a FLAC track from your library and press Enter.",
                Style::default().fg(Theme::color(theme.muted)),
            )),
            Line::from(Span::styled(
                "Press ? at any time to view all keybindings.",
                Style::default().fg(Theme::color(theme.accent_primary)),
            )),
        ],
    };

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), info_area);
}

fn render_transport(frame: &mut Frame, area: Rect, snapshot: &UiSnapshot<'_>, theme: &Theme) {
    let (status_text, status_color) = if snapshot.is_playing {
        (" ▶ Playing ", Theme::color(theme.success))
    } else if snapshot.is_paused {
        (" ❚❚ Paused ", Theme::color(theme.warning))
    } else {
        (" ■ Idle ", Theme::color(theme.muted))
    };

    let vol_pct = (snapshot.volume * 100.0).round() as u16;
    let vol_filled = ((snapshot.volume / 1.5) * 10.0).round().clamp(0.0, 10.0) as usize;
    let vol_empty = 10usize.saturating_sub(vol_filled);
    let volume_spans = if snapshot.muted {
        vec![Span::styled(
            " ✕ Muted ",
            Style::default()
                .fg(Theme::color(theme.accent_tertiary))
                .add_modifier(Modifier::BOLD),
        )]
    } else {
        vec![
            Span::styled(" Vol ", Style::default().fg(Theme::color(theme.muted))),
            Span::styled(
                "█".repeat(vol_filled),
                Style::default().fg(Theme::color(theme.accent_primary)),
            ),
            Span::styled(
                "░".repeat(vol_empty),
                Style::default().fg(Theme::color(theme.border)),
            ),
            Span::styled(
                format!(" {vol_pct}% "),
                Style::default()
                    .fg(Theme::color(theme.text))
                    .add_modifier(Modifier::BOLD),
            ),
        ]
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.border)))
        .style(Style::default().bg(Theme::color(theme.surface)))
        .padding(Padding::horizontal(1))
        .title(Span::styled(
            status_text,
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ))
        .title(Line::from(volume_spans).alignment(Alignment::Right));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width < 16 || inner.height == 0 {
        return;
    }

    let elapsed = format_duration(snapshot.position);
    let total = format_duration(snapshot.duration);
    let ratio = if snapshot.duration.is_zero() {
        0.0
    } else {
        (snapshot.position.as_secs_f64() / snapshot.duration.as_secs_f64()).clamp(0.0, 1.0)
    };

    let bar_width = usize::from(inner.width).saturating_sub(elapsed.len() + total.len() + 4);
    let filled = ((bar_width as f64) * ratio).round() as usize;
    let filled = filled.min(bar_width);

    let mut spans = Vec::with_capacity(bar_width + 4);
    spans.push(Span::styled(
        format!("{elapsed}  "),
        Style::default()
            .fg(Theme::color(theme.text))
            .add_modifier(Modifier::BOLD),
    ));

    for i in 0..bar_width {
        let t = i as f32 / bar_width.saturating_sub(1).max(1) as f32;
        if i < filled.saturating_sub(1) {
            let color = Theme::lerp(theme.accent_primary, theme.accent_secondary, t);
            spans.push(Span::styled("━", Style::default().fg(color)));
        } else if i == filled.saturating_sub(1) && filled > 0 {
            spans.push(Span::styled(
                "●",
                Style::default()
                    .fg(Theme::color(theme.peak))
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                "─",
                Style::default().fg(Theme::color(theme.border)),
            ));
        }
    }

    spans.push(Span::styled(
        format!("  {total}"),
        Style::default().fg(Theme::color(theme.muted)),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), inner);
}

fn render_footer(frame: &mut Frame, area: Rect, snapshot: &UiSnapshot<'_>, theme: &Theme) {
    if let Some(toast) = snapshot.toast {
        let color = if toast.is_error {
            Theme::color(theme.accent_tertiary)
        } else {
            Theme::color(theme.accent_primary)
        };
        let line = Line::from(vec![
            Span::styled(
                " ◆ ",
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(toast.message, Style::default().fg(color)),
        ]);
        frame.render_widget(Paragraph::new(line), area);
        return;
    }

    let hints = match snapshot.input_mode {
        InputMode::Search => vec![("Type", "Filter"), ("Enter", "Apply"), ("Esc", "Clear")],
        InputMode::FolderModal => vec![
            ("Enter", "Add Path"),
            ("Tab", "Complete"),
            ("↑/↓", "Select"),
            ("Del", "Remove"),
            ("Esc", "Close"),
        ],
        InputMode::Normal => vec![
            ("Space", "Play"),
            ("←/→", "Seek"),
            ("n/p", "Track"),
            ("+/-", "Vol"),
            ("o", "Folders"),
            ("/", "Search"),
            ("s", "Shuffle"),
            ("r", "Repeat"),
            ("t", "Theme"),
            ("v", "Vis"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
    };

    let mut spans = Vec::with_capacity(hints.len() * 3);
    for (idx, (key, action)) in hints.into_iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled("  ", Style::default()));
        }
        spans.push(Span::styled(
            key,
            Style::default()
                .fg(Theme::color(theme.accent_primary))
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {action}"),
            Style::default().fg(Theme::color(theme.muted)),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn render_folder_modal(frame: &mut Frame, area: Rect, snapshot: &UiSnapshot<'_>, theme: &Theme) {
    let modal_w = area.width.min(72);
    let modal_h = area.height.min(18);
    let modal_area = Rect {
        x: area.x + area.width.saturating_sub(modal_w) / 2,
        y: area.y + area.height.saturating_sub(modal_h) / 2,
        width: modal_w,
        height: modal_h,
    };

    frame.render_widget(Clear, modal_area);

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.accent_primary)))
        .style(Style::default().bg(Theme::color(theme.surface)))
        .padding(Padding::horizontal(1))
        .title(Span::styled(
            " ▤ Music Folders ",
            Style::default()
                .fg(Theme::color(theme.accent_primary))
                .add_modifier(Modifier::BOLD),
        ));

    let inner = outer.inner(modal_area);
    frame.render_widget(outer, modal_area);

    if inner.height < 6 || inner.width < 20 {
        return;
    }

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(inner);

    let cursor = if ((snapshot.tick / 12) & 1) == 0 {
        "▏"
    } else {
        ""
    };

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.accent_secondary)))
        .style(Style::default().bg(Theme::color(theme.bg)))
        .title(Span::styled(
            " Add Folder or FLAC Path (Paste / Type + Enter) ",
            Style::default().fg(Theme::color(theme.muted)),
        ));

    let input_text = if snapshot.folder_input.is_empty() {
        Line::from(vec![
            Span::styled(
                cursor,
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "e.g. D:\\Music\\FLAC  (Tab to autocomplete)",
                Style::default().fg(Theme::color(theme.muted)),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                snapshot.folder_input,
                Style::default().fg(Theme::color(theme.text)),
            ),
            Span::styled(
                cursor,
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    };

    frame.render_widget(Paragraph::new(input_text).block(input_block), sections[0]);

    let (status_msg, status_color) = match snapshot.folder_preview {
        FolderPreview::Empty => (
            "Saved library folders (persisted in config.toml):",
            Theme::color(theme.muted),
        ),
        FolderPreview::ValidDir => (
            "✓ Directory found — press Enter to add & scan",
            Theme::color(theme.success),
        ),
        FolderPreview::ValidFile => (
            "✓ FLAC file found — press Enter to add",
            Theme::color(theme.success),
        ),
        FolderPreview::NotFound => (
            "✕ Path not found — check path or press Tab to autocomplete",
            Theme::color(theme.accent_tertiary),
        ),
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            status_msg,
            Style::default().fg(status_color),
        ))),
        sections[1],
    );

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.border)))
        .style(Style::default().bg(Theme::color(theme.bg)));

    if snapshot.folders.is_empty() {
        frame.render_widget(
            Paragraph::new("No folders added yet.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Theme::color(theme.muted)))
                .block(list_block),
            sections[2],
        );
    } else {
        let max_len = usize::from(sections[2].width.saturating_sub(6));
        let items: Vec<ListItem> = snapshot
            .folders
            .iter()
            .map(|path| {
                let text = truncate_str(&path.display().to_string(), max_len);
                let exists = path.exists();
                let icon_color = if exists {
                    Theme::color(theme.success)
                } else {
                    Theme::color(theme.accent_tertiary)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(" ● ", Style::default().fg(icon_color)),
                    Span::styled(text, Style::default().fg(Theme::color(theme.text))),
                ]))
            })
            .collect();

        let mut modal_state = ListState::default();
        modal_state.select(snapshot.folder_selected);
        let list = List::new(items)
            .block(list_block)
            .highlight_style(
                Style::default()
                    .bg(Theme::color(theme.selection_bg))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▎");
        frame.render_stateful_widget(list, sections[2], &mut modal_state);
    }

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Add  ", Style::default().fg(Theme::color(theme.muted))),
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " Complete  ",
                Style::default().fg(Theme::color(theme.muted)),
            ),
            Span::styled(
                "↑/↓",
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Select  ", Style::default().fg(Theme::color(theme.muted))),
            Span::styled(
                "Del",
                Style::default()
                    .fg(Theme::color(theme.accent_tertiary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Remove  ", Style::default().fg(Theme::color(theme.muted))),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(Theme::color(theme.accent_primary))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" Close", Style::default().fg(Theme::color(theme.muted))),
        ]))
        .alignment(Alignment::Center),
        sections[3],
    );
}

fn render_help_modal(frame: &mut Frame, area: Rect, theme: &Theme) {
    let modal_w = area.width.min(62);
    let modal_h = area.height.min(21);
    let modal_area = Rect {
        x: area.x + area.width.saturating_sub(modal_w) / 2,
        y: area.y + area.height.saturating_sub(modal_h) / 2,
        width: modal_w,
        height: modal_h,
    };

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Theme::color(theme.accent_primary)))
        .style(Style::default().bg(Theme::color(theme.surface)))
        .padding(Padding::uniform(1))
        .title(Span::styled(
            " SONORA Keybindings ",
            Style::default()
                .fg(Theme::color(theme.accent_primary))
                .add_modifier(Modifier::BOLD),
        ));

    let rows = [
        ("↑ / k, ↓ / j", "Move selection up / down"),
        ("g / G", "Jump to top / bottom of library"),
        ("Enter", "Play selected FLAC track"),
        ("Space", "Play / Pause"),
        ("← / →", "Seek backward / forward 5 seconds"),
        ("n / p", "Next / Previous track"),
        ("+ / -  (or = / _)", "Increase / Decrease volume"),
        ("m", "Toggle mute"),
        ("s", "Toggle shuffle"),
        ("r", "Cycle repeat (Off / All / One)"),
        ("o", "Open / Add / Remove music folders"),
        ("R", "Rescan all music folders"),
        ("/", "Live filter library by title, artist, album"),
        (
            "t",
            "Cycle monochrome theme (Obsidian / Graphite / Noir / Paper)",
        ),
        ("v", "Cycle visualizer mode (Spectrum / Mirror / Wave)"),
        ("D", "Toggle Discord Rich Presence"),
        ("? / Esc / q", "Toggle help / Quit"),
    ];

    let lines: Vec<Line> = rows
        .into_iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:<20}"),
                    Style::default()
                        .fg(Theme::color(theme.accent_secondary))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(desc, Style::default().fg(Theme::color(theme.text))),
            ])
        })
        .collect();

    frame.render_widget(Paragraph::new(lines).block(block), modal_area);
}

fn truncate_str(input: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let count = input.chars().count();
    if count <= max_chars {
        return input.to_string();
    }
    if max_chars == 1 {
        return String::from("…");
    }
    let mut out: String = input.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}
