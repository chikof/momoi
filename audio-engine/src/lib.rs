//! # audio-engine
//!
//! Real-time audio capture and FFT analysis.
//! Uses `PipeWire` when the `pipewire-audio` feature is enabled;
//! falls back to a silent stub that always returns zeroed spectrum data.

pub mod analysis;
pub mod error;
pub mod source;
pub mod spectrum;

pub use analysis::AudioAnalyser;
pub use error::AudioError;
pub use source::{AudioSource, SilentSource};
pub use spectrum::AudioSpectrum;

#[cfg(feature = "pipewire-audio")]
pub mod pipewire_source;
