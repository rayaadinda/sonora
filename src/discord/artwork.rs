use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::Duration;

use serde::Deserialize;
use ureq::Agent;

const LOOKUP_TIMEOUT: Duration = Duration::from_secs(3);
const MAX_JSON_BYTES: u64 = 128 * 1024;

#[derive(Deserialize)]
struct ItunesResponse {
    results: Vec<ItunesResult>,
}

#[derive(Deserialize)]
struct ItunesResult {
    #[serde(rename = "artworkUrl100")]
    artwork_url_100: Option<String>,
}

pub struct ArtworkCache {
    agent: Agent,
    entries: HashMap<(String, String), Option<String>>,
}

impl Default for ArtworkCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ArtworkCache {
    pub fn new() -> Self {
        let config = Agent::config_builder()
            .timeout_global(Some(LOOKUP_TIMEOUT))
            .build();
        Self {
            agent: config.into(),
            entries: HashMap::new(),
        }
    }

    pub fn cached(&self, artist: &str, album: &str, title: &str) -> Option<&Option<String>> {
        let key = cache_key(artist, album, title);
        self.entries.get(&key)
    }

    pub fn resolve(&mut self, artist: &str, album: &str, title: &str) -> Option<String> {
        let key = cache_key(artist, album, title);
        if let Some(hit) = self.entries.get(&key) {
            return hit.clone();
        }

        let found = self.fetch_cover(artist, album, title);
        self.entries.insert(key, found.clone());
        found
    }

    fn fetch_cover(&self, artist: &str, album: &str, title: &str) -> Option<String> {
        let clean_artist = normalize_artist(artist);
        let clean_album = normalize_album(album);

        if !clean_album.is_empty() {
            let term = if clean_artist.is_empty() {
                clean_album.to_string()
            } else {
                format!("{clean_artist} {clean_album}")
            };
            if let Some(url) = self.query_itunes(&term, "album") {
                return Some(url);
            }
        }

        let clean_title = title.trim();
        if clean_title.is_empty() {
            return None;
        }
        let song_term = if clean_artist.is_empty() {
            clean_title.to_string()
        } else {
            format!("{clean_artist} {clean_title}")
        };
        self.query_itunes(&song_term, "song")
    }

    fn query_itunes(&self, term: &str, entity: &str) -> Option<String> {
        let encoded = percent_encode(term);
        let url = format!("https://itunes.apple.com/search?term={encoded}&entity={entity}&limit=1");
        let mut response = self
            .agent
            .get(&url)
            .header("User-Agent", "sonora-flac-player/1.0")
            .call()
            .ok()?;

        let parsed: ItunesResponse = response
            .body_mut()
            .with_config()
            .limit(MAX_JSON_BYTES)
            .read_json()
            .ok()?;

        parsed
            .results
            .into_iter()
            .find_map(|item| item.artwork_url_100)
            .map(|raw| raw.replace("100x100bb", "600x600bb"))
    }
}

fn cache_key(artist: &str, album: &str, title: &str) -> (String, String) {
    let secondary = if normalize_album(album).is_empty() {
        title.trim().to_lowercase()
    } else {
        album.trim().to_lowercase()
    };
    (artist.trim().to_lowercase(), secondary)
}

fn normalize_artist(artist: &str) -> &str {
    let trimmed = artist.trim();
    if trimmed.eq_ignore_ascii_case("unknown artist") {
        ""
    } else {
        trimmed
    }
}

fn normalize_album(album: &str) -> &str {
    let trimmed = album.trim();
    if trimmed.eq_ignore_ascii_case("unknown album") {
        ""
    } else {
        trimmed
    }
}

fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            other => {
                let _ = write!(out, "%{other:02X}");
            }
        }
    }
    out
}
