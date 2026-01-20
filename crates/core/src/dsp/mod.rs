//! Digital Signal Processing primitives.
//!
//! This module contains the fundamental types and algorithms for audio processing.

mod sample;
mod buffer;
mod stereo_buffer;

pub use sample::{Sample, SILENCE, db_to_linear, linear_to_db, clamp, lerp};
pub use buffer::Buffer;
pub use stereo_buffer::StereoBuffer;
