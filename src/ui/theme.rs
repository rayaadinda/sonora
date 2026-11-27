use ratatui::style::Color;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeKind {
    #[default]
    #[serde(
        alias = "tokyo_night",
        alias = "catppuccin_mocha",
        alias = "rose_pine",
        alias = "gruvbox_dark"
    )]
    Obsidian,
    Graphite,
    Noir,
    Paper,
}

impl ThemeKind {
    pub fn next(self) -> Self {
        match self {
            Self::Obsidian => Self::Graphite,
            Self::Graphite => Self::Noir,
            Self::Noir => Self::Paper,
            Self::Paper => Self::Obsidian,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Obsidian => "Obsidian",
            Self::Graphite => "Graphite",
            Self::Noir => "Noir",
            Self::Paper => "Paper",
        }
    }

    pub fn palette(self) -> Theme {
        match self {
            Self::Obsidian => Theme {
                bg: (12, 12, 12),
                surface: (18, 18, 18),
                border: (54, 54, 54),
                border_focus: (184, 184, 184),
                text: (236, 236, 236),
                muted: (126, 126, 126),
                accent_primary: (248, 248, 248),
                accent_secondary: (196, 196, 196),
                accent_tertiary: (156, 156, 156),
                selection_bg: (38, 38, 38),
                bar_low: (76, 76, 76),
                bar_mid: (162, 162, 162),
                bar_high: (242, 242, 242),
                peak: (255, 255, 255),
                success: (218, 218, 218),
                warning: (176, 176, 176),
            },
            Self::Graphite => Theme {
                bg: (22, 22, 22),
                surface: (30, 30, 30),
                border: (72, 72, 72),
                border_focus: (200, 200, 200),
                text: (224, 224, 224),
                muted: (136, 136, 136),
                accent_primary: (240, 240, 240),
                accent_secondary: (186, 186, 186),
                accent_tertiary: (148, 148, 148),
                selection_bg: (50, 50, 50),
                bar_low: (88, 88, 88),
                bar_mid: (164, 164, 164),
                bar_high: (236, 236, 236),
                peak: (250, 250, 250),
                success: (212, 212, 212),
                warning: (172, 172, 172),
            },
            Self::Noir => Theme {
                bg: (0, 0, 0),
                surface: (8, 8, 8),
                border: (44, 44, 44),
                border_focus: (230, 230, 230),
                text: (246, 246, 246),
                muted: (116, 116, 116),
                accent_primary: (255, 255, 255),
                accent_secondary: (206, 206, 206),
                accent_tertiary: (158, 158, 158),
                selection_bg: (30, 30, 30),
                bar_low: (66, 66, 66),
                bar_mid: (160, 160, 160),
                bar_high: (255, 255, 255),
                peak: (255, 255, 255),
                success: (232, 232, 232),
                warning: (180, 180, 180),
            },
            Self::Paper => Theme {
                bg: (244, 244, 244),
                surface: (234, 234, 234),
                border: (182, 182, 182),
                border_focus: (48, 48, 48),
                text: (24, 24, 24),
                muted: (110, 110, 110),
                accent_primary: (12, 12, 12),
                accent_secondary: (64, 64, 64),
                accent_tertiary: (96, 96, 96),
                selection_bg: (208, 208, 208),
                bar_low: (166, 166, 166),
                bar_mid: (92, 92, 92),
                bar_high: (18, 18, 18),
                peak: (0, 0, 0),
                success: (38, 38, 38),
                warning: (84, 84, 84),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub bg: (u8, u8, u8),
    pub surface: (u8, u8, u8),
    pub border: (u8, u8, u8),
    pub border_focus: (u8, u8, u8),
    pub text: (u8, u8, u8),
    pub muted: (u8, u8, u8),
    pub accent_primary: (u8, u8, u8),
    pub accent_secondary: (u8, u8, u8),
    pub accent_tertiary: (u8, u8, u8),
    pub selection_bg: (u8, u8, u8),
    pub bar_low: (u8, u8, u8),
    pub bar_mid: (u8, u8, u8),
    pub bar_high: (u8, u8, u8),
    pub peak: (u8, u8, u8),
    pub success: (u8, u8, u8),
    pub warning: (u8, u8, u8),
}

impl Theme {
    pub fn color(rgb: (u8, u8, u8)) -> Color {
        Color::Rgb(rgb.0, rgb.1, rgb.2)
    }

    pub fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> Color {
        let clamped = t.clamp(0.0, 1.0);
        let r = (f32::from(a.0) + (f32::from(b.0) - f32::from(a.0)) * clamped).round() as u8;
        let g = (f32::from(a.1) + (f32::from(b.1) - f32::from(a.1)) * clamped).round() as u8;
        let bl = (f32::from(a.2) + (f32::from(b.2) - f32::from(a.2)) * clamped).round() as u8;
        Color::Rgb(r, g, bl)
    }

    pub fn gradient_three(a: (u8, u8, u8), b: (u8, u8, u8), c: (u8, u8, u8), t: f32) -> Color {
        let clamped = t.clamp(0.0, 1.0);
        if clamped < 0.5 {
            Self::lerp(a, b, clamped * 2.0)
        } else {
            Self::lerp(b, c, (clamped - 0.5) * 2.0)
        }
    }
}
