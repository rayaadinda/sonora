use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

use crate::audio::tap::{SampleRing, TapSource};
use crate::error::AudioError;

pub struct AudioEngine {
    _sink: MixerDeviceSink,
    player: Player,
    ring: Arc<Mutex<SampleRing>>,
    volume: f32,
    muted: bool,
    has_loaded: bool,
    track_duration: Duration,
}

impl AudioEngine {
    pub fn new(initial_volume: f32) -> Result<Self, AudioError> {
        let mut sink = DeviceSinkBuilder::open_default_sink()?;
        sink.log_on_drop(false);
        let player = Player::connect_new(sink.mixer());
        let volume = initial_volume.clamp(0.0, 1.5);
        player.set_volume(volume);

        Ok(Self {
            _sink: sink,
            player,
            ring: Arc::new(Mutex::new(SampleRing::default())),
            volume,
            muted: false,
            has_loaded: false,
            track_duration: Duration::ZERO,
        })
    }

    pub fn play_file(
        &mut self,
        path: &Path,
        fallback_duration: Duration,
    ) -> Result<(), AudioError> {
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
        self.player.clear();

        let tapped = TapSource::new(decoder, Arc::clone(&self.ring));
        self.player.append(tapped);
        self.apply_volume();
        self.player.play();

        self.has_loaded = true;
        self.track_duration = duration;
        Ok(())
    }

    pub fn toggle_pause(&mut self) {
        if !self.has_loaded || self.player.empty() {
            return;
        }
        if self.player.is_paused() {
            self.player.play();
        } else {
            self.player.pause();
            self.clear_ring();
        }
    }

    pub fn stop(&mut self) {
        self.player.clear();
        self.clear_ring();
        self.has_loaded = false;
        self.track_duration = Duration::ZERO;
    }

    pub fn seek_relative(&self, delta_secs: i64) -> Result<Duration, AudioError> {
        if !self.has_loaded || self.player.empty() {
            return Ok(Duration::ZERO);
        }
        let current = self.player.get_pos();
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
        self.player.try_seek(clamped)?;
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
        self.has_loaded && !self.player.empty() && !self.player.is_paused()
    }

    pub fn is_paused(&self) -> bool {
        self.has_loaded && !self.player.empty() && self.player.is_paused()
    }

    pub fn is_finished(&self) -> bool {
        self.has_loaded && self.player.empty()
    }

    pub fn position(&self) -> Duration {
        if !self.has_loaded {
            return Duration::ZERO;
        }
        let pos = self.player.get_pos();
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
        let effective = if self.muted { 0.0 } else { self.volume };
        self.player.set_volume(effective);
    }

    fn clear_ring(&self) {
        if let Ok(mut guard) = self.ring.lock() {
            guard.clear();
        }
    }
}
