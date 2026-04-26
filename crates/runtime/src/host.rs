//! Runtime host: owns the audio backend and drives the engine from the audio callback.

use ondeks_core::{Command, Engine};
use ondeks_core::dsp::StereoBuffer;
use crate::audio::{AudioBackend, AudioConfig, AudioDeviceInfo, AudioError, CpalBackend};
use crate::queue::{CommandQueue, CommandSender, EventQueue, EventReceiver, RuntimeCommand, RuntimeEvent};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum HostError {
    #[error("Audio error: {0}")]
    Audio(#[from] AudioError),

    #[error("Already running")]
    AlreadyRunning,

    #[error("Not running")]
    NotRunning,
}

/// Runtime host. Owns the audio backend; drives `Engine::process()` from the callback.
pub struct Host {
    backend: Box<dyn AudioBackend>,
    command_queue: CommandQueue,
    event_queue: EventQueue,
    command_sender: CommandSender,
    event_receiver: EventReceiver,
    config: AudioConfig,
    state: Arc<Mutex<HostState>>,
}

/// State owned by the audio thread (wrapped in Mutex for cross-thread init,
/// but contended only briefly at command-drain boundaries).
struct HostState {
    engine: Engine,
    /// Stereo buffer the engine writes into. Pre-allocated; resized only if
    /// CPAL delivers an unexpectedly large block.
    master_buffer: StereoBuffer,
    /// Throttle counter for position updates (fires every N blocks).
    position_update_counter: u32,
}

impl Host {
    /// Create a new host with default audio configuration.
    pub fn new() -> Result<Self, HostError> {
        Self::with_config(AudioConfig::default())
    }

    /// Create a host with the given audio configuration.
    pub fn with_config(config: AudioConfig) -> Result<Self, HostError> {
        let backend = Box::new(CpalBackend::new(config.clone())?);
        let command_queue = CommandQueue::new(256);
        let event_queue = EventQueue::new(256);

        let command_sender = command_queue.sender();
        let event_receiver = event_queue.receiver();

        let buffer_size = config.buffer_size as usize;
        let sample_rate = config.sample_rate;

        let state = Arc::new(Mutex::new(HostState {
            engine: Engine::new(sample_rate, buffer_size),
            master_buffer: StereoBuffer::allocate(buffer_size),
            position_update_counter: 0,
        }));

        Ok(Self {
            backend,
            command_queue,
            event_queue,
            command_sender,
            event_receiver,
            config,
            state,
        })
    }

    /// Start audio processing. Spawns the CPAL callback; the callback drives the engine.
    pub fn start(&mut self) -> Result<(), HostError> {
        if self.backend.is_running() {
            return Err(HostError::AlreadyRunning);
        }

        let state = self.state.clone();
        let command_receiver = self.command_queue.receiver();
        let event_sender = self.event_queue.sender();

        let callback = move |output: &mut [f32]| {
            let mut guard = state.lock().unwrap();
            // Reborrow through the guard so the borrow checker can split
            // disjoint field accesses (engine / master_buffer / counter).
            let host_state: &mut HostState = &mut guard;

            // Drain command queue: apply to engine, emit mirror events for UI.
            for cmd in command_receiver.drain() {
                match cmd {
                    RuntimeCommand::Core(c) => {
                        host_state.engine.apply_command(c.clone());
                        match c {
                            Command::Play => {
                                event_sender.send(RuntimeEvent::TransportStateChanged {
                                    is_playing: true,
                                });
                            }
                            Command::Stop => {
                                event_sender.send(RuntimeEvent::TransportStateChanged {
                                    is_playing: false,
                                });
                            }
                            _ => {}
                        }
                    }
                    RuntimeCommand::Shutdown => return,
                    RuntimeCommand::SetAudioConfig(_) => {
                        // Device reconfiguration handled in a later slice.
                    }
                    RuntimeCommand::PlayTestTone { .. } => {
                        // Deprecated: the default engine graph already produces a tone
                        // on Play. Ignored; preserved for wire-compat until Slice 4
                        // ships a proper CLI 'test tone' path.
                    }
                }
            }

            // CPAL delivers interleaved stereo: 2 samples per frame.
            let frames = output.len() / 2;

            // Resize the master buffer if the block size changed (rare).
            if host_state.master_buffer.len() != frames {
                host_state.master_buffer = StereoBuffer::allocate(frames);
                host_state.engine.set_buffer_size(frames);
            }

            // Run the engine. Writes into master_buffer.
            host_state.master_buffer.silence();
            host_state.engine.process(&mut host_state.master_buffer, frames as u32);

            // Interleave master_buffer → CPAL output.
            let left = host_state.master_buffer.left().as_slice();
            let right = host_state.master_buffer.right().as_slice();
            for i in 0..frames {
                output[i * 2] = left[i];
                output[i * 2 + 1] = right[i];
            }

            // Position update: throttle to every 2 blocks for smooth UI display.
            host_state.position_update_counter += 1;
            if host_state.position_update_counter >= 2 {
                host_state.position_update_counter = 0;
                let transport = host_state.engine.transport();
                event_sender.send(RuntimeEvent::PositionUpdate {
                    samples: transport.position().0,
                    beats: transport.position_beats().0,
                    bbt: transport.position_bbt(),
                });
            }

            // Meter update: peak of each channel.
            let mut left_peak = 0.0f32;
            let mut right_peak = 0.0f32;
            for &s in left {
                let a = s.abs();
                if a > left_peak { left_peak = a; }
            }
            for &s in right {
                let a = s.abs();
                if a > right_peak { right_peak = a; }
            }
            event_sender.send(RuntimeEvent::MeterUpdate {
                left: left_peak,
                right: right_peak,
            });
        };

        self.backend.start(Box::new(callback))?;
        Ok(())
    }

    /// Stop audio processing.
    pub fn stop(&mut self) -> Result<(), HostError> {
        if !self.backend.is_running() {
            return Err(HostError::NotRunning);
        }
        self.backend.stop()?;
        Ok(())
    }

    /// Send a command to the audio engine.
    pub fn send_command(&self, command: Command) -> Result<(), HostError> {
        self.command_sender.send(RuntimeCommand::Core(command))
            .map_err(|_| HostError::NotRunning)
    }

    /// Deprecated: use `send_command(Command::Play)` instead — the default graph
    /// produces a tone when playing. Kept for wire compatibility.
    #[deprecated(note = "use send_command(Command::Play); default graph produces a tone")]
    pub fn play_test_tone(&self, frequency: f32, duration: f32) -> Result<(), HostError> {
        self.command_sender.send(RuntimeCommand::PlayTestTone { frequency, duration })
            .map_err(|_| HostError::NotRunning)
    }

    /// Poll for events from the engine (non-blocking).
    pub fn poll_events(&self) -> impl Iterator<Item = RuntimeEvent> + '_ {
        self.event_receiver.drain()
    }

    /// Get audio configuration.
    pub fn config(&self) -> &AudioConfig {
        &self.config
    }

    /// Get the current sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.backend.sample_rate()
    }

    /// List available audio devices.
    pub fn list_devices(&self) -> Vec<AudioDeviceInfo> {
        self.backend.list_devices()
    }

    /// Check if the host is currently running.
    pub fn is_running(&self) -> bool {
        self.backend.is_running()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
