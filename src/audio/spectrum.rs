use std::f32::consts::TAU;
use std::sync::{Arc, Mutex};

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::audio::tap::SampleRing;

const FFT_SIZE: usize = 1024;
const WAVE_POINTS: usize = 128;

pub struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    sample_buf: Vec<f32>,
    fft_buf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    bars: Vec<f32>,
    peaks: Vec<f32>,
    waveform: Vec<f32>,
}

impl Default for SpectrumAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl SpectrumAnalyzer {
    pub fn new() -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let scratch_len = fft.get_inplace_scratch_len();
        let denom = (FFT_SIZE - 1) as f32;
        let window = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (TAU * i as f32 / denom).cos()))
            .collect();

        Self {
            fft,
            window,
            sample_buf: vec![0.0; FFT_SIZE],
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            bars: Vec::new(),
            peaks: Vec::new(),
            waveform: vec![0.0; WAVE_POINTS],
        }
    }

    pub fn update(&mut self, ring: &Arc<Mutex<SampleRing>>, band_count: usize, is_playing: bool) {
        let count = band_count.max(1);
        if self.bars.len() != count {
            self.bars.resize(count, 0.0);
            self.peaks.resize(count, 0.0);
        }

        if !is_playing {
            for bar in &mut self.bars {
                *bar *= 0.78;
                if *bar < 0.005 {
                    *bar = 0.0;
                }
            }
            for peak in &mut self.peaks {
                *peak = (*peak - 0.025).max(0.0);
            }
            for sample in &mut self.waveform {
                *sample *= 0.75;
            }
            return;
        }

        if let Ok(guard) = ring.lock() {
            guard.copy_latest(&mut self.sample_buf);
        }

        let step = (FFT_SIZE / WAVE_POINTS).max(1);
        for (idx, slot) in self.waveform.iter_mut().enumerate() {
            let src = self.sample_buf.get(idx * step).copied().unwrap_or(0.0);
            *slot = (*slot * 0.45 + src * 0.55).clamp(-1.0, 1.0);
        }

        for ((slot, &sample), &weight) in self
            .fft_buf
            .iter_mut()
            .zip(self.sample_buf.iter())
            .zip(self.window.iter())
        {
            *slot = Complex::new(sample * weight, 0.0);
        }

        self.fft
            .process_with_scratch(&mut self.fft_buf, &mut self.scratch);

        let max_bin = FFT_SIZE / 2;
        let min_freq = 1.0f32;
        let max_freq = (max_bin as f32).min(420.0);
        let ratio = max_freq / min_freq;

        for band in 0..count {
            let low_t = band as f32 / count as f32;
            let high_t = (band + 1) as f32 / count as f32;
            let start = (min_freq * ratio.powf(low_t)).floor() as usize;
            let end = ((min_freq * ratio.powf(high_t)).ceil() as usize)
                .max(start + 1)
                .min(max_bin);

            let mut peak_mag = 0.0f32;
            for bin in &self.fft_buf[start..end] {
                let mag = bin.norm();
                if mag > peak_mag {
                    peak_mag = mag;
                }
            }

            let normalized = peak_mag / (FFT_SIZE as f32 * 0.14);
            let tilt = 1.0 + (band as f32 / count as f32) * 0.85;
            let target = ((normalized * tilt).ln_1p() * 0.62).clamp(0.0, 1.0);

            let prev = self.bars[band];
            let next = if target > prev {
                prev * 0.35 + target * 0.65
            } else {
                prev * 0.78 + target * 0.22
            };
            self.bars[band] = next;

            if next >= self.peaks[band] {
                self.peaks[band] = next;
            } else {
                self.peaks[band] = (self.peaks[band] - 0.018).max(next);
            }
        }
    }

    pub fn bars(&self) -> &[f32] {
        &self.bars
    }

    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }

    pub fn waveform(&self) -> &[f32] {
        &self.waveform
    }
}
