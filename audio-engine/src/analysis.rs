//! FFT-based audio analysis converting PCM samples into frequency bands.

use crate::{AudioError, AudioSpectrum};
use rustfft::{FftPlanner, num_complex::Complex};
use tracing::debug;

/// Runs FFT on interleaved PCM f32 samples and produces an [`AudioSpectrum`].
pub struct AudioAnalyser {
    fft_size: usize,
    planner: FftPlanner<f32>,
}

impl AudioAnalyser {
    /// Create an analyser with the given FFT window size.
    /// `fft_size` must be a power of two.
    ///
    /// # Errors
    /// Returns [`AudioError::FftConfig`] if `fft_size` is not a power of two.
    pub fn new(fft_size: usize) -> Result<Self, AudioError> {
        if !fft_size.is_power_of_two() {
            return Err(AudioError::FftConfig(format!(
                "fft_size must be power of two, got {fft_size}"
            )));
        }
        debug!(fft_size, "audio analyser created");
        Ok(Self {
            fft_size,
            planner: FftPlanner::new(),
        })
    }

    /// Analyse a slice of mono f32 PCM samples.
    ///
    /// Samples outside `[0.0, fft_size]` are silently discarded.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::similar_names
    )]
    pub fn analyse(&mut self, samples: &[f32]) -> AudioSpectrum {
        let len = self.fft_size.min(samples.len());
        let mut buffer: Vec<Complex<f32>> = samples[..len]
            .iter()
            .map(|&s| Complex { re: s, im: 0.0 })
            .collect();

        // Zero-pad if we have fewer samples than fft_size.
        buffer.resize(self.fft_size, Complex::default());

        let fft = self.planner.plan_fft_forward(self.fft_size);
        fft.process(&mut buffer);

        // Only the first half (positive frequencies) is useful.
        let half = self.fft_size / 2;
        let magnitudes: Vec<f32> = buffer[..half]
            .iter()
            .map(|c| c.norm() / (self.fft_size as f32).sqrt())
            .collect();

        // Bin into 32 logarithmically-spaced bands.
        let mut bands = [0.0f32; 32];
        let log_min = (20_f32).log2();
        let log_max = (20_000_f32.min(half as f32)).log2();
        let step = (log_max - log_min) / 32.0;
        let bin_hz = 48_000.0 / self.fft_size as f32;

        for (band_idx, band) in bands.iter_mut().enumerate() {
            let freq_lo = 2_f32.powf(log_min + step * band_idx as f32);
            let freq_hi = 2_f32.powf(log_min + step * (band_idx + 1) as f32);
            let bin_lo = (freq_lo / bin_hz) as usize;
            let bin_hi = ((freq_hi / bin_hz) as usize + 1).min(half);
            if bin_lo < bin_hi {
                *band = magnitudes[bin_lo..bin_hi]
                    .iter()
                    .copied()
                    .fold(0.0_f32, f32::max);
            }
        }

        let peak = bands.iter().copied().fold(0.0_f32, f32::max);
        let rms = (magnitudes.iter().map(|m| m * m).sum::<f32>() / half as f32).sqrt();

        // Normalise bands against peak to keep them in 0..1.
        if peak > f32::EPSILON {
            for b in &mut bands {
                *b /= peak;
            }
        }

        AudioSpectrum { bands, peak, rms }
    }
}
