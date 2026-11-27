use std::sync::{Arc, Mutex};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};

const RING_CAPACITY: usize = 2048;

pub struct SampleRing {
    buffer: Box<[f32]>,
    write_pos: usize,
}

impl Default for SampleRing {
    fn default() -> Self {
        Self {
            buffer: vec![0.0; RING_CAPACITY].into_boxed_slice(),
            write_pos: 0,
        }
    }
}

impl SampleRing {
    pub fn push(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
    }

    pub fn copy_latest(&self, out: &mut [f32]) {
        let cap = self.buffer.len();
        let count = out.len().min(cap);
        let start = (self.write_pos + cap - count) % cap;
        for (idx, slot) in out.iter_mut().take(count).enumerate() {
            *slot = self.buffer[(start + idx) % cap];
        }
    }

    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }
}

pub struct TapSource<S> {
    inner: S,
    ring: Arc<Mutex<SampleRing>>,
    channel_acc: f32,
    channel_idx: u16,
}

impl<S> TapSource<S> {
    pub fn new(inner: S, ring: Arc<Mutex<SampleRing>>) -> Self {
        Self {
            inner,
            ring,
            channel_acc: 0.0,
            channel_idx: 0,
        }
    }
}

impl<S: Source> Iterator for TapSource<S> {
    type Item = Sample;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.inner.next()?;
        let channels = self.inner.channels().get();
        self.channel_acc += sample;
        self.channel_idx += 1;

        if self.channel_idx >= channels {
            let mono = self.channel_acc / f32::from(channels);
            self.channel_acc = 0.0;
            self.channel_idx = 0;
            if let Ok(mut guard) = self.ring.try_lock() {
                guard.push(mono);
            }
        }

        Some(sample)
    }
}

impl<S: Source> Source for TapSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration()
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), SeekError> {
        self.channel_acc = 0.0;
        self.channel_idx = 0;
        self.inner.try_seek(pos)
    }
}
