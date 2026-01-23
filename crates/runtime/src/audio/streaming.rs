use std::path::{Path, PathBuf};
use std::collections::HashMap;
use ondeks_core::AudioPoolId;

/// How much audio to keep buffered ahead (in seconds).
const LOOKAHEAD_SECONDS: f32 = 2.0;
/// How much audio to keep buffered behind (for reverse playback).
const LOOKBEHIND_SECONDS: f32 = 0.5;

/// A streaming audio file.
#[derive(Debug)]
pub struct StreamingAudioFile {
    path: PathBuf,
    spec: AudioFileSpec,
    /// Current playback position in samples.
    position: u64,
    /// Cached audio data: (start_sample, samples).
    cache: AudioCache,
}

/// Audio file specifications.
#[derive(Debug, Clone)]
pub struct AudioFileSpec {
    pub sample_rate: u32,
    pub channels: usize,
    pub total_samples: u64,
    pub bit_depth: u16,
}

impl AudioFileSpec {
    pub fn duration_seconds(&self) -> f32 {
        self.total_samples as f32 / self.sample_rate as f32
    }
}

/// Cached portion of an audio file.
#[derive(Debug)]
struct AudioCache {
    start_sample: u64,
    samples: Vec<f32>,
    channels: usize,
}

impl AudioCache {
    fn new() -> Self {
        Self {
            start_sample: 0,
            samples: Vec::new(),
            channels: 2,
        }
    }

    fn contains(&self, sample: u64) -> bool {
        let frame_count = self.samples.len() / self.channels;
        sample >= self.start_sample && sample < self.start_sample + frame_count as u64
    }

    fn get(&self, sample: u64, channels: usize) -> Option<&[f32]> {
        if !self.contains(sample) {
            return None;
        }
        let offset = ((sample - self.start_sample) as usize) * channels;
        if offset + channels <= self.samples.len() {
            Some(&self.samples[offset..offset + channels])
        } else {
            None
        }
    }
}

impl StreamingAudioFile {
    /// Open an audio file for streaming.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StreamingError> {
        let path = path.as_ref().to_path_buf();
        
        // Read header to get spec
        let reader = hound::WavReader::open(&path)
            .map_err(|e| StreamingError::IoError(e.to_string()))?;
        
        let spec = reader.spec();
        let total_samples = reader.duration() as u64;

        Ok(Self {
            path,
            spec: AudioFileSpec {
                sample_rate: spec.sample_rate,
                channels: spec.channels as usize,
                total_samples,
                bit_depth: spec.bits_per_sample,
            },
            position: 0,
            cache: AudioCache::new(),
        })
    }

    /// Get file specifications.
    pub fn spec(&self) -> &AudioFileSpec {
        &self.spec
    }

    /// Seek to a sample position.
    pub fn seek(&mut self, sample: u64) {
        self.position = sample.min(self.spec.total_samples);
    }

    /// Read samples into buffer. Returns number of frames read.
    pub fn read(&mut self, output: &mut [f32], _sample_rate: u32) -> usize {
        let channels = self.spec.channels;
        let frames_requested = output.len() / channels;
        
        // Ensure cache is filled around current position
        self.ensure_cached(self.position, frames_requested);

        let mut frames_read = 0;
        for frame in 0..frames_requested {
            let sample_pos = self.position + frame as u64;
            
            if sample_pos >= self.spec.total_samples {
                // End of file - fill with silence
                for ch in 0..channels {
                    output[frame * channels + ch] = 0.0;
                }
            } else if let Some(samples) = self.cache.get(sample_pos, channels) {
                // Copy from cache
                for ch in 0..channels {
                    output[frame * channels + ch] = samples.get(ch).copied().unwrap_or(0.0);
                }
                frames_read = frame + 1;
            } else {
                // Cache miss - shouldn't happen if ensure_cached worked
                for ch in 0..channels {
                    output[frame * channels + ch] = 0.0;
                }
            }
        }

        self.position += frames_read as u64;
        frames_read
    }

    /// Ensure the cache contains data around the given position.
    fn ensure_cached(&mut self, position: u64, min_frames: usize) {
        // Check if we need to reload
        let need_reload = !self.cache.contains(position) 
            || !self.cache.contains(position + min_frames as u64);

        if need_reload {
            self.load_cache_around(position);
        }
    }

    /// Load cache around a position.
    fn load_cache_around(&mut self, position: u64) {
        let lookahead_samples = (LOOKAHEAD_SECONDS * self.spec.sample_rate as f32) as u64;
        let lookbehind_samples = (LOOKBEHIND_SECONDS * self.spec.sample_rate as f32) as u64;

        let start = position.saturating_sub(lookbehind_samples);
        let end = (position + lookahead_samples).min(self.spec.total_samples);
        let frames_to_load = (end - start) as usize;

        // Read from file
        if let Ok(mut reader) = hound::WavReader::open(&self.path) {
            // Seek to start position
            // Note: hound doesn't have efficient seeking, so for production
            // you'd want to use a different approach or raw file access
            let samples_to_skip = start * self.spec.channels as u64;
            
            let mut samples = Vec::with_capacity(frames_to_load * self.spec.channels);
            
            // Read samples based on format
            match (self.spec.bit_depth, reader.spec().sample_format) {
                (16, hound::SampleFormat::Int) => {
                    for sample in reader.samples::<i16>().skip(samples_to_skip as usize).take(frames_to_load * self.spec.channels) {
                        if let Ok(s) = sample {
                            samples.push(s as f32 / i16::MAX as f32);
                        }
                    }
                }
                (24, hound::SampleFormat::Int) => {
                    for sample in reader.samples::<i32>().skip(samples_to_skip as usize).take(frames_to_load * self.spec.channels) {
                        if let Ok(s) = sample {
                            samples.push(s as f32 / 8388607.0);
                        }
                    }
                }
                (32, hound::SampleFormat::Float) => {
                    for sample in reader.samples::<f32>().skip(samples_to_skip as usize).take(frames_to_load * self.spec.channels) {
                        if let Ok(s) = sample {
                            samples.push(s);
                        }
                    }
                }
                _ => {}
            }

            self.cache = AudioCache {
                start_sample: start,
                samples,
                channels: self.spec.channels,
            };
        }
    }
}

/// Manages multiple streaming files.
pub struct StreamingPool {
    files: HashMap<AudioPoolId, StreamingAudioFile>,
}

impl StreamingPool {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: AudioPoolId, file: StreamingAudioFile) {
        self.files.insert(id, file);
    }

    pub fn get(&self, id: AudioPoolId) -> Option<&StreamingAudioFile> {
        self.files.get(&id)
    }

    pub fn get_mut(&mut self, id: AudioPoolId) -> Option<&mut StreamingAudioFile> {
        self.files.get_mut(&id)
    }

    pub fn remove(&mut self, id: AudioPoolId) -> Option<StreamingAudioFile> {
        self.files.remove(&id)
    }
}

impl Default for StreamingPool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StreamingError {
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_cache_contains() {
        let cache = AudioCache {
            start_sample: 100,
            samples: vec![0.0; 20], // 10 frames at stereo
            channels: 2,
        };

        assert!(cache.contains(100));
        assert!(cache.contains(109));
        assert!(!cache.contains(99));
        assert!(!cache.contains(110));
    }

    #[test]
    fn streaming_pool_operations() {
        let pool = StreamingPool::new();
        let id = AudioPoolId::generate();

        assert!(pool.get(id).is_none());
        
        // Would need a real audio file to test fully
        // Just testing the API structure here
    }
}
