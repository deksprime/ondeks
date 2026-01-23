use ondeks_core::dsp::Sample;

/// Sample rate conversion quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResampleQuality {
    /// Fast but lower quality (linear interpolation).
    Draft,
    /// Balanced quality/speed.
    #[default]
    Normal,
    /// High quality (for final export).
    High,
}

/// A simple linear interpolation resampler.
/// For production, you'd want a proper polyphase filter resampler.
pub struct Resampler {
    quality: ResampleQuality,
    ratio: f64,
    /// Fractional position in source.
    position: f64,
}

impl Resampler {
    pub fn new(source_rate: u32, target_rate: u32, quality: ResampleQuality) -> Self {
        Self {
            quality,
            ratio: source_rate as f64 / target_rate as f64,
            position: 0.0,
        }
    }

    /// Resample audio.
    /// Returns the number of output samples written.
    pub fn process(&mut self, input: &[Sample], output: &mut [Sample]) -> usize {
        if (self.ratio - 1.0).abs() < 0.0001 {
            // No resampling needed
            let copy_len = input.len().min(output.len());
            output[..copy_len].copy_from_slice(&input[..copy_len]);
            return copy_len;
        }

        let input_len = input.len();
        let mut output_pos = 0;

        while output_pos < output.len() {
            let int_pos = self.position as usize;
            
            if int_pos + 1 >= input_len {
                break;
            }

            // Linear interpolation
            let frac = (self.position - int_pos as f64) as f32;
            let sample = match self.quality {
                ResampleQuality::Draft => {
                    // Nearest neighbor
                    input[int_pos]
                }
                ResampleQuality::Normal | ResampleQuality::High => {
                    // Linear interpolation
                    let a = input[int_pos];
                    let b = input[int_pos + 1];
                    a + (b - a) * frac
                }
            };

            output[output_pos] = sample;
            output_pos += 1;
            self.position += self.ratio;
        }

        // Reset position for next call, keeping fractional part
        self.position -= (self.position as usize) as f64;

        output_pos
    }

    /// Reset resampler state.
    pub fn reset(&mut self) {
        self.position = 0.0;
    }

    /// Calculate output size needed for given input size.
    pub fn output_size_for(&self, input_samples: usize) -> usize {
        ((input_samples as f64 / self.ratio).ceil()) as usize
    }

    /// Calculate input size needed for given output size.
    pub fn input_size_for(&self, output_samples: usize) -> usize {
        ((output_samples as f64 * self.ratio).ceil()) as usize + 1
    }
}

/// Resample an entire buffer (convenience function).
pub fn resample_buffer(
    input: &[Sample],
    source_rate: u32,
    target_rate: u32,
    quality: ResampleQuality,
) -> Vec<Sample> {
    let mut resampler = Resampler::new(source_rate, target_rate, quality);
    let output_size = resampler.output_size_for(input.len());
    let mut output = vec![0.0; output_size];
    resampler.process(input, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_resample_when_same_rate() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let mut output = vec![0.0; 4];
        let mut resampler = Resampler::new(44100, 44100, ResampleQuality::Normal);
        let written = resampler.process(&input, &mut output);
        
        assert_eq!(written, 4);
        assert_eq!(output, input);
    }

    #[test]
    fn downsample_2x() {
        let input = vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
        let mut output = vec![0.0; 3];
        let mut resampler = Resampler::new(48000, 24000, ResampleQuality::Normal);
        let written = resampler.process(&input, &mut output);
        
        assert!(written > 0);
        // First sample should be 0.0
        assert!((output[0] - 0.0_f32).abs() < 0.01);
    }

    #[test]
    fn upsample_2x() {
        let input = vec![0.0, 1.0];
        let mut output = vec![0.0; 4];
        let mut resampler = Resampler::new(22050, 44100, ResampleQuality::Normal);
        let written = resampler.process(&input, &mut output);
        
        assert!(written > 0);
        // Should interpolate between 0 and 1
        assert!(output[1] > 0.0 && output[1] < 1.0);
    }

    #[test]
    fn output_size_calculation() {
        let resampler = Resampler::new(48000, 44100, ResampleQuality::Normal);
        let output_size = resampler.output_size_for(48000);
        // Should be close to 44100
        assert!(output_size > 44000 && output_size < 44200);
    }
}
