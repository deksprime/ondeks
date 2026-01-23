//! Digital Signal Processing primitives.
//!
//! This module contains the fundamental types and algorithms for audio processing.

mod sample;
mod buffer;
mod stereo_buffer;
mod oscillator;
mod envelope;
mod filter;
mod delay;
mod compressor;
mod synth;

pub use sample::{Sample, SILENCE, db_to_linear, linear_to_db, clamp, lerp};
pub use buffer::Buffer;
pub use stereo_buffer::StereoBuffer;
pub use oscillator::{Oscillator, Waveform};
pub use envelope::{AdsrEnvelope, EnvelopeStage};
pub use filter::{SvFilter, FilterType};
pub use delay::StereoDelay;
pub use compressor::Compressor;
pub use synth::SimpleSynth;