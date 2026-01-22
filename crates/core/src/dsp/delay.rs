use super::{Sample, StereoBuffer};

/// A stereo delay effect.
#[derive(Debug)]
pub struct StereoDelay {
    buffer_left: Vec<Sample>,
    buffer_right: Vec<Sample>,
    write_pos: usize,
    delay_samples_left: usize,
    delay_samples_right: usize,
    feedback: f32,
    mix: f32,
    sample_rate: u32,
}

impl StereoDelay {
    pub fn new(sample_rate: u32, max_delay_seconds: f32) -> Self {
        let max_samples = (sample_rate as f32 * max_delay_seconds) as usize;
        Self {
            buffer_left: vec![0.0; max_samples],
            buffer_right: vec![0.0; max_samples],
            write_pos: 0,
            delay_samples_left: sample_rate as usize / 4,
            delay_samples_right: sample_rate as usize / 4,
            feedback: 0.3,
            mix: 0.5,
            sample_rate,
        }
    }

    /// Set delay time in seconds.
    pub fn set_delay_time(&mut self, left: f32, right: f32) {
        let max = self.buffer_left.len();
        self.delay_samples_left = ((left * self.sample_rate as f32) as usize).min(max - 1);
        self.delay_samples_right = ((right * self.sample_rate as f32) as usize).min(max - 1);
    }

    /// Set feedback amount (0.0 - 1.0).
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.99);
    }

    /// Set dry/wet mix (0.0 = dry, 1.0 = wet).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Process stereo audio.
    pub fn process(&mut self, input: &StereoBuffer, output: &mut StereoBuffer) {
        let len = input.len();
        
        for i in 0..len {
            let in_left = input.left().as_slice()[i];
            let in_right = input.right().as_slice()[i];

            // Calculate read positions
            let read_left = (self.write_pos + self.buffer_left.len() - self.delay_samples_left) 
                % self.buffer_left.len();
            let read_right = (self.write_pos + self.buffer_right.len() - self.delay_samples_right) 
                % self.buffer_right.len();

            // Read from delay buffers
            let delayed_left = self.buffer_left[read_left];
            let delayed_right = self.buffer_right[read_right];

            // Write to delay buffers with feedback
            self.buffer_left[self.write_pos] = in_left + delayed_left * self.feedback;
            self.buffer_right[self.write_pos] = in_right + delayed_right * self.feedback;

            // Mix dry and wet
            output.left_mut().as_mut_slice()[i] = in_left * (1.0 - self.mix) + delayed_left * self.mix;
            output.right_mut().as_mut_slice()[i] = in_right * (1.0 - self.mix) + delayed_right * self.mix;

            // Advance write position
            self.write_pos = (self.write_pos + 1) % self.buffer_left.len();
        }
    }

    /// Clear delay buffers.
    pub fn reset(&mut self) {
        self.buffer_left.fill(0.0);
        self.buffer_right.fill(0.0);
        self.write_pos = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_creation() {
        let delay = StereoDelay::new(44100, 1.0);
        assert_eq!(delay.feedback, 0.3);
        assert_eq!(delay.mix, 0.5);
    }

    #[test]
    fn delay_set_delay_time() {
        let mut delay = StereoDelay::new(44100, 1.0);
        delay.set_delay_time(0.1, 0.2);
        // Should be approximately 4410 and 8820 samples
        assert!(delay.delay_samples_left > 4000);
        assert!(delay.delay_samples_right > 8000);
    }

    #[test]
    fn delay_set_feedback() {
        let mut delay = StereoDelay::new(44100, 1.0);
        delay.set_feedback(0.5);
        assert_eq!(delay.feedback, 0.5);
    }

    #[test]
    fn delay_reset() {
        let mut delay = StereoDelay::new(44100, 1.0);
        delay.reset();
        assert_eq!(delay.write_pos, 0);
    }
}
