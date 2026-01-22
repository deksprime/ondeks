use super::Sample;
use std::f32::consts::PI;

/// Waveform type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Waveform {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
    Noise,
}

/// A basic oscillator.
#[derive(Debug, Clone)]
pub struct Oscillator {
    waveform: Waveform,
    frequency: f32,
    phase: f32,
    sample_rate: u32,
    phase_increment: f32,
}

impl Oscillator {
    pub fn new(sample_rate: u32) -> Self {
        let mut osc = Self {
            waveform: Waveform::Sine,
            frequency: 440.0,
            phase: 0.0,
            sample_rate,
            phase_increment: 0.0,
        };
        osc.update_increment();
        osc
    }

    pub fn with_waveform(mut self, waveform: Waveform) -> Self {
        self.waveform = waveform;
        self
    }

    pub fn set_frequency(&mut self, frequency: f32) {
        self.frequency = frequency;
        self.update_increment();
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
    }

    fn update_increment(&mut self) {
        self.phase_increment = self.frequency / self.sample_rate as f32;
    }

    /// Generate the next sample.
    pub fn tick(&mut self) -> Sample {
        let sample = match self.waveform {
            Waveform::Sine => (self.phase * 2.0 * PI).sin(),
            Waveform::Triangle => {
                let p = self.phase;
                if p < 0.25 {
                    p * 4.0
                } else if p < 0.75 {
                    2.0 - p * 4.0
                } else {
                    p * 4.0 - 4.0
                }
            }
            Waveform::Saw => 2.0 * self.phase - 1.0,
            Waveform::Square => {
                if self.phase < 0.5 { 1.0 } else { -1.0 }
            }
            Waveform::Noise => fastrand::f32() * 2.0 - 1.0,
        };

        self.phase += self.phase_increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }

    /// Fill a buffer with oscillator output.
    pub fn process(&mut self, buffer: &mut [Sample]) {
        for sample in buffer.iter_mut() {
            *sample = self.tick();
        }
    }

    /// Reset phase to zero.
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oscillator_creation() {
        let osc = Oscillator::new(44100);
        assert_eq!(osc.frequency, 440.0);
        assert_eq!(osc.waveform, Waveform::Sine);
    }

    #[test]
    fn oscillator_set_frequency() {
        let mut osc = Oscillator::new(44100);
        osc.set_frequency(880.0);
        assert_eq!(osc.frequency, 880.0);
    }

    #[test]
    fn oscillator_sine_wave() {
        let mut osc = Oscillator::new(44100);
        osc.set_waveform(Waveform::Sine);
        osc.set_frequency(440.0);
        osc.reset();
        
        let sample = osc.tick();
        // First sample should be close to 0 (sin(0) = 0)
        assert!(sample.abs() < 0.1);
    }

    #[test]
    fn oscillator_square_wave() {
        let mut osc = Oscillator::new(44100);
        osc.set_waveform(Waveform::Square);
        osc.reset();
        
        let sample = osc.tick();
        // Square wave starts at 1.0 (phase < 0.5)
        assert_eq!(sample, 1.0);
    }

    #[test]
    fn oscillator_reset() {
        let mut osc = Oscillator::new(44100);
        osc.tick();
        osc.tick();
        osc.reset();
        assert_eq!(osc.phase, 0.0);
    }
}
