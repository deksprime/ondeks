use ondeks_core::{TrackId, transport::SampleTime};
use std::collections::HashMap;
use crossbeam_channel::Receiver;
use super::super::audio::input::CapturedBuffer;

/// State of a recording on a track.
#[derive(Debug)]
struct TrackRecording {
    track_id: TrackId,
    start_position: SampleTime,
    samples: Vec<f32>,
    channels: usize,
}

/// Recording mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordMode {
    /// Normal recording - creates new clip.
    Normal,
    /// Punch in/out - only record within loop region.
    Punch,
    /// Overdub - layer on top of existing clips.
    Overdub,
}

/// Manages recording across tracks.
pub struct Recorder {
    /// Active recordings per track.
    active_recordings: HashMap<TrackId, TrackRecording>,
    /// Recording mode.
    mode: RecordMode,
    /// Sample rate for position calculations.
    sample_rate: u32,
    /// Pre-roll buffer (keeps last N seconds for retroactive recording).
    pre_roll_seconds: f32,
}

impl Recorder {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            active_recordings: HashMap::new(),
            mode: RecordMode::Normal,
            sample_rate,
            pre_roll_seconds: 2.0,
        }
    }

    /// Set recording mode.
    pub fn set_mode(&mut self, mode: RecordMode) {
        self.mode = mode;
    }

    /// Start recording on a track.
    pub fn start_recording(&mut self, track_id: TrackId, position: SampleTime, channels: usize) {
        self.active_recordings.insert(track_id, TrackRecording {
            track_id,
            start_position: position,
            samples: Vec::new(),
            channels,
        });
    }

    /// Stop recording on a track and return the recorded clip.
    pub fn stop_recording(&mut self, track_id: TrackId) -> Option<RecordedAudio> {
        self.active_recordings.remove(&track_id).map(|rec| {
            let length_samples = rec.samples.len() / rec.channels;
            RecordedAudio {
                track_id: rec.track_id,
                start_position: rec.start_position,
                samples: rec.samples,
                channels: rec.channels,
                sample_rate: self.sample_rate,
            }
        })
    }

    /// Stop all recordings.
    pub fn stop_all(&mut self) -> Vec<RecordedAudio> {
        let track_ids: Vec<_> = self.active_recordings.keys().copied().collect();
        track_ids.into_iter()
            .filter_map(|id| self.stop_recording(id))
            .collect()
    }

    /// Process captured audio buffer.
    pub fn process_captured(&mut self, track_id: TrackId, buffer: CapturedBuffer) {
        if let Some(recording) = self.active_recordings.get_mut(&track_id) {
            recording.samples.extend(buffer.samples);
        }
    }

    /// Check if any recording is active.
    pub fn is_recording(&self) -> bool {
        !self.active_recordings.is_empty()
    }

    /// Check if a specific track is recording.
    pub fn is_track_recording(&self, track_id: TrackId) -> bool {
        self.active_recordings.contains_key(&track_id)
    }

    /// Get current recording length for a track (in samples).
    pub fn recording_length(&self, track_id: TrackId) -> Option<usize> {
        self.active_recordings.get(&track_id)
            .map(|r| r.samples.len() / r.channels)
    }
}

/// Completed recording ready to be converted to a clip.
#[derive(Debug)]
pub struct RecordedAudio {
    pub track_id: TrackId,
    pub start_position: SampleTime,
    pub samples: Vec<f32>,
    pub channels: usize,
    pub sample_rate: u32,
}

impl RecordedAudio {
    /// Get duration in seconds.
    pub fn duration_seconds(&self) -> f32 {
        (self.samples.len() / self.channels) as f32 / self.sample_rate as f32
    }

    /// Get duration in samples (per channel).
    pub fn duration_samples(&self) -> usize {
        self.samples.len() / self.channels
    }

    /// Convert to mono by averaging channels.
    pub fn to_mono(&self) -> Vec<f32> {
        let frames = self.samples.len() / self.channels;
        let mut mono = Vec::with_capacity(frames);
        
        for frame in 0..frames {
            let mut sum = 0.0;
            for ch in 0..self.channels {
                sum += self.samples[frame * self.channels + ch];
            }
            mono.push(sum / self.channels as f32);
        }
        
        mono
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_and_stop_recording() {
        let mut recorder = Recorder::new(44100);
        let track_id = TrackId::generate();
        
        assert!(!recorder.is_recording());
        
        recorder.start_recording(track_id, SampleTime(0), 2);
        assert!(recorder.is_recording());
        assert!(recorder.is_track_recording(track_id));
        
        let result = recorder.stop_recording(track_id);
        assert!(result.is_some());
        assert!(!recorder.is_recording());
    }

    #[test]
    fn process_captured_buffer() {
        let mut recorder = Recorder::new(44100);
        let track_id = TrackId::generate();
        
        recorder.start_recording(track_id, SampleTime(0), 2);
        
        let buffer = CapturedBuffer {
            samples: vec![0.1, 0.2, 0.3, 0.4],
            channels: 2,
            position: 0,
            timestamp_ns: 0,
        };
        
        recorder.process_captured(track_id, buffer);
        
        let length = recorder.recording_length(track_id);
        assert_eq!(length, Some(2)); // 4 samples / 2 channels = 2 frames
    }

    #[test]
    fn recorded_audio_conversion() {
        let audio = RecordedAudio {
            track_id: TrackId::generate(),
            start_position: SampleTime(0),
            samples: vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            channels: 2,
            sample_rate: 44100,
        };
        
        assert_eq!(audio.duration_samples(), 3);
        
        let mono = audio.to_mono();
        assert_eq!(mono.len(), 3);
        // First frame average: (0.1 + 0.2) / 2 = 0.15
        assert!((mono[0] - 0.15).abs() < 0.001);
    }
}
