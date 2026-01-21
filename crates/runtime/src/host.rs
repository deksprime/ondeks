//! Runtime host that manages audio processing.

use ondeks_core::{Command, Engine, State};
use ondeks_core::graph::ProcessContext;
use ondeks_core::transport::Transport;
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

/// The main runtime host.
pub struct Host {
    backend: Box<dyn AudioBackend>,
    command_queue: CommandQueue,
    event_queue: EventQueue,
    command_sender: CommandSender,
    event_receiver: EventReceiver,
    config: AudioConfig,
    state: Arc<Mutex<HostState>>,
}

struct HostState {
    engine: Engine,
    core_state: State,
    transport: Transport,
    // Test tone state
    test_tone_active: bool,
    test_tone_frequency: f32,
    test_tone_phase: f32,
    test_tone_samples_remaining: u64,
    test_tone_phase_increment: f32,
}

impl Host {
    /// Create a new host with default configuration.
    pub fn new() -> Result<Self, HostError> {
        Self::with_config(AudioConfig::default())
    }

    /// Create with specific configuration.
    pub fn with_config(config: AudioConfig) -> Result<Self, HostError> {
        let backend = Box::new(CpalBackend::new(config.clone())?);
        let command_queue = CommandQueue::new(256);
        let event_queue = EventQueue::new(256);
        
        let command_sender = command_queue.sender();
        let event_receiver = event_queue.receiver();

        let state = Arc::new(Mutex::new(HostState {
            engine: Engine::new(),
            core_state: State::new(),
            transport: Transport::new(config.sample_rate),
            test_tone_active: false,
            test_tone_frequency: 440.0,
            test_tone_phase: 0.0,
            test_tone_samples_remaining: 0,
            test_tone_phase_increment: 0.0,
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

    /// Start audio processing.
    pub fn start(&mut self) -> Result<(), HostError> {
        if self.backend.is_running() {
            return Err(HostError::AlreadyRunning);
        }

        let state = self.state.clone();
        let command_receiver = self.command_queue.receiver();
        let event_sender = self.event_queue.sender();
        let sample_rate = self.config.sample_rate;

        let callback = move |output: &mut [f32]| {
            let mut host_state = state.lock().unwrap();
            
            // Process commands
            for cmd in command_receiver.drain() {
                match cmd {
                    RuntimeCommand::Core(c) => {
                        match c {
                            Command::Play => {
                                host_state.transport = host_state.transport.play();
                            }
                            Command::Stop => {
                                host_state.transport = host_state.transport.stop();
                            }
                            Command::SetTempo(bpm) => {
                                host_state.transport = host_state.transport.set_tempo(bpm);
                            }
                        }
                    }
                    RuntimeCommand::Shutdown => return,
                    RuntimeCommand::SetAudioConfig(_) => {
                        // Configuration changes will be handled in a future phase
                    }
                    RuntimeCommand::PlayTestTone { frequency, duration } => {
                        host_state.test_tone_active = true;
                        host_state.test_tone_frequency = frequency;
                        host_state.test_tone_phase = 0.0;
                        host_state.test_tone_samples_remaining = (duration * sample_rate as f32) as u64;
                        host_state.test_tone_phase_increment = frequency / sample_rate as f32;
                    }
                }
            }

            // Create process context (will be used when processing the graph)
            let _context = ProcessContext {
                buffer_size: output.len() / 2,
                sample_rate,
                tempo: host_state.transport.tempo(),
                position: host_state.transport.position(),
                is_playing: host_state.transport.is_playing(),
            };

            // Process audio graph
            // For now, output test tone if active, otherwise silence
            // TODO: Use engine and context to process the graph in future phases
            let num_samples = output.len() / 2; // Stereo, so divide by 2
            let samples_to_generate = num_samples.min(host_state.test_tone_samples_remaining as usize);
            
            if host_state.test_tone_active && samples_to_generate > 0 {
                // Generate sine wave
                for i in 0..samples_to_generate {
                    let sample_value = (host_state.test_tone_phase * 2.0 * std::f32::consts::PI).sin() * 0.3; // 0.3 amplitude to avoid clipping
                    
                    // Write to both left and right channels (interleaved)
                    output[i * 2] = sample_value;     // Left
                    output[i * 2 + 1] = sample_value; // Right
                    
                    // Advance phase
                    host_state.test_tone_phase += host_state.test_tone_phase_increment;
                    if host_state.test_tone_phase >= 1.0 {
                        host_state.test_tone_phase -= 1.0;
                    }
                }
                
                // Fill remaining samples with silence
                for i in samples_to_generate..num_samples {
                    output[i * 2] = 0.0;
                    output[i * 2 + 1] = 0.0;
                }
                
                // Update remaining samples
                host_state.test_tone_samples_remaining = host_state.test_tone_samples_remaining.saturating_sub(samples_to_generate as u64);
                if host_state.test_tone_samples_remaining == 0 {
                    host_state.test_tone_active = false;
                }
            } else {
                // Silence
                for sample in output.iter_mut() {
                    *sample = 0.0;
                }
            }

            // Advance transport
            host_state.transport = host_state.transport.advance((output.len() / 2) as u64);

            // Calculate meters (peak values for left and right channels)
            let mut left_peak = 0.0f32;
            let mut right_peak = 0.0f32;
            for i in 0..num_samples {
                left_peak = left_peak.max(output[i * 2].abs());
                right_peak = right_peak.max(output[i * 2 + 1].abs());
            }
            event_sender.send(RuntimeEvent::MeterUpdate { left: left_peak, right: right_peak });
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

    /// Play a test tone.
    pub fn play_test_tone(&self, frequency: f32, duration: f32) -> Result<(), HostError> {
        self.command_sender.send(RuntimeCommand::PlayTestTone { frequency, duration })
            .map_err(|_| HostError::NotRunning)
    }

    /// Poll for events (non-blocking).
    pub fn poll_events(&self) -> impl Iterator<Item = RuntimeEvent> + '_ {
        self.event_receiver.drain()
    }

    /// Get audio configuration.
    pub fn config(&self) -> &AudioConfig {
        &self.config
    }

    /// Get sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.backend.sample_rate()
    }

    /// List available audio devices.
    pub fn list_devices(&self) -> Vec<AudioDeviceInfo> {
        self.backend.list_devices()
    }

    /// Check if running.
    pub fn is_running(&self) -> bool {
        self.backend.is_running()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
