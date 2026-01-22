use super::{Sample, db_to_linear, linear_to_db};

/// A dynamics compressor.
#[derive(Debug, Clone)]
pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_gain_db: f32,
    
    envelope: f32,
    sample_rate: u32,
}

impl Compressor {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            threshold_db: -12.0,
            ratio: 4.0,
            attack_ms: 10.0,
            release_ms: 100.0,
            makeup_gain_db: 0.0,
            envelope: 0.0,
            sample_rate,
        }
    }

    pub fn set_threshold(&mut self, db: f32) {
        self.threshold_db = db;
    }

    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.max(1.0);
    }

    pub fn set_attack(&mut self, ms: f32) {
        self.attack_ms = ms.max(0.1);
    }

    pub fn set_release(&mut self, ms: f32) {
        self.release_ms = ms.max(1.0);
    }

    pub fn set_makeup_gain(&mut self, db: f32) {
        self.makeup_gain_db = db;
    }

    /// Process a single sample.
    pub fn tick(&mut self, input: Sample) -> Sample {
        let input_db = linear_to_db(input.abs());
        
        // Calculate gain reduction
        let over_threshold = input_db - self.threshold_db;
        let target_reduction = if over_threshold > 0.0 {
            over_threshold - over_threshold / self.ratio
        } else {
            0.0
        };

        // Apply envelope
        let attack_coef = (-1.0 / (self.attack_ms * 0.001 * self.sample_rate as f32)).exp();
        let release_coef = (-1.0 / (self.release_ms * 0.001 * self.sample_rate as f32)).exp();

        if target_reduction > self.envelope {
            self.envelope = attack_coef * self.envelope + (1.0 - attack_coef) * target_reduction;
        } else {
            self.envelope = release_coef * self.envelope + (1.0 - release_coef) * target_reduction;
        }

        // Apply gain reduction and makeup
        let gain = db_to_linear(-self.envelope + self.makeup_gain_db);
        input * gain
    }

    /// Process a buffer.
    pub fn process(&mut self, buffer: &mut [Sample]) {
        for sample in buffer.iter_mut() {
            *sample = self.tick(*sample);
        }
    }

    /// Reset envelope.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressor_creation() {
        let comp = Compressor::new(44100);
        assert_eq!(comp.threshold_db, -12.0);
        assert_eq!(comp.ratio, 4.0);
    }

    #[test]
    fn compressor_set_threshold() {
        let mut comp = Compressor::new(44100);
        comp.set_threshold(-6.0);
        assert_eq!(comp.threshold_db, -6.0);
    }

    #[test]
    fn compressor_set_ratio() {
        let mut comp = Compressor::new(44100);
        comp.set_ratio(8.0);
        assert_eq!(comp.ratio, 8.0);
    }

    #[test]
    fn compressor_reset() {
        let mut comp = Compressor::new(44100);
        comp.tick(1.0);
        comp.reset();
        assert_eq!(comp.envelope, 0.0);
    }
}
