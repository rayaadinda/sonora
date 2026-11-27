use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::ConfigError;
use crate::ui::theme::ThemeKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    Off,
    #[default]
    All,
    One,
}

impl RepeatMode {
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::All,
            Self::All => Self::One,
            Self::One => Self::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::All => "All",
            Self::One => "One",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisualizerMode {
    #[default]
    Bars,
    Mirror,
    Wave,
}

impl VisualizerMode {
    pub fn next(self) -> Self {
        match self {
            Self::Bars => Self::Mirror,
            Self::Mirror => Self::Wave,
            Self::Wave => Self::Bars,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bars => "Spectrum",
            Self::Mirror => "Mirror",
            Self::Wave => "Wave",
        }
    }
}

pub const DEFAULT_DISCORD_CLIENT_ID: &str = "1575844168532754523";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub music_dir: Option<PathBuf>,
    pub music_dirs: Vec<PathBuf>,
    pub volume: f32,
    pub theme: ThemeKind,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub visualizer: VisualizerMode,
    pub discord_rpc: bool,
    pub discord_client_id: Option<String>,
    pub discord_cover_lookup: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            music_dir: None,
            music_dirs: Vec::new(),
            volume: 0.8,
            theme: ThemeKind::Obsidian,
            shuffle: false,
            repeat: RepeatMode::All,
            visualizer: VisualizerMode::Bars,
            discord_rpc: true,
            discord_client_id: None,
            discord_cover_lookup: true,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join("sonora").join("config.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };
        let mut config: Self = toml::from_str(&raw).unwrap_or_default();
        config.volume = config.volume.clamp(0.0, 1.5);
        if let Some(legacy) = config.music_dir.take() {
            if !config.music_dirs.contains(&legacy) {
                config.music_dirs.push(legacy);
            }
        }
        config
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let Some(path) = Self::config_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = toml::to_string_pretty(self)?;
        fs::write(path, serialized)?;
        Ok(())
    }

    pub fn resolve_music_dirs(&mut self, cli_path: Option<PathBuf>) -> Vec<PathBuf> {
        if let Some(path) = cli_path {
            if path.is_dir() && !self.music_dirs.contains(&path) {
                self.music_dirs.push(path.clone());
            }
            return vec![path];
        }

        self.music_dirs.retain(|path| path.exists());
        if !self.music_dirs.is_empty() {
            return self.music_dirs.clone();
        }

        let fallback = dirs::audio_dir()
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        self.music_dirs.push(fallback.clone());
        vec![fallback]
    }

    pub fn effective_discord_client_id(&self) -> &str {
        self.discord_client_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(DEFAULT_DISCORD_CLIENT_ID)
    }
}

pub fn expand_user_path(raw: &str) -> PathBuf {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();

    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }

    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }

    PathBuf::from(trimmed)
}
