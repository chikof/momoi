//! Audio source trait and the built-in silent fallback.

use crate::{AudioError, AudioSpectrum};

/// Implemented by anything that can deliver audio spectrum data.
pub trait AudioSource: Send + Sync {
    /// Return the latest audio spectrum.
    ///
    /// Non-blocking; returns cached data if no new buffer has arrived.
    ///
    /// # Errors
    /// Returns [`AudioError::StreamDisconnected`] if the stream is lost.
    fn current_spectrum(&self) -> Result<AudioSpectrum, AudioError>;

    /// Human-readable name of this audio source.
    fn source_name(&self) -> &'static str;
}

/// Silent fallback — always returns a zeroed spectrum, never fails.
#[derive(Debug, Default)]
pub struct SilentSource;

impl AudioSource for SilentSource {
    fn current_spectrum(&self) -> Result<AudioSpectrum, AudioError> {
        Ok(AudioSpectrum::default())
    }

    fn source_name(&self) -> &'static str {
        "silent"
    }
}
