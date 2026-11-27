use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;

#[derive(Debug, Clone)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: Option<u32>,
    pub year: Option<String>,
    pub duration: Duration,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub bitrate: Option<u32>,
}

impl Track {
    pub fn quality_label(&self) -> String {
        match (self.bit_depth, self.sample_rate) {
            (Some(bits), Some(hz)) => {
                let khz = f64::from(hz) / 1000.0;
                if (khz.fract()).abs() < f64::EPSILON {
                    format!("{bits}-bit / {khz:.0} kHz")
                } else {
                    format!("{bits}-bit / {khz:.1} kHz")
                }
            }
            (Some(bits), None) => format!("{bits}-bit FLAC"),
            (None, Some(hz)) => format!("{:.1} kHz FLAC", f64::from(hz) / 1000.0),
            (None, None) => String::from("FLAC Lossless"),
        }
    }

    pub fn bitrate_label(&self) -> String {
        match self.bitrate {
            Some(kbps) if kbps > 0 => format!("{kbps} kbps"),
            _ => String::from("Lossless"),
        }
    }

    pub fn channel_label(&self) -> &'static str {
        match self.channels {
            Some(1) => "MONO",
            Some(2) => "STEREO",
            Some(6) => "5.1",
            Some(8) => "7.1",
            Some(_) => "MULTI",
            None => "FLAC",
        }
    }

    pub fn matches_query(&self, query: &str) -> bool {
        let needle = query.trim();
        if needle.is_empty() {
            return true;
        }
        let lower = needle.to_lowercase();
        self.title.to_lowercase().contains(&lower)
            || self.artist.to_lowercase().contains(&lower)
            || self.album.to_lowercase().contains(&lower)
    }

    pub fn load_cover_bytes(path: &Path) -> Option<Vec<u8>> {
        let tagged = lofty::read_from_path(path).ok()?;
        let preferred = tagged
            .primary_tag()
            .into_iter()
            .chain(tagged.tags())
            .find_map(|tag| {
                tag.pictures()
                    .iter()
                    .find(|pic| pic.pic_type() == PictureType::CoverFront)
            });

        if let Some(picture) = preferred {
            return Some(picture.data().to_vec());
        }

        tagged
            .primary_tag()
            .into_iter()
            .chain(tagged.tags())
            .find_map(|tag| tag.pictures().first())
            .map(|picture| picture.data().to_vec())
    }
}

pub fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}
