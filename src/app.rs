use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use rand::Rng;
use ratatui::widgets::ListState;
use ratatui::DefaultTerminal;

use crate::audio::engine::AudioEngine;
use crate::audio::spectrum::SpectrumAnalyzer;
use crate::config::{expand_user_path, AppConfig, RepeatMode};
use crate::discord::rpc::DiscordRpc;
use crate::library::scanner::{spawn_scan, ScanEvent};
use crate::library::track::{format_duration, Track};
use crate::ui::cover::CoverArt;
use crate::ui::layout::{self, FolderPreview, InputMode, ToastView, UiSnapshot};
use crate::ui::visualizer;

const FRAME_DURATION: Duration = Duration::from_millis(33);
const TOAST_TTL: Duration = Duration::from_secs(3);
const SEEK_STEP_SECS: i64 = 5;
const VOLUME_STEP: f32 = 0.05;

struct Toast {
    message: String,
    is_error: bool,
    expires_at: Instant,
}

pub struct App {
    audio: AudioEngine,
    analyzer: SpectrumAnalyzer,
    cover: CoverArt,
    discord: DiscordRpc,
    config: AppConfig,
    folders: Vec<PathBuf>,
    root_label: String,
    auto_play_single_file: bool,
    tracks: Vec<Track>,
    visible_indices: Vec<usize>,
    list_state: ListState,
    playing_index: Option<usize>,
    active_track_path: Option<PathBuf>,
    history: Vec<usize>,
    scan_rx: Option<Receiver<ScanEvent>>,
    scanning: bool,
    input_mode: InputMode,
    search_query: String,
    folder_input: String,
    folder_selected: Option<usize>,
    show_help: bool,
    should_quit: bool,
    toast: Option<Toast>,
    tick: u64,
    vis_width: u16,
}

impl App {
    pub fn new(cli_path: Option<PathBuf>) -> Self {
        let mut config = AppConfig::load();
        let auto_play_single_file = cli_path.as_ref().is_some_and(|p| p.is_file());
        let folders = config.resolve_music_dirs(cli_path);
        let root_label = format_roots_label(&folders);
        let mut audio = AudioEngine::new(config.volume);
        let toast = audio.take_init_error().map(|message| Toast {
            message,
            is_error: true,
            expires_at: Instant::now() + Duration::from_secs(6),
        });
        let scan_rx = spawn_scan(folders.clone());
        let discord = DiscordRpc::new(
            config.effective_discord_client_id().to_string(),
            config.discord_rpc,
            config.discord_cover_lookup,
        );

        Self {
            audio,
            analyzer: SpectrumAnalyzer::new(),
            cover: CoverArt::new(),
            discord,
            config,
            folders,
            root_label,
            auto_play_single_file,
            tracks: Vec::new(),
            visible_indices: Vec::new(),
            list_state: ListState::default(),
            playing_index: None,
            active_track_path: None,
            history: Vec::new(),
            scan_rx: Some(scan_rx),
            scanning: true,
            input_mode: InputMode::Normal,
            search_query: String::new(),
            folder_input: String::new(),
            folder_selected: None,
            show_help: false,
            should_quit: false,
            toast,
            tick: 0,
            vis_width: 48,
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.should_quit {
            let frame_start = Instant::now();
            self.tick = self.tick.wrapping_add(1);

            self.drain_scan_events();
            self.check_track_completion();
            self.expire_toast();

            let bands = visualizer::band_count_for_width(self.vis_width);
            self.analyzer
                .update(self.audio.sample_ring(), bands, self.audio.is_playing());

            let toast = self.toast.as_ref().map(|t| ToastView {
                message: &t.message,
                is_error: t.is_error,
            });
            let folder_preview = if self.input_mode == InputMode::FolderModal {
                classify_folder_input(&self.folder_input)
            } else {
                FolderPreview::Empty
            };
            let snapshot = UiSnapshot {
                theme_kind: self.config.theme,
                tracks: &self.tracks,
                visible_indices: &self.visible_indices,
                playing_index: self.playing_index,
                is_playing: self.audio.is_playing(),
                is_paused: self.audio.is_paused(),
                scanning: self.scanning,
                position: self.audio.position(),
                duration: self.audio.duration(),
                volume: self.audio.volume(),
                muted: self.audio.is_muted(),
                shuffle: self.config.shuffle,
                repeat: self.config.repeat,
                visualizer_mode: self.config.visualizer,
                discord_rpc: self.config.discord_rpc,
                input_mode: self.input_mode,
                search_query: &self.search_query,
                folders: &self.folders,
                folder_input: &self.folder_input,
                folder_selected: self.folder_selected,
                folder_preview,
                show_help: self.show_help,
                toast,
                tick: self.tick,
                root_label: &self.root_label,
            };
            let mut next_vis_width = self.vis_width;
            let list_state = &mut self.list_state;
            let cover = &mut self.cover;
            let analyzer = &self.analyzer;
            terminal.draw(|frame| {
                next_vis_width = layout::render(frame, &snapshot, list_state, cover, analyzer);
            })?;
            self.vis_width = next_vis_width;

            let timeout = FRAME_DURATION
                .saturating_sub(frame_start.elapsed())
                .max(Duration::from_millis(1));
            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key);
                    }
                }
            }
        }

        self.discord.clear();
        self.config.volume = self.audio.volume();
        self.config.music_dirs = self.folders.clone();
        let _ = self.config.save();
        Ok(())
    }

    fn drain_scan_events(&mut self) {
        let Some(rx) = self.scan_rx.take() else {
            return;
        };

        let mut finished = false;
        let mut received_any = false;

        for _ in 0..64 {
            match rx.try_recv() {
                Ok(ScanEvent::Track(track)) => {
                    let idx = self.tracks.len();
                    let matches = track.matches_query(&self.search_query);
                    self.tracks.push(*track);
                    if matches {
                        self.visible_indices.push(idx);
                        if self.list_state.selected().is_none() {
                            self.list_state.select(Some(0));
                        }
                    }
                    received_any = true;
                }
                Ok(ScanEvent::Finished { total }) => {
                    finished = true;
                    self.scanning = false;
                    self.sort_library();
                    if total > 0 {
                        self.show_toast(format!("Loaded {total} FLAC tracks"), false);
                        if self.auto_play_single_file && self.playing_index.is_none() {
                            self.auto_play_single_file = false;
                            self.play_track_at(0, false);
                        }
                    } else if self.toast.is_none() {
                        self.show_toast(
                            String::from("No FLAC files found — press 'o' to add a folder"),
                            true,
                        );
                    }
                    break;
                }
                Ok(ScanEvent::Error(message)) => {
                    self.show_toast(message, true);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    finished = true;
                    self.scanning = false;
                    break;
                }
            }
        }

        if received_any && self.list_state.selected().is_none() && !self.visible_indices.is_empty()
        {
            self.list_state.select(Some(0));
        }

        if !finished {
            self.scan_rx = Some(rx);
        }
    }

    fn sort_library(&mut self) {
        let active_path = self
            .playing_index
            .and_then(|idx| self.tracks.get(idx))
            .map(|track| track.path.clone())
            .or_else(|| self.active_track_path.clone());

        self.tracks.sort_by(|a, b| {
            a.artist
                .to_lowercase()
                .cmp(&b.artist.to_lowercase())
                .then_with(|| a.album.to_lowercase().cmp(&b.album.to_lowercase()))
                .then_with(|| {
                    a.track_number
                        .unwrap_or(u32::MAX)
                        .cmp(&b.track_number.unwrap_or(u32::MAX))
                })
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });

        self.playing_index =
            active_path.and_then(|path| self.tracks.iter().position(|track| track.path == path));
        self.rebuild_visible_indices();
    }

    fn rebuild_visible_indices(&mut self) {
        let previous_track = self
            .list_state
            .selected()
            .and_then(|pos| self.visible_indices.get(pos).copied());

        self.visible_indices = self
            .tracks
            .iter()
            .enumerate()
            .filter_map(|(idx, track)| track.matches_query(&self.search_query).then_some(idx))
            .collect();

        if self.visible_indices.is_empty() {
            self.list_state.select(None);
            return;
        }

        let restored = previous_track
            .and_then(|idx| self.visible_indices.iter().position(|&v| v == idx))
            .unwrap_or(0);
        self.list_state.select(Some(restored));
    }

    fn rescan_library(&mut self) {
        self.root_label = format_roots_label(&self.folders);
        self.tracks.clear();
        self.visible_indices.clear();
        self.list_state.select(None);
        self.playing_index = None;
        self.history.clear();
        self.scanning = true;
        self.scan_rx = Some(spawn_scan(self.folders.clone()));
    }

    fn check_track_completion(&mut self) {
        if !self.audio.is_finished() {
            return;
        }
        let Some(current) = self.playing_index else {
            return;
        };

        match self.config.repeat {
            RepeatMode::One => self.play_track_at(current, false),
            RepeatMode::All => self.play_next(true),
            RepeatMode::Off => self.play_next(false),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }

        if self.input_mode == InputMode::FolderModal {
            self.handle_folder_key(key);
            return;
        }

        if self.show_help {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter
            ) {
                self.show_help = false;
            }
            return;
        }

        if self.input_mode == InputMode::Search {
            self.handle_search_key(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Esc => {
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.rebuild_visible_indices();
                }
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('o') => {
                self.folder_input.clear();
                self.folder_selected = if self.folders.is_empty() {
                    None
                } else {
                    Some(0)
                };
                self.input_mode = InputMode::FolderModal;
            }
            KeyCode::Char('R') => {
                self.rescan_library();
                self.show_toast(String::from("Rescanning library folders..."), false);
            }
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search;
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::Char('g') => {
                if !self.visible_indices.is_empty() {
                    self.list_state.select(Some(0));
                }
            }
            KeyCode::Char('G') => {
                if !self.visible_indices.is_empty() {
                    self.list_state.select(Some(self.visible_indices.len() - 1));
                }
            }
            KeyCode::Enter => self.play_selected(),
            KeyCode::Char(' ') => {
                if self.playing_index.is_some()
                    && (self.audio.is_playing() || self.audio.is_paused())
                {
                    self.audio.toggle_pause();
                    self.sync_discord_state();
                } else {
                    self.play_selected();
                }
            }
            KeyCode::Right | KeyCode::Char('l') => self.seek_relative(SEEK_STEP_SECS),
            KeyCode::Left | KeyCode::Char('h') => self.seek_relative(-SEEK_STEP_SECS),
            KeyCode::Char('n') => self.play_next(true),
            KeyCode::Char('p') => self.play_previous(),
            KeyCode::Char('+' | '=') => {
                let vol = self.audio.adjust_volume(VOLUME_STEP);
                self.show_toast(format!("Volume {}%", (vol * 100.0).round() as u16), false);
            }
            KeyCode::Char('-' | '_') => {
                let vol = self.audio.adjust_volume(-VOLUME_STEP);
                self.show_toast(format!("Volume {}%", (vol * 100.0).round() as u16), false);
            }
            KeyCode::Char('m') => {
                let muted = self.audio.toggle_mute();
                let label = if muted {
                    "Audio muted"
                } else {
                    "Audio unmuted"
                };
                self.show_toast(String::from(label), false);
            }
            KeyCode::Char('s') => {
                self.config.shuffle = !self.config.shuffle;
                let state = if self.config.shuffle { "ON" } else { "OFF" };
                self.show_toast(format!("Shuffle {state}"), false);
            }
            KeyCode::Char('r') => {
                self.config.repeat = self.config.repeat.next();
                self.show_toast(format!("Repeat: {}", self.config.repeat.label()), false);
            }
            KeyCode::Char('t') => {
                self.config.theme = self.config.theme.next();
                self.show_toast(format!("Theme: {}", self.config.theme.name()), false);
            }
            KeyCode::Char('v') => {
                self.config.visualizer = self.config.visualizer.next();
                self.show_toast(
                    format!("Visualizer: {}", self.config.visualizer.label()),
                    false,
                );
            }
            KeyCode::Char('D') => {
                self.config.discord_rpc = !self.config.discord_rpc;
                self.discord.set_enabled(self.config.discord_rpc);
                if self.config.discord_rpc {
                    self.sync_discord_state();
                }
                let _ = self.config.save();
                let state = if self.config.discord_rpc { "ON" } else { "OFF" };
                self.show_toast(format!("Discord RPC: {state}"), false);
            }
            _ => {}
        }
    }

    fn handle_folder_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('d') {
            self.remove_selected_folder();
            return;
        }

        match key.code {
            KeyCode::Esc => {
                self.folder_input.clear();
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => self.submit_folder_input(),
            KeyCode::Tab => {
                if let Some(completed) = complete_directory_path(&self.folder_input) {
                    self.folder_input = completed;
                }
            }
            KeyCode::Backspace => {
                self.folder_input.pop();
            }
            KeyCode::Delete => self.remove_selected_folder(),
            KeyCode::Down => self.move_folder_selection(1),
            KeyCode::Up => self.move_folder_selection(-1),
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.folder_input.push(ch);
            }
            _ => {}
        }
    }

    fn move_folder_selection(&mut self, delta: isize) {
        let len = self.folders.len();
        if len == 0 {
            self.folder_selected = None;
            return;
        }
        let current = self.folder_selected.unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.folder_selected = Some(next);
    }

    fn submit_folder_input(&mut self) {
        if self.folder_input.trim().is_empty() {
            self.input_mode = InputMode::Normal;
            return;
        }

        let path = expand_user_path(&self.folder_input);
        if !path.exists() {
            self.show_toast(format!("Path not found: {}", path.display()), true);
            return;
        }

        let is_valid_file = path.is_file()
            && path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("flac"));

        if !path.is_dir() && !is_valid_file {
            self.show_toast(String::from("Enter a folder path or a .flac file"), true);
            return;
        }

        if !self.folders.contains(&path) {
            self.folders.push(path.clone());
            self.config.music_dirs = self.folders.clone();
            let _ = self.config.save();
        }

        self.folder_input.clear();
        self.input_mode = InputMode::Normal;
        self.rescan_library();
        self.show_toast(format!("Added: {}", path.display()), false);
    }

    fn remove_selected_folder(&mut self) {
        let Some(idx) = self.folder_selected else {
            return;
        };
        if idx >= self.folders.len() {
            return;
        }
        let removed = self.folders.remove(idx);
        if self.folders.is_empty() {
            self.folder_selected = None;
        } else {
            self.folder_selected = Some(idx.min(self.folders.len() - 1));
        }
        self.config.music_dirs = self.folders.clone();
        let _ = self.config.save();
        self.rescan_library();
        self.show_toast(format!("Removed: {}", removed.display()), false);
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search_query.clear();
                self.rebuild_visible_indices();
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                self.rebuild_visible_indices();
            }
            KeyCode::Down => self.move_selection(1),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Char(ch) => {
                self.search_query.push(ch);
                self.rebuild_visible_indices();
            }
            _ => {}
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.visible_indices.len();
        if len == 0 {
            self.list_state.select(None);
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(len as isize) as usize;
        self.list_state.select(Some(next));
    }

    fn play_selected(&mut self) {
        let Some(pos) = self.list_state.selected() else {
            return;
        };
        let Some(&track_idx) = self.visible_indices.get(pos) else {
            return;
        };
        self.play_track_at(track_idx, true);
    }

    fn play_track_at(&mut self, track_idx: usize, record_history: bool) {
        let Some(track) = self.tracks.get(track_idx) else {
            return;
        };
        let path = track.path.clone();
        let duration = track.duration;
        let title = track.title.clone();

        if record_history {
            if let Some(prev) = self.playing_index {
                if prev != track_idx {
                    self.history.push(prev);
                    if self.history.len() > 64 {
                        self.history.remove(0);
                    }
                }
            }
        }

        match self.audio.play_file(&path, duration) {
            Ok(()) => {
                self.playing_index = Some(track_idx);
                self.active_track_path = Some(path.clone());
                self.cover.load_for_track(&path);
                if let Some(vis_pos) = self.visible_indices.iter().position(|&i| i == track_idx) {
                    self.list_state.select(Some(vis_pos));
                }
                if let Some(active) = self.tracks.get(track_idx) {
                    self.discord.update_playing(active, Duration::ZERO);
                }
                self.show_toast(format!("Playing: {title}"), false);
            }
            Err(err) => {
                self.show_toast(err.to_string(), true);
            }
        }
    }

    fn play_next(&mut self, wrap: bool) {
        if self.visible_indices.is_empty() {
            return;
        }

        if self.config.shuffle && self.visible_indices.len() > 1 {
            let mut rng = rand::rng();
            let mut candidate =
                self.visible_indices[rng.random_range(0..self.visible_indices.len())];
            if Some(candidate) == self.playing_index {
                let next_pos = (self
                    .visible_indices
                    .iter()
                    .position(|&i| i == candidate)
                    .unwrap_or(0)
                    + 1)
                    % self.visible_indices.len();
                candidate = self.visible_indices[next_pos];
            }
            self.play_track_at(candidate, true);
            return;
        }

        let current_pos = self
            .playing_index
            .and_then(|active| self.visible_indices.iter().position(|&i| i == active));

        let next_pos = match current_pos {
            Some(pos) if pos + 1 < self.visible_indices.len() => Some(pos + 1),
            Some(_) if wrap => Some(0),
            Some(_) => None,
            None => Some(0),
        };

        if let Some(pos) = next_pos {
            let track_idx = self.visible_indices[pos];
            self.play_track_at(track_idx, true);
        } else {
            self.audio.stop();
            self.cover.clear();
            self.discord.clear();
            self.playing_index = None;
            self.active_track_path = None;
        }
    }

    fn play_previous(&mut self) {
        if self.visible_indices.is_empty() {
            return;
        }

        if let Some(active) = self.playing_index {
            if self.audio.position() > Duration::from_secs(3) {
                self.play_track_at(active, false);
                return;
            }
        }

        if let Some(prev_idx) = self.history.pop() {
            if prev_idx < self.tracks.len() {
                self.play_track_at(prev_idx, false);
                return;
            }
        }

        let current_pos = self
            .playing_index
            .and_then(|active| self.visible_indices.iter().position(|&i| i == active))
            .unwrap_or(0);
        let prev_pos = if current_pos == 0 {
            self.visible_indices.len() - 1
        } else {
            current_pos - 1
        };
        let track_idx = self.visible_indices[prev_pos];
        self.play_track_at(track_idx, false);
    }

    fn seek_relative(&mut self, delta_secs: i64) {
        match self.audio.seek_relative(delta_secs) {
            Ok(pos) => {
                if let Some(active) = self.playing_index.and_then(|idx| self.tracks.get(idx)) {
                    if self.audio.is_playing() {
                        self.discord.update_playing(active, pos);
                    }
                    self.show_toast(format!("Seek {}", format_duration(pos)), false);
                }
            }
            Err(err) => {
                self.show_toast(err.to_string(), true);
            }
        }
    }

    fn sync_discord_state(&self) {
        if self.audio.is_playing() {
            if let Some(active) = self.playing_index.and_then(|idx| self.tracks.get(idx)) {
                self.discord.update_playing(active, self.audio.position());
            }
        } else {
            self.discord.clear();
        }
    }

    fn show_toast(&mut self, message: String, is_error: bool) {
        self.toast = Some(Toast {
            message,
            is_error,
            expires_at: Instant::now() + TOAST_TTL,
        });
    }

    fn expire_toast(&mut self) {
        if let Some(toast) = &self.toast {
            if Instant::now() >= toast.expires_at {
                self.toast = None;
            }
        }
    }
}

fn format_roots_label(roots: &[PathBuf]) -> String {
    match roots {
        [] => String::from("(none)"),
        [single] => single.display().to_string(),
        [first, rest @ ..] => format!("{} (+{} more)", first.display(), rest.len()),
    }
}

fn classify_folder_input(raw: &str) -> FolderPreview {
    if raw.trim().is_empty() {
        return FolderPreview::Empty;
    }
    let path = expand_user_path(raw);
    if path.is_dir() {
        FolderPreview::ValidDir
    } else if path.is_file()
        && path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("flac"))
    {
        FolderPreview::ValidFile
    } else {
        FolderPreview::NotFound
    }
}

fn complete_directory_path(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return None;
    }

    let expanded = expand_user_path(trimmed);
    let ends_with_sep = trimmed.ends_with('/') || trimmed.ends_with('\\');

    let (search_dir, prefix): (PathBuf, String) = if ends_with_sep && expanded.is_dir() {
        (expanded, String::new())
    } else {
        let parent = expanded.parent()?.to_path_buf();
        let file_name = expanded
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_lowercase();
        (parent, file_name)
    };

    let entries = fs::read_dir(&search_dir).ok()?;
    let mut matches: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.to_lowercase().starts_with(&prefix))
        })
        .collect();

    matches.sort();
    let best = matches.into_iter().next()?;
    let mut completed = best.display().to_string();
    if !completed.ends_with(std::path::MAIN_SEPARATOR) {
        completed.push(std::path::MAIN_SEPARATOR);
    }
    Some(completed)
}
