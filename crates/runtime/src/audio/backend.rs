//! Audio backend abstraction.

use thiserror::Error;

/// Configuration for audio backend.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Buffer size in samples.
    pub buffer_size: usize,
    /// Number of output channels.
    pub output_channels: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            buffer_size: 512,
            output_channels: 2,
        }
    }
}

/// Information about an audio device.
#[derive(Debug, Clone)]
pub struct AudioDeviceInfo {
    /// Device name.
    pub name: String,
    /// Whether this is the default device.
    pub is_default: bool,
    /// Maximum number of output channels.
    pub max_channels: usize,
}

/// Callback function type for audio processing.
pub type AudioCallback = Box<dyn FnMut(&mut [f32]) + Send>;

/// Errors that can occur in audio operations.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("No audio device available")]
    NoDevice,

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to start stream: {0}")]
    StreamError(String),

    #[error("Backend error: {0}")]
    BackendError(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Trait for audio hardware abstraction.
pub trait AudioBackend: Send {
    /// Start audio processing with the given callback.
    fn start(&mut self, callback: AudioCallback) -> Result<(), AudioError>;

    /// Stop audio processing.
    fn stop(&mut self) -> Result<(), AudioError>;

    /// Get the actual sample rate.
    fn sample_rate(&self) -> u32;

    /// Get the actual buffer size.
    fn buffer_size(&self) -> usize;

    /// Check if currently running.
    fn is_running(&self) -> bool;

    /// List available output devices.
    fn list_devices(&self) -> Vec<AudioDeviceInfo>;

    /// Select output device by name.
    fn select_device(&mut self, name: &str) -> Result<(), AudioError>;
}
