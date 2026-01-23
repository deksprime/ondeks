use std::sync::{Arc, Mutex};
use crossbeam_channel::{Sender, Receiver, bounded};

/// Configuration for audio input.
#[derive(Debug, Clone)]
pub struct InputConfig {
    /// Which hardware input channels to capture (0-indexed).
    pub channels: Vec<usize>,
    /// Whether to enable input monitoring (hear input through output).
    pub monitoring: bool,
    /// Monitoring volume in dB.
    pub monitor_level_db: f32,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            channels: vec![0, 1], // Stereo input
            monitoring: false,
            monitor_level_db: 0.0,
        }
    }
}

/// Manages audio input capture.
pub struct AudioInput {
    config: InputConfig,
    /// Ring buffer for captured audio
    capture_buffer: Arc<Mutex<CaptureRingBuffer>>,
    /// Channel to send captured buffers to the recording system
    capture_sender: Sender<CapturedBuffer>,
    /// Receiver for captured buffers
    capture_receiver: Receiver<CapturedBuffer>,
    is_armed: bool,
    is_recording: bool,
}

/// A buffer of captured audio with timestamp.
#[derive(Debug, Clone)]
pub struct CapturedBuffer {
    /// Audio data (interleaved if stereo).
    pub samples: Vec<f32>,
    /// Number of channels.
    pub channels: usize,
    /// Position in samples when this was captured.
    pub position: u64,
    /// Timestamp when capture started (for latency compensation).
    pub timestamp_ns: u64,
}

/// Ring buffer for low-latency capture.
struct CaptureRingBuffer {
    buffer: Vec<f32>,
    write_pos: usize,
    capacity: usize,
}

impl CaptureRingBuffer {
    fn new(capacity_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; capacity_samples],
            write_pos: 0,
            capacity: capacity_samples,
        }
    }

    /// Write samples to the ring buffer.
    fn write(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % self.capacity;
        }
    }

    /// Read the last N samples.
    fn read_last(&self, count: usize) -> Vec<f32> {
        let mut result = Vec::with_capacity(count);
        let start = (self.write_pos + self.capacity - count) % self.capacity;
        for i in 0..count {
            result.push(self.buffer[(start + i) % self.capacity]);
        }
        result
    }
}

impl AudioInput {
    pub fn new(config: InputConfig, buffer_capacity: usize) -> Self {
        let (sender, receiver) = bounded(64);
        Self {
            config,
            capture_buffer: Arc::new(Mutex::new(CaptureRingBuffer::new(buffer_capacity))),
            capture_sender: sender,
            capture_receiver: receiver,
            is_armed: false,
            is_recording: false,
        }
    }

    /// Arm the input for recording.
    pub fn arm(&mut self) {
        self.is_armed = true;
    }

    /// Disarm the input.
    pub fn disarm(&mut self) {
        self.is_armed = false;
        self.is_recording = false;
    }

    /// Start recording (must be armed first).
    pub fn start_recording(&mut self, _position: u64) -> bool {
        if self.is_armed {
            self.is_recording = true;
            true
        } else {
            false
        }
    }

    /// Stop recording.
    pub fn stop_recording(&mut self) {
        self.is_recording = false;
    }

    /// Process incoming audio from hardware.
    /// Called from audio callback with input buffer.
    pub fn process_input(
        &mut self,
        input_samples: &[f32],
        input_channels: usize,
        position: u64,
        output_for_monitoring: Option<&mut [f32]>,
    ) {
        // Extract the channels we care about
        let extracted = self.extract_channels(input_samples, input_channels);

        // Write to ring buffer (always, for monitoring)
        if let Ok(mut buffer) = self.capture_buffer.lock() {
            buffer.write(&extracted);
        }

        // If recording, send to recording system
        if self.is_recording {
            let captured = CapturedBuffer {
                samples: extracted.clone(),
                channels: self.config.channels.len(),
                position,
                timestamp_ns: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64,
            };
            let _ = self.capture_sender.try_send(captured);
        }

        // Input monitoring
        if self.config.monitoring {
            if let Some(output) = output_for_monitoring {
                let gain = db_to_linear(self.config.monitor_level_db);
                let monitor_channels = self.config.channels.len().min(2);
                
                for (i, sample) in output.iter_mut().enumerate() {
                    let ch = i % monitor_channels;
                    let frame = i / monitor_channels;
                    if frame * self.config.channels.len() + ch < extracted.len() {
                        // Mix monitoring signal into output
                        *sample += extracted[frame * self.config.channels.len() + ch] * gain;
                    }
                }
            }
        }
    }

    /// Extract configured channels from interleaved input.
    fn extract_channels(&self, input: &[f32], total_channels: usize) -> Vec<f32> {
        let frames = input.len() / total_channels;
        let mut result = Vec::with_capacity(frames * self.config.channels.len());

        for frame in 0..frames {
            for &ch in &self.config.channels {
                if ch < total_channels {
                    result.push(input[frame * total_channels + ch]);
                } else {
                    result.push(0.0); // Channel doesn't exist, use silence
                }
            }
        }

        result
    }

    /// Get receiver for captured buffers (for the recording system).
    pub fn capture_receiver(&self) -> Receiver<CapturedBuffer> {
        self.capture_receiver.clone()
    }

    /// Check if currently recording.
    pub fn is_recording(&self) -> bool {
        self.is_recording
    }

    /// Check if armed.
    pub fn is_armed(&self) -> bool {
        self.is_armed
    }
}

/// Convert dB to linear gain.
fn db_to_linear(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_channels_stereo_from_stereo() {
        let input = AudioInput::new(InputConfig::default(), 1024);
        let interleaved = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6]; // 3 frames, 2 channels
        let extracted = input.extract_channels(&interleaved, 2);
        assert_eq!(extracted, vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6]);
    }

    #[test]
    fn extract_mono_from_stereo() {
        let input = AudioInput::new(
            InputConfig {
                channels: vec![0], // Just left channel
                ..Default::default()
            },
            1024,
        );
        let interleaved = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
        let extracted = input.extract_channels(&interleaved, 2);
        assert_eq!(extracted, vec![0.1, 0.3, 0.5]);
    }

    #[test]
    fn arm_and_record() {
        let mut input = AudioInput::new(InputConfig::default(), 1024);
        
        assert!(!input.is_armed());
        assert!(!input.is_recording());
        
        // Can't record without arming
        assert!(!input.start_recording(0));
        assert!(!input.is_recording());
        
        // Arm and record
        input.arm();
        assert!(input.is_armed());
        assert!(input.start_recording(0));
        assert!(input.is_recording());
        
        // Stop
        input.stop_recording();
        assert!(!input.is_recording());
        assert!(input.is_armed()); // Still armed
    }
}
