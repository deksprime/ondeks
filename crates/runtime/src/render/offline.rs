use ondeks_core::project::Project;
use ondeks_core::transport::{Beats, Transport, SampleTime};
use ondeks_core::graph::{AudioGraph, GraphProcessor, ProcessContext};
use ondeks_core::dsp::StereoBuffer;
use std::path::Path;

/// Configuration for offline rendering.
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Output sample rate (may differ from project).
    pub sample_rate: u32,
    /// Bit depth for output file.
    pub bit_depth: BitDepth,
    /// Output format.
    pub format: OutputFormat,
    /// Start position in beats.
    pub start: Beats,
    /// End position in beats.
    pub end: Beats,
    /// Whether to normalize output.
    pub normalize: bool,
    /// Whether to dither when reducing bit depth.
    pub dither: bool,
    /// Render buffer size (larger = faster but more memory).
    pub buffer_size: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            bit_depth: BitDepth::Bit16,
            format: OutputFormat::Wav,
            start: Beats(0.0),
            end: Beats(16.0),
            normalize: false,
            dither: true,
            buffer_size: 4096,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    Bit16,
    Bit24,
    Bit32Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Wav,
    // Future: Mp3, Flac, Ogg
}

/// Progress callback type.
pub type ProgressCallback = Box<dyn FnMut(RenderProgress) + Send>;

/// Progress information during render.
#[derive(Debug, Clone)]
pub struct RenderProgress {
    /// Current position being rendered.
    pub current_position: Beats,
    /// Total length to render.
    pub total_length: Beats,
    /// Progress as 0.0 - 1.0.
    pub progress: f32,
    /// Estimated time remaining in seconds.
    pub eta_seconds: Option<f32>,
    /// Peak level seen so far.
    pub peak_level: f32,
}

/// Render error.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("Invalid render range: {0:?} to {1:?}")]
    InvalidRange(Beats, Beats),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("Render cancelled")]
    Cancelled,
}

/// Offline renderer.
pub struct OfflineRenderer {
    config: RenderConfig,
    cancel_flag: std::sync::atomic::AtomicBool,
}

impl OfflineRenderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            config,
            cancel_flag: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Cancel an in-progress render.
    pub fn cancel(&self) {
        self.cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Render project to file.
    pub fn render(
        &self,
        project: &Project,
        graph: &mut AudioGraph,
        output_path: &Path,
        progress: Option<ProgressCallback>,
    ) -> Result<RenderStats, RenderError> {
        // Validate range
        if self.config.end.0 <= self.config.start.0 {
            return Err(RenderError::InvalidRange(self.config.start, self.config.end));
        }

        let mut progress_cb = progress;
        let tempo = project.tempo_map.default_tempo();
        let sample_rate = self.config.sample_rate;
        let buffer_size = self.config.buffer_size;

        // Calculate total samples to render
        let start_samples = SampleTime::from_beats(self.config.start, tempo, sample_rate);
        let end_samples = SampleTime::from_beats(self.config.end, tempo, sample_rate);
        let total_samples = end_samples.0 - start_samples.0;

        // Pre-allocate output buffer
        let mut output_samples: Vec<f32> = Vec::with_capacity((total_samples * 2) as usize);
        
        // Create processor
        let mut processor = GraphProcessor::new(64, buffer_size);
        
        // Create transport starting at render start position
        let mut transport = Transport::new(sample_rate)
            .set_tempo(tempo)
            .seek(self.config.start)
            .play();

        let mut current_sample = start_samples.0;
        let mut peak_level: f32 = 0.0;
        let start_time = std::time::Instant::now();

        // Render loop
        while current_sample < end_samples.0 {
            // Check for cancellation
            if self.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return Err(RenderError::Cancelled);
            }

            // Calculate how many samples this iteration
            let remaining = end_samples.0 - current_sample;
            let this_buffer = (remaining as usize).min(buffer_size);

            // Create process context
            let context = ProcessContext {
                buffer_size: this_buffer,
                sample_rate,
                tempo,
                position: SampleTime(current_sample),
                is_playing: true,
            };

            // Process graph
            let stereo_output = processor.process(graph, &context, &[]);

            // Collect samples and track peak
            for i in 0..this_buffer {
                let left = stereo_output.left()[i];
                let right = stereo_output.right()[i];
                output_samples.push(left);
                output_samples.push(right);
                peak_level = peak_level.max(left.abs()).max(right.abs());
            }

            current_sample += this_buffer as u64;

            // Report progress
            if let Some(ref mut cb) = progress_cb {
                let progress_ratio = (current_sample - start_samples.0) as f32 / total_samples as f32;
                let elapsed = start_time.elapsed().as_secs_f32();
                let eta = if progress_ratio > 0.01 {
                    Some((elapsed / progress_ratio) - elapsed)
                } else {
                    None
                };

                cb(RenderProgress {
                    current_position: transport.position_beats(),
                    total_length: Beats(self.config.end.0 - self.config.start.0),
                    progress: progress_ratio,
                    eta_seconds: eta,
                    peak_level,
                });
            }

            transport = transport.advance(this_buffer as u64);
        }

        // Normalize if requested
        if self.config.normalize && peak_level > 0.0 {
            let normalize_gain = 0.99 / peak_level;
            for sample in &mut output_samples {
                *sample *= normalize_gain;
            }
            peak_level = 0.99;
        }

        // Write output file
        self.write_wav_file(output_path, &output_samples, sample_rate)?;

        Ok(RenderStats {
            duration_seconds: total_samples as f32 / sample_rate as f32,
            peak_level,
            sample_rate,
            render_time_seconds: start_time.elapsed().as_secs_f32(),
            output_size_bytes: std::fs::metadata(output_path)
                .map(|m| m.len())
                .unwrap_or(0),
        })
    }

    fn write_wav_file(&self, path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), RenderError> {
        use std::io::BufWriter;
        use std::fs::File;

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate,
            bits_per_sample: match self.config.bit_depth {
                BitDepth::Bit16 => 16,
                BitDepth::Bit24 => 24,
                BitDepth::Bit32Float => 32,
            },
            sample_format: match self.config.bit_depth {
                BitDepth::Bit16 | BitDepth::Bit24 => hound::SampleFormat::Int,
                BitDepth::Bit32Float => hound::SampleFormat::Float,
            },
        };

        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        let mut wav_writer = hound::WavWriter::new(writer, spec)
            .map_err(|e| RenderError::EncodingError(e.to_string()))?;

        match self.config.bit_depth {
            BitDepth::Bit16 => {
                for &sample in samples {
                    let int_sample = (sample * i16::MAX as f32) as i16;
                    wav_writer.write_sample(int_sample)
                        .map_err(|e| RenderError::EncodingError(e.to_string()))?;
                }
            }
            BitDepth::Bit24 => {
                for &sample in samples {
                    let int_sample = (sample * 8388607.0) as i32; // 2^23 - 1
                    wav_writer.write_sample(int_sample)
                        .map_err(|e| RenderError::EncodingError(e.to_string()))?;
                }
            }
            BitDepth::Bit32Float => {
                for &sample in samples {
                    wav_writer.write_sample(sample)
                        .map_err(|e| RenderError::EncodingError(e.to_string()))?;
                }
            }
        }

        wav_writer.finalize()
            .map_err(|e| RenderError::EncodingError(e.to_string()))?;

        Ok(())
    }
}

/// Statistics from a completed render.
#[derive(Debug, Clone)]
pub struct RenderStats {
    pub duration_seconds: f32,
    pub peak_level: f32,
    pub sample_rate: u32,
    pub render_time_seconds: f32,
    pub output_size_bytes: u64,
}

impl RenderStats {
    /// Render speed as multiple of real-time.
    pub fn speed_multiplier(&self) -> f32 {
        self.duration_seconds / self.render_time_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_config_defaults() {
        let config = RenderConfig::default();
        assert_eq!(config.sample_rate, 44100);
        assert_eq!(config.bit_depth, BitDepth::Bit16);
        assert_eq!(config.start, Beats(0.0));
    }

    #[test]
    fn invalid_range_detected() {
        let renderer = OfflineRenderer::new(RenderConfig {
            start: Beats(10.0),
            end: Beats(5.0), // End before start!
            ..Default::default()
        });

        let project = Project::new("Test");
        let mut graph = AudioGraph::new();
        let result = renderer.render(&project, &mut graph, Path::new("test.wav"), None);
        
        assert!(matches!(result, Err(RenderError::InvalidRange(_, _))));
    }

    #[test]
    fn render_stats_speed() {
        let stats = RenderStats {
            duration_seconds: 60.0,
            peak_level: 0.5,
            sample_rate: 44100,
            render_time_seconds: 2.0,
            output_size_bytes: 1000000,
        };
        
        // 60 seconds rendered in 2 seconds = 30x real-time
        assert!((stats.speed_multiplier() - 30.0).abs() < 0.01);
    }
}
