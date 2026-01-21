//! CPAL audio backend implementation.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use super::backend::{AudioBackend, AudioCallback, AudioConfig, AudioDeviceInfo, AudioError};

/// CPAL-based audio backend.
pub struct CpalBackend {
    host: cpal::Host,
    device: Option<cpal::Device>,
    stream: Option<cpal::Stream>,
    config: AudioConfig,
    is_running: bool,
}

// CPAL Stream is actually Send-safe, but the type system doesn't know it
unsafe impl Send for CpalBackend {}

impl CpalBackend {
    /// Create a new CPAL backend with the given configuration.
    pub fn new(config: AudioConfig) -> Result<Self, AudioError> {
        let host = cpal::default_host();
        let device = host.default_output_device();

        Ok(Self {
            host,
            device,
            stream: None,
            config,
            is_running: false,
        })
    }
}

impl AudioBackend for CpalBackend {
    fn start(&mut self, callback: AudioCallback) -> Result<(), AudioError> {
        let device = self.device.as_ref()
            .ok_or(AudioError::NoDevice)?;

        let supported_config = device.default_output_config()
            .map_err(|e| AudioError::BackendError(e.to_string()))?;

        let sample_rate = supported_config.sample_rate().0;
        self.config.sample_rate = sample_rate;

        let channels = supported_config.channels() as usize;
        
        let callback = Arc::new(Mutex::new(callback));
        let callback_clone = callback.clone();

        let stream_config = cpal::StreamConfig {
            channels: channels as u16,
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                if let Ok(mut cb) = callback_clone.lock() {
                    cb(data);
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        ).map_err(|e| AudioError::StreamError(e.to_string()))?;

        stream.play().map_err(|e| AudioError::StreamError(e.to_string()))?;

        self.stream = Some(stream);
        self.is_running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), AudioError> {
        self.stream = None;
        self.is_running = false;
        Ok(())
    }

    fn sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    fn buffer_size(&self) -> usize {
        self.config.buffer_size
    }

    fn is_running(&self) -> bool {
        self.is_running
    }

    fn list_devices(&self) -> Vec<AudioDeviceInfo> {
        let default_name = self.host.default_output_device()
            .and_then(|d| d.name().ok());

        self.host.output_devices()
            .map(|devices| {
                devices.filter_map(|d| {
                    let name = d.name().ok()?;
                    Some(AudioDeviceInfo {
                        is_default: Some(&name) == default_name.as_ref(),
                        name,
                        max_channels: d.default_output_config()
                            .map(|c| c.channels() as usize)
                            .unwrap_or(2),
                    })
                }).collect()
            })
            .unwrap_or_default()
    }

    fn select_device(&mut self, name: &str) -> Result<(), AudioError> {
        let device = self.host.output_devices()
            .map_err(|e| AudioError::BackendError(e.to_string()))?
            .find(|d| d.name().ok().as_deref() == Some(name))
            .ok_or_else(|| AudioError::DeviceNotFound(name.to_string()))?;

        self.device = Some(device);
        Ok(())
    }
}
