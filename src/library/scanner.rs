use std::borrow::Cow;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use walkdir::WalkDir;

use crate::error::LibraryError;
use crate::library::track::Track;

#[derive(Debug)]
pub enum ScanEvent {
    Track(Box<Track>),
    Finished { total: usize },
    Error(String),
}

pub fn spawn_scan(roots: Vec<PathBuf>) -> Receiver<ScanEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut total = 0usize;
        let mut seen = HashSet::new();

        for root in roots {
            if !root.exists() {
                let _ = tx.send(ScanEvent::Error(
                    LibraryError::MissingDirectory(root).to_string(),
                ));
                continue;
            }

            if root.is_file() {
                if is_flac(&root) && mark_unique(&mut seen, &root) {
                    if let Ok(track) = read_track(&root) {
                        total += 1;
                        if tx.send(ScanEvent::Track(Box::new(track))).is_err() {
                            return;
                        }
                    }
                }
                continue;
            }

            for entry in WalkDir::new(&root)
                .follow_links(true)
                .into_iter()
                .filter_map(Result::ok)
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                let path = entry.path();
                if !is_flac(path) || !mark_unique(&mut seen, path) {
                    continue;
                }
                if let Ok(track) = read_track(path) {
                    total += 1;
                    if tx.send(ScanEvent::Track(Box::new(track))).is_err() {
                        return;
                    }
                }
            }
        }

        let _ = tx.send(ScanEvent::Finished { total });
    });
    rx
}

fn mark_unique(seen: &mut HashSet<PathBuf>, path: &Path) -> bool {
    let key = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    seen.insert(key)
}

fn is_flac(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("flac"))
}

fn read_track(path: &Path) -> Result<Track, LibraryError> {
    let tagged = lofty::read_from_path(path).map_err(|source| LibraryError::Metadata {
        path: path.to_path_buf(),
        source,
    })?;

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let properties = tagged.properties();

    let title = tag
        .and_then(|t| t.title().map(Cow::into_owned))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback_title(path));

    let artist = tag
        .and_then(|t| t.artist().map(Cow::into_owned))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("Unknown Artist"));

    let album = tag
        .and_then(|t| t.album().map(Cow::into_owned))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| String::from("Unknown Album"));

    let track_number = tag.and_then(Accessor::track);

    let year = tag.and_then(|t| {
        t.get_strings(ItemKey::Year)
            .next()
            .or_else(|| t.get_strings(ItemKey::RecordingDate).next())
            .and_then(extract_year)
    });

    Ok(Track {
        path: path.to_path_buf(),
        title,
        artist,
        album,
        track_number,
        year,
        duration: properties.duration(),
        sample_rate: properties.sample_rate(),
        bit_depth: properties.bit_depth(),
        channels: properties.channels(),
        bitrate: properties
            .audio_bitrate()
            .or_else(|| properties.overall_bitrate()),
    })
}

fn fallback_title(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(String::from)
        .unwrap_or_else(|| String::from("Untitled"))
}

fn extract_year(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let candidate: String = trimmed.chars().take(4).collect();
    if candidate.len() == 4 && candidate.chars().all(|ch| ch.is_ascii_digit()) {
        Some(candidate)
    } else {
        None
    }
}
