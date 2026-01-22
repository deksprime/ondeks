use super::Sample;

/// ADSR envelope stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// ADSR envelope generator.
#[derive(Debug, Clone)]
pub struct AdsrEnvelope {
    attack_time: f32,   // seconds
    decay_time: f32,    // seconds
    sustain_level: f32, // 0.0 - 1.0
    release_time: f32,  // seconds
    
    stage: EnvelopeStage,
    level: f32,
    sample_rate: u32,
    samples_in_stage: u32,
}

impl AdsrEnvelope {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            attack_time: 0.01,
            decay_time: 0.1,
            sustain_level: 0.7,
            release_time: 0.3,
            stage: EnvelopeStage::Idle,
            level: 0.0,
            sample_rate,
            samples_in_stage: 0,
        }
    }

    pub fn set_attack(&mut self, time: f32) {
        self.attack_time = time.max(0.001);
    }

    pub fn set_decay(&mut self, time: f32) {
        self.decay_time = time.max(0.001);
    }

    pub fn set_sustain(&mut self, level: f32) {
        self.sustain_level = level.clamp(0.0, 1.0);
    }

    pub fn set_release(&mut self, time: f32) {
        self.release_time = time.max(0.001);
    }

    /// Trigger the envelope (note on).
    pub fn trigger(&mut self) {
        self.stage = EnvelopeStage::Attack;
        self.samples_in_stage = 0;
    }

    /// Release the envelope (note off).
    pub fn release(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.stage = EnvelopeStage::Release;
            self.samples_in_stage = 0;
        }
    }

    /// Get the next envelope value.
    pub fn tick(&mut self) -> Sample {
        let attack_samples = (self.attack_time * self.sample_rate as f32) as u32;
        let decay_samples = (self.decay_time * self.sample_rate as f32) as u32;
        let release_samples = (self.release_time * self.sample_rate as f32) as u32;

        match self.stage {
            EnvelopeStage::Idle => {
                self.level = 0.0;
            }
            EnvelopeStage::Attack => {
                self.level = self.samples_in_stage as f32 / attack_samples as f32;
                self.samples_in_stage += 1;
                if self.samples_in_stage >= attack_samples {
                    self.stage = EnvelopeStage::Decay;
                    self.samples_in_stage = 0;
                    self.level = 1.0;
                }
            }
            EnvelopeStage::Decay => {
                let progress = self.samples_in_stage as f32 / decay_samples as f32;
                self.level = 1.0 - progress * (1.0 - self.sustain_level);
                self.samples_in_stage += 1;
                if self.samples_in_stage >= decay_samples {
                    self.stage = EnvelopeStage::Sustain;
                    self.level = self.sustain_level;
                }
            }
            EnvelopeStage::Sustain => {
                self.level = self.sustain_level;
            }
            EnvelopeStage::Release => {
                let start_level = self.sustain_level;
                let progress = self.samples_in_stage as f32 / release_samples as f32;
                self.level = start_level * (1.0 - progress);
                self.samples_in_stage += 1;
                if self.samples_in_stage >= release_samples {
                    self.stage = EnvelopeStage::Idle;
                    self.level = 0.0;
                }
            }
        }

        self.level
    }

    /// Check if envelope is active.
    pub fn is_active(&self) -> bool {
        self.stage != EnvelopeStage::Idle
    }

    /// Get current stage.
    pub fn stage(&self) -> EnvelopeStage {
        self.stage
    }

    /// Reset to idle.
    pub fn reset(&mut self) {
        self.stage = EnvelopeStage::Idle;
        self.level = 0.0;
        self.samples_in_stage = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_creation() {
        let env = AdsrEnvelope::new(44100);
        assert_eq!(env.stage(), EnvelopeStage::Idle);
        assert!(!env.is_active());
    }

    #[test]
    fn envelope_trigger() {
        let mut env = AdsrEnvelope::new(44100);
        env.trigger();
        assert_eq!(env.stage(), EnvelopeStage::Attack);
        assert!(env.is_active());
    }

    #[test]
    fn envelope_release() {
        let mut env = AdsrEnvelope::new(44100);
        env.trigger();
        env.release();
        assert_eq!(env.stage(), EnvelopeStage::Release);
    }

    #[test]
    fn envelope_attack_phase() {
        let mut env = AdsrEnvelope::new(44100);
        env.set_attack(0.01); // 10ms
        env.trigger();
        
        let first = env.tick();
        // First tick should be >= 0 (could be 0 if attack is very short)
        assert!(first >= 0.0);
        assert!(first <= 1.0);
        
        // After a few ticks, should definitely be > 0
        let mut value = first;
        for _ in 0..10 {
            value = env.tick();
        }
        assert!(value > 0.0);
    }

    #[test]
    fn envelope_reset() {
        let mut env = AdsrEnvelope::new(44100);
        env.trigger();
        env.reset();
        assert_eq!(env.stage(), EnvelopeStage::Idle);
        assert!(!env.is_active());
    }
}
