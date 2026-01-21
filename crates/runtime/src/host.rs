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
            // For now, just output silence or a test tone
            // TODO: Use engine and context to process the graph in future phases
            for sample in output.iter_mut() {
                *sample = 0.0;
            }

            // Advance transport
            host_state.transport = host_state.transport.advance((output.len() / 2) as u64);

            // Send meter update
            event_sender.send(RuntimeEvent::MeterUpdate { left: 0.0, right: 0.0 });
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
