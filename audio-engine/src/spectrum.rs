//! Frequency-band data extracted from a raw FFT.

use serde::{Deserialize, Serialize};

/// Compact audio spectrum for shader consumption.
///
/// 32 frequency bands spanning roughly 20 Hz – 20 kHz.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AudioSpectrum {
    /// Normalised magnitude per band, range `0.0..=1.0`.
    pub bands: [f32; 32],
    /// Peak magnitude across all bands this frame.
    pub peak: f32,
    /// Root-mean-square amplitude (loudness proxy).
    pub rms: f32,
}

impl Default for AudioSpectrum {
    fn default() -> Self {
        Self {
            bands: [0.0; 32],
            peak: 0.0,
            rms: 0.0,
        }
    }
}

impl AudioSpectrum {
    /// Return the 32 bands as a flat `[f32; 32]` for direct uniform upload.
    #[must_use]
    pub fn to_f32_array(&self) -> [f32; 32] {
        self.bands
    }
}
