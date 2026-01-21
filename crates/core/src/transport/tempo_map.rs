use super::time::{Beats, SampleTime};

/// A tempo change at a specific point in time.
#[derive(Debug, Clone)]
pub struct TempoEvent {
    /// Position in beats where this tempo change occurs.
    pub position: Beats,
    /// Tempo in BPM at this position.
    pub tempo: f64,
    /// Interpolation curve to the next tempo change.
    pub curve: TempoCurve,
}

/// The interpolation curve to the next tempo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TempoCurve {
    /// Jump instantly at the next point.
    #[default]
    Instant,
    /// Linear interpolation (gradual ramp).
    Linear,
}

/// Maps time to tempo, supporting tempo changes.
#[derive(Debug, Clone)]
pub struct TempoMap {
    events: Vec<TempoEvent>,
    default_tempo: f64,
}

impl TempoMap {
    /// Create a tempo map with constant tempo.
    pub fn constant(tempo: f64) -> Self {
        Self {
            events: Vec::new(),
            default_tempo: tempo,
        }
    }

    /// Get tempo at a specific beat position.
    pub fn tempo_at(&self, position: Beats) -> f64 {
        if self.events.is_empty() {
            return self.default_tempo;
        }

        // Find the event at or before this position
        let mut current_tempo = self.default_tempo;
        let mut last_event: Option<&TempoEvent> = None;

        for event in &self.events {
            if event.position.0 > position.0 {
                // This event is in the future
                if let Some(last) = last_event {
                    if last.curve == TempoCurve::Linear {
                        // Interpolate
                        let progress = (position.0 - last.position.0) 
                            / (event.position.0 - last.position.0);
                        return last.tempo + (event.tempo - last.tempo) * progress;
                    }
                }
                break;
            }
            current_tempo = event.tempo;
            last_event = Some(event);
        }

        current_tempo
    }

    /// Convert beats to samples given a sample rate.
    /// Handles tempo changes by integrating through the tempo map.
    pub fn beats_to_samples(&self, beats: Beats, sample_rate: u32) -> SampleTime {
        if self.events.is_empty() {
            return SampleTime::from_beats(beats, self.default_tempo, sample_rate);
        }

        // Integrate through tempo events
        let mut total_samples: f64 = 0.0;
        let mut current_beat = 0.0;
        let mut current_tempo = self.default_tempo;

        for event in &self.events {
            if event.position.0 >= beats.0 {
                break;
            }

            let segment_beats = event.position.0 - current_beat;
            let segment_seconds = segment_beats * 60.0 / current_tempo;
            total_samples += segment_seconds * sample_rate as f64;

            current_beat = event.position.0;
            current_tempo = event.tempo;
        }

        // Add remaining beats at current tempo
        let remaining_beats = beats.0 - current_beat;
        let remaining_seconds = remaining_beats * 60.0 / current_tempo;
        total_samples += remaining_seconds * sample_rate as f64;

        SampleTime(total_samples as u64)
    }

    /// Add a tempo change event.
    pub fn add_event(&mut self, event: TempoEvent) {
        // Insert in sorted order
        let pos = self.events
            .iter()
            .position(|e| e.position.0 > event.position.0)
            .unwrap_or(self.events.len());
        self.events.insert(pos, event);
    }

    /// Remove event at position.
    pub fn remove_event(&mut self, position: Beats) {
        self.events.retain(|e| (e.position.0 - position.0).abs() > 0.001);
    }

    /// Get all tempo events.
    pub fn events(&self) -> &[TempoEvent] {
        &self.events
    }

    /// Get the default/initial tempo.
    pub fn default_tempo(&self) -> f64 {
        self.default_tempo
    }

    /// Set the default tempo.
    pub fn set_default_tempo(&mut self, tempo: f64) {
        self.default_tempo = tempo;
    }
}

impl Default for TempoMap {
    fn default() -> Self {
        Self::constant(120.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_tempo_map() {
        let map = TempoMap::constant(120.0);
        assert_eq!(map.tempo_at(Beats(0.0)), 120.0);
        assert_eq!(map.tempo_at(Beats(100.0)), 120.0);
    }

    #[test]
    fn tempo_change_instant() {
        let mut map = TempoMap::constant(120.0);
        map.add_event(TempoEvent {
            position: Beats(8.0),
            tempo: 140.0,
            curve: TempoCurve::Instant,
        });

        assert_eq!(map.tempo_at(Beats(7.99)), 120.0);
        assert_eq!(map.tempo_at(Beats(8.0)), 140.0);
        assert_eq!(map.tempo_at(Beats(100.0)), 140.0);
    }

    #[test]
    fn multiple_tempo_events() {
        let mut map = TempoMap::constant(100.0);
        map.add_event(TempoEvent {
            position: Beats(4.0),
            tempo: 120.0,
            curve: TempoCurve::Instant,
        });
        map.add_event(TempoEvent {
            position: Beats(8.0),
            tempo: 140.0,
            curve: TempoCurve::Instant,
        });

        assert_eq!(map.tempo_at(Beats(0.0)), 100.0);
        assert_eq!(map.tempo_at(Beats(4.0)), 120.0);
        assert_eq!(map.tempo_at(Beats(8.0)), 140.0);
    }

    #[test]
    fn beats_to_samples_constant() {
        let map = TempoMap::constant(120.0);
        let samples = map.beats_to_samples(Beats(4.0), 44100);
        // 4 beats at 120 BPM = 2 seconds = 88200 samples
        assert_eq!(samples.0, 88200);
    }

    #[test]
    fn beats_to_samples_with_change() {
        let mut map = TempoMap::constant(120.0);
        map.add_event(TempoEvent {
            position: Beats(4.0),
            tempo: 60.0,
            curve: TempoCurve::Instant,
        });

        let samples = map.beats_to_samples(Beats(8.0), 44100);
        // First 4 beats at 120 BPM = 2 seconds
        // Next 4 beats at 60 BPM = 4 seconds
        // Total = 6 seconds = 264600 samples
        assert_eq!(samples.0, 264600);
    }
}
