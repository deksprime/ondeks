//! Audio backend abstraction and implementations.

pub mod backend;
pub mod cpal_backend;

pub use backend::{AudioBackend, AudioConfig, AudioDeviceInfo, AudioError, AudioCallback};
pub use cpal_backend::CpalBackend;

