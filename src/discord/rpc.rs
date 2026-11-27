use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::discord::artwork::ArtworkCache;
use crate::library::track::Track;

const FALLBACK_ASSET_KEY: &str = "sonora";
const RECONNECT_COOLDOWN: Duration = Duration::from_secs(10);
const MAX_FIELD_CHARS: usize = 128;

pub struct RpcTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub quality_label: String,
    pub duration: Duration,
    pub position: Duration,
}

impl RpcTrack {
    pub fn from_track(track: &Track, position: Duration) -> Self {
        Self {
            title: track.title.clone(),
            artist: track.artist.clone(),
            album: track.album.clone(),
            quality_label: track.quality_label(),
            duration: track.duration,
            position,
        }
    }
}

enum RpcCommand {
    Playing(Box<RpcTrack>),
    Clear,
    SetEnabled(bool),
    Shutdown,
}

pub struct DiscordRpc {
    tx: Sender<RpcCommand>,
    worker: Option<JoinHandle<()>>,
}

impl DiscordRpc {
    pub fn new(client_id: String, enabled: bool, cover_lookup: bool) -> Self {
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_worker(rx, client_id, enabled, cover_lookup);
        });
        Self {
            tx,
            worker: Some(worker),
        }
    }

    pub fn update_playing(&self, track: &Track, position: Duration) {
        let payload = Box::new(RpcTrack::from_track(track, position));
        let _ = self.tx.send(RpcCommand::Playing(payload));
    }

    pub fn clear(&self) {
        let _ = self.tx.send(RpcCommand::Clear);
    }

    pub fn set_enabled(&self, enabled: bool) {
        let _ = self.tx.send(RpcCommand::SetEnabled(enabled));
    }
}

impl Drop for DiscordRpc {
    fn drop(&mut self) {
        let _ = self.tx.send(RpcCommand::Shutdown);
        if let Some(handle) = self.worker.take() {
            let _ = handle.join();
        }
    }
}

struct WorkerState {
    client: DiscordIpcClient,
    connected: bool,
    enabled: bool,
    cover_lookup: bool,
    last_connect_attempt: Option<Instant>,
    artwork: ArtworkCache,
}

impl WorkerState {
    fn new(client_id: String, enabled: bool, cover_lookup: bool) -> Self {
        Self {
            client: DiscordIpcClient::new(client_id),
            connected: false,
            enabled,
            cover_lookup,
            last_connect_attempt: None,
            artwork: ArtworkCache::new(),
        }
    }

    fn ensure_connected(&mut self) -> bool {
        if self.connected {
            return true;
        }
        if let Some(prev) = self.last_connect_attempt {
            if prev.elapsed() < RECONNECT_COOLDOWN {
                return false;
            }
        }
        self.last_connect_attempt = Some(Instant::now());
        if self.client.connect().is_ok() {
            self.connected = true;
            return true;
        }
        false
    }

    fn apply_playing(&mut self, track: &RpcTrack, rx: &Receiver<RpcCommand>) -> Option<RpcCommand> {
        if !self.enabled || !self.ensure_connected() {
            return None;
        }

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs() as i64)
            .unwrap_or(0);
        let start_secs = now_secs.saturating_sub(track.position.as_secs() as i64);

        let cached_url = if self.cover_lookup {
            self.artwork
                .cached(&track.artist, &track.album, &track.title)
                .cloned()
        } else {
            Some(None)
        };

        let initial_image = cached_url
            .as_ref()
            .and_then(|entry| entry.as_deref())
            .unwrap_or(FALLBACK_ASSET_KEY);

        self.send_activity(track, start_secs, initial_image);

        if self.cover_lookup && cached_url.is_none() {
            let fetched = self
                .artwork
                .resolve(&track.artist, &track.album, &track.title);
            if let Ok(pending) = rx.try_recv() {
                return Some(pending);
            }
            if let Some(fetched_url) = fetched {
                if self.enabled {
                    self.send_activity(track, start_secs, &fetched_url);
                }
            }
        }

        None
    }

    fn send_activity(&mut self, track: &RpcTrack, start_secs: i64, large_image: &str) {
        let details = sanitize_field(&track.title, "Lossless Track");
        let state = sanitize_field(&track.artist, "Unknown Artist");
        let tooltip = build_tooltip(&track.album, &track.quality_label);
        let dur_secs = track.duration.as_secs() as i64;

        let activity = build_activity(
            &details,
            &state,
            &tooltip,
            large_image,
            start_secs,
            dur_secs,
        );

        if self.client.set_activity(activity).is_err() {
            self.connected = false;
            if self.client.reconnect().is_ok() {
                self.connected = true;
                let retry = build_activity(
                    &details,
                    &state,
                    &tooltip,
                    large_image,
                    start_secs,
                    dur_secs,
                );
                if self.client.set_activity(retry).is_err() {
                    self.connected = false;
                }
            }
        }
    }

    fn clear(&mut self) {
        if !self.connected {
            return;
        }
        if self.client.clear_activity().is_err() {
            self.connected = false;
        }
    }

    fn shutdown(&mut self) {
        if self.connected {
            let _ = self.client.clear_activity();
            let _ = self.client.close();
            self.connected = false;
        }
    }
}

fn build_activity<'a>(
    details: &'a str,
    state: &'a str,
    tooltip: &'a str,
    large_image: &'a str,
    start_secs: i64,
    dur_secs: i64,
) -> Activity<'a> {
    let mut activity = Activity::new()
        .activity_type(ActivityType::Listening)
        .details(details)
        .state(state)
        .assets(Assets::new().large_image(large_image).large_text(tooltip));

    if dur_secs > 0 {
        let end_secs = start_secs.saturating_add(dur_secs);
        activity = activity.timestamps(Timestamps::new().start(start_secs).end(end_secs));
    } else if start_secs > 0 {
        activity = activity.timestamps(Timestamps::new().start(start_secs));
    }

    activity
}

fn run_worker(rx: Receiver<RpcCommand>, client_id: String, enabled: bool, cover_lookup: bool) {
    let mut state = WorkerState::new(client_id, enabled, cover_lookup);
    if state.enabled {
        state.ensure_connected();
    }

    let mut queued: Option<RpcCommand> = None;

    loop {
        let first = match queued.take() {
            Some(cmd) => cmd,
            None => match rx.recv() {
                Ok(cmd) => cmd,
                Err(_) => break,
            },
        };

        let mut latest = first;
        if !matches!(latest, RpcCommand::Shutdown) {
            while let Ok(newer) = rx.try_recv() {
                latest = newer;
                if matches!(latest, RpcCommand::Shutdown) {
                    break;
                }
            }
        }

        match latest {
            RpcCommand::Playing(track) => {
                queued = state.apply_playing(&track, &rx);
            }
            RpcCommand::Clear => state.clear(),
            RpcCommand::SetEnabled(next) => {
                state.enabled = next;
                if !next {
                    state.clear();
                } else {
                    state.last_connect_attempt = None;
                    state.ensure_connected();
                }
            }
            RpcCommand::Shutdown => {
                state.shutdown();
                return;
            }
        }
    }

    state.shutdown();
}

fn build_tooltip(album: &str, quality_label: &str) -> String {
    let clean_album = album.trim();
    let raw = if clean_album.is_empty() || clean_album.eq_ignore_ascii_case("unknown album") {
        format!("Sonora • {quality_label}")
    } else {
        format!("{clean_album} • {quality_label}")
    };
    sanitize_field(&raw, "Sonora FLAC")
}

fn sanitize_field(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    let base = if trimmed.is_empty() {
        fallback
    } else {
        trimmed
    };
    let count = base.chars().count();
    if count < 2 {
        return format!("{base} ");
    }
    if count <= MAX_FIELD_CHARS {
        return base.to_string();
    }
    let mut out: String = base.chars().take(MAX_FIELD_CHARS - 1).collect();
    out.push('…');
    out
}
