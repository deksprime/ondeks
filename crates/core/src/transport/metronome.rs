use crate::dsp::{Buffer, Sample};
use super::state::Transport;
use super::time::{SampleTime, Beats};

/// Metronome configuration.
#[derive(Debug, Clone)]
pub struct MetronomeConfig {
    pub enabled: bool,
    pub volume: Sample,
    pub count_in_bars: u8,
    pub accent_downbeat: bool,
}

impl Default for MetronomeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            volume: 0.8,
            count_in_bars: 0,
            accent_downbeat: true,
        }
    }
}

/// Generates metronome clicks.
pub struct Metronome {
    config: MetronomeConfig,
    click_high: Vec<Sample>,   // Downbeat click (higher pitch)
    click_low: Vec<Sample>,    // Other beats click (lower pitch)
    playback_position: usize,
    currently_playing: Option<bool>, // true = high, false = low
    last_beat: i64,
}

impl Metronome {
    /// Create a new metronome.
    pub fn new(config: MetronomeConfig, sample_rate: u32) -> Self {
        let click_high = Self::generate_click(sample_rate, 1500.0, 0.02);
        let click_low = Self::generate_click(sample_rate, 1000.0, 0.015);

        Self {
            config,
            click_high,
            click_low,
            playback_position: 0,
            currently_playing: None,
            last_beat: -1,
        }
    }

    /// Generate a click sound (decaying sine burst).
    fn generate_click(sample_rate: u32, frequency: f32, duration: f32) -> Vec<Sample> {
        let num_samples = (sample_rate as f32 * duration) as usize;
        let mut click = Vec::with_capacity(num_samples);

        for i in 0..num_samples {
            let t = i as f32 / sample_rate as f32;
            let envelope = (-t * 50.0).exp(); // Fast decay
            let sine = (2.0 * std::f32::consts::PI * frequency * t).sin();
            click.push(sine * envelope);
        }

        click
    }

    /// Generate metronome audio for one buffer cycle.
    pub fn process(
        &mut self,
        buffer: &mut Buffer,
        transport: &Transport,
        buffer_start_sample: SampleTime,
    ) {
        if !self.config.enabled || !transport.is_playing() {
            return;
        }

        let sample_rate = transport.sample_rate();
        let tempo = transport.tempo();
        let samples_per_beat = (60.0 / tempo * sample_rate as f64) as u64;
        let numerator = transport.time_signature().numerator;

        for i in 0..buffer.len() {
            let current_sample = buffer_start_sample.0 + i as u64;
            let current_beat = (current_sample / samples_per_beat) as i64;

            // Check if we crossed a beat boundary
            if current_beat > self.last_beat {
                self.last_beat = current_beat;
                let beat_in_bar = (current_beat as u8) % numerator;
                
                // Start a new click
                self.playback_position = 0;
                self.currently_playing = Some(
                    self.config.accent_downbeat && beat_in_bar == 0
                );
            }

            // Play click if active
            if let Some(is_high) = self.currently_playing {
                let click = if is_high { &self.click_high } else { &self.click_low };
                
                if self.playback_position < click.len() {
                    buffer[i] += click[self.playback_position] * self.config.volume;
                    self.playback_position += 1;
                } else {
                    self.currently_playing = None;
                }
            }
        }
    }

    /// Set configuration.
    pub fn set_config(&mut self, config: MetronomeConfig) {
        self.config = config;
    }

    /// Get configuration.
    pub fn config(&self) -> &MetronomeConfig {
        &self.config
    }

    /// Reset state (called on transport stop).
    pub fn reset(&mut self) {
        self.playback_position = 0;
        self.currently_playing = None;
        self.last_beat = -1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metronome_silent_when_disabled() {
        let mut metro = Metronome::new(
            MetronomeConfig {
                enabled: false,
                ..Default::default()
            },
            44100,
        );

        let transport = Transport::new(44100).set_tempo(120.0).play();
        let mut buffer = Buffer::allocate(512);
        metro.process(&mut buffer, &transport, SampleTime(0));

        assert_eq!(buffer.peak(), 0.0);
    }

    #[test]
    fn metronome_silent_when_stopped() {
        let mut metro = Metronome::new(MetronomeConfig::default(), 44100);
        let transport = Transport::new(44100); // Stopped
        let mut buffer = Buffer::allocate(512);
        metro.process(&mut buffer, &transport, SampleTime(0));

        assert_eq!(buffer.peak(), 0.0);
    }

    #[test]
    fn metronome_produces_clicks() {
        let mut metro = Metronome::new(MetronomeConfig::default(), 44100);
        let transport = Transport::new(44100).set_tempo(120.0).play();
        let mut buffer = Buffer::allocate(512);
        
        // Process at beat 0
        metro.process(&mut buffer, &transport, SampleTime(0));

        // Should have non-zero samples (the click)
        assert!(buffer.peak() > 0.0);
    }

    #[test]
    fn click_generation() {
        let click = Metronome::generate_click(44100, 1000.0, 0.02);
        
        // Should have samples
        assert!(!click.is_empty());
        
        // Should have non-zero samples (the click sound)
        let peak = click.iter().map(|s| s.abs()).fold(0.0, f32::max);
        assert!(peak > 0.0);
        
        // Should decay overall (envelope decreases)
        // Check that the maximum is in the first half
        let first_half_max = click[..click.len()/2].iter().map(|s| s.abs()).fold(0.0, f32::max);
        let second_half_max = click[click.len()/2..].iter().map(|s| s.abs()).fold(0.0, f32::max);
        assert!(first_half_max >= second_half_max);
    }
}
