use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::audio::tap::{SampleRing, TapSource};
use crate::error::AudioError;

pub struct AudioEngine {
    sink: Option<MixerDeviceSink>,
    player: Option<Player>,
    ring: Arc<Mutex<SampleRing>>,
    volume: f32,
    muted: bool,
    has_loaded: bool,
    track_duration: Duration,
    last_init_error: Option<String>,
}

impl AudioEngine {
    pub fn new(initial_volume: f32) -> Self {
        let volume = initial_volume.clamp(0.0, 1.5);
        let mut engine = Self {
            sink: None,
            player: None,
            ring: Arc::new(Mutex::new(SampleRing::default())),
            volume,
            muted: false,
            has_loaded: false,
            track_duration: Duration::ZERO,
            last_init_error: None,
        };
        if let Err(err) = engine.ensure_device() {
            engine.last_init_error = Some(err.to_string());
        }
        engine
    }

    pub fn take_init_error(&mut self) -> Option<String> {
        self.last_init_error.take()
    }

    fn ensure_device(&mut self) -> Result<(), AudioError> {
        if self.sink.is_some() && self.player.is_some() {
            return Ok(());
        }
        let mut sink = DeviceSinkBuilder::open_default_sink()?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());
        let effective = if self.muted { 0.0 } else { self.volume };
        player.set_volume(effective);
        self.sink = Some(sink);
        self.player = Some(player);
        self.last_init_error = None;
        Ok(())
    }

    pub fn play_file(
        &mut self,
        path: &Path,
        fallback_duration: Duration,
    ) -> Result<(), AudioError> {
        self.ensure_device()?;

        let file = File::open(path).map_err(|source| AudioError::FileOpen {
            path: path.to_path_buf(),
            source,
        })?;
        let byte_len = file
            .metadata()
            .map_err(|source| AudioError::FileOpen {
                path: path.to_path_buf(),
                source,
            })?
            .len();

        let reader = BufReader::new(file);
        let decoder = Decoder::builder()
            .with_data(reader)
            .with_byte_len(byte_len)
            .with_seekable(true)
            .with_hint("flac")
            .build()?;

        let duration = decoder.total_duration().unwrap_or(fallback_duration);
        self.clear_ring();

        if let Some(player) = &self.player {
            player.clear();
            let tapped = TapSource::new(decoder, Arc::clone(&self.ring));
            player.append(tapped);
            self.apply_volume();
            player.play();
        }

        self.has_loaded = true;
        self.track_duration = duration;
        Ok(())
    }

    pub fn toggle_pause(&mut self) {
        let Some(player) = &self.player else {
            return;
        };
        if !self.has_loaded || player.empty() {
            return;
        }
        if player.is_paused() {
            player.play();
        } else {
            player.pause();
            self.clear_ring();
        }
    }

    pub fn stop(&mut self) {
        if let Some(player) = &self.player {
            player.clear();
        }
        self.clear_ring();
        self.has_loaded = false;
        self.track_duration = Duration::ZERO;
    }

    pub fn seek_relative(&self, delta_secs: i64) -> Result<Duration, AudioError> {
        let Some(player) = &self.player else {
            return Ok(Duration::ZERO);
        };
        if !self.has_loaded || player.empty() {
            return Ok(Duration::ZERO);
        }
        let current = player.get_pos();
        let target = if delta_secs >= 0 {
            current.saturating_add(Duration::from_secs(delta_secs.unsigned_abs()))
        } else {
            current.saturating_sub(Duration::from_secs(delta_secs.unsigned_abs()))
        };
        let max_pos = self
            .track_duration
            .saturating_sub(Duration::from_millis(250));
        let clamped = if self.track_duration.is_zero() {
            target
        } else {
            target.min(max_pos)
        };
        player.try_seek(clamped)?;
        Ok(clamped)
    }

    pub fn adjust_volume(&mut self, delta: f32) -> f32 {
        self.muted = false;
        self.volume = (self.volume + delta).clamp(0.0, 1.5);
        self.apply_volume();
        self.volume
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.muted = !self.muted;
        self.apply_volume();
        self.muted
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn is_muted(&self) -> bool {
        self.muted
    }

    pub fn is_playing(&self) -> bool {
        self.has_loaded
            && self
                .player
                .as_ref()
                .is_some_and(|player| !player.empty() && !player.is_paused())
    }

    pub fn is_paused(&self) -> bool {
        self.has_loaded
            && self
                .player
                .as_ref()
                .is_some_and(|player| !player.empty() && player.is_paused())
    }

    pub fn is_finished(&self) -> bool {
        self.has_loaded && self.player.as_ref().is_none_or(Player::empty)
    }

    pub fn position(&self) -> Duration {
        let Some(player) = &self.player else {
            return Duration::ZERO;
        };
        if !self.has_loaded {
            return Duration::ZERO;
        }
        let pos = player.get_pos();
        if self.track_duration.is_zero() {
            pos
        } else {
            pos.min(self.track_duration)
        }
    }

    pub fn duration(&self) -> Duration {
        self.track_duration
    }

    pub fn sample_ring(&self) -> &Arc<Mutex<SampleRing>> {
        &self.ring
    }

    fn apply_volume(&self) {
        if let Some(player) = &self.player {
            let effective = if self.muted { 0.0 } else { self.volume };
            player.set_volume(effective);
        }
    }

    fn clear_ring(&self) {
        if let Ok(mut guard) = self.ring.lock() {
            guard.clear();
        }
    }
}
