use super::Sample;
use std::f32::consts::PI;

/// Filter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterType {
    #[default]
    LowPass,
    HighPass,
    BandPass,
    Notch,
}

/// State variable filter (SVF).
#[derive(Debug, Clone)]
pub struct SvFilter {
    filter_type: FilterType,
    cutoff: f32,
    resonance: f32,
    sample_rate: u32,
    
    // State
    low: f32,
    band: f32,
    high: f32,
    notch: f32,
    
    // Coefficients
    f: f32,
    q: f32,
}

impl SvFilter {
    pub fn new(sample_rate: u32) -> Self {
        let mut filter = Self {
            filter_type: FilterType::LowPass,
            cutoff: 1000.0,
            resonance: 0.5,
            sample_rate,
            low: 0.0,
            band: 0.0,
            high: 0.0,
            notch: 0.0,
            f: 0.0,
            q: 0.0,
        };
        filter.update_coefficients();
        filter
    }

    pub fn set_cutoff(&mut self, freq: f32) {
        self.cutoff = freq.clamp(20.0, self.sample_rate as f32 * 0.45);
        self.update_coefficients();
    }

    pub fn set_resonance(&mut self, res: f32) {
        self.resonance = res.clamp(0.0, 1.0);
        self.update_coefficients();
    }

    pub fn set_type(&mut self, filter_type: FilterType) {
        self.filter_type = filter_type;
    }

    fn update_coefficients(&mut self) {
        self.f = 2.0 * (PI * self.cutoff / self.sample_rate as f32).sin();
        self.q = 1.0 - self.resonance * 0.99; // Avoid division by zero
    }

    /// Process a single sample.
    pub fn tick(&mut self, input: Sample) -> Sample {
        // Two-pass for stability
        for _ in 0..2 {
            self.low += self.f * self.band;
            self.high = input - self.low - self.q * self.band;
            self.band += self.f * self.high;
            self.notch = self.high + self.low;
        }

        match self.filter_type {
            FilterType::LowPass => self.low,
            FilterType::HighPass => self.high,
            FilterType::BandPass => self.band,
            FilterType::Notch => self.notch,
        }
    }

    /// Process a buffer.
    pub fn process(&mut self, buffer: &mut [Sample]) {
        for sample in buffer.iter_mut() {
            *sample = self.tick(*sample);
        }
    }

    /// Reset filter state.
    pub fn reset(&mut self) {
        self.low = 0.0;
        self.band = 0.0;
        self.high = 0.0;
        self.notch = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_creation() {
        let filter = SvFilter::new(44100);
        assert_eq!(filter.filter_type, FilterType::LowPass);
        assert_eq!(filter.cutoff, 1000.0);
    }

    #[test]
    fn filter_set_cutoff() {
        let mut filter = SvFilter::new(44100);
        filter.set_cutoff(2000.0);
        assert_eq!(filter.cutoff, 2000.0);
    }

    #[test]
    fn filter_set_type() {
        let mut filter = SvFilter::new(44100);
        filter.set_type(FilterType::HighPass);
        assert_eq!(filter.filter_type, FilterType::HighPass);
    }

    #[test]
    fn filter_reset() {
        let mut filter = SvFilter::new(44100);
        filter.tick(1.0);
        filter.reset();
        assert_eq!(filter.low, 0.0);
        assert_eq!(filter.band, 0.0);
    }
}
