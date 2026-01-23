//! Audio backend abstraction and implementations.

pub mod backend;
pub mod cpal_backend;
pub mod input;
pub mod streaming;
pub mod resample;

pub use backend::{AudioBackend, AudioConfig, AudioDeviceInfo, AudioError, AudioCallback};
pub use cpal_backend::CpalBackend;
pub use input::{AudioInput, InputConfig, CapturedBuffer};
pub use streaming::{StreamingAudioFile, StreamingPool, AudioFileSpec, StreamingError};
pub use resample::{Resampler, ResampleQuality, resample_buffer};
