use super::time::{SampleTime, Seconds, Beats, BarBeatTick};
use super::time_signature::TimeSignature;

/// The current state of the transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
    Recording,
}

/// Configuration for loop playback.
#[derive(Debug, Clone, Copy)]
pub struct LoopRegion {
    pub start: Beats,
    pub end: Beats,
    pub enabled: bool,
}

impl Default for LoopRegion {
    fn default() -> Self {
        Self {
            start: Beats(0.0),
            end: Beats(4.0),
            enabled: false,
        }
    }
}

/// Complete transport state.
/// 
/// This struct is immutable - all "mutation" methods return a new Transport.
#[derive(Debug, Clone)]
pub struct Transport {
    state: TransportState,
    position: SampleTime,
    tempo: f64,
    time_signature: TimeSignature,
    sample_rate: u32,
    loop_region: LoopRegion,
}

impl Transport {
    /// Create a new transport at position 0, stopped.
    pub fn new(sample_rate: u32) -> Self {
        Self {
            state: TransportState::Stopped,
            position: SampleTime(0),
            tempo: 120.0,
            time_signature: TimeSignature::FOUR_FOUR,
            sample_rate,
            loop_region: LoopRegion::default(),
        }
    }

    // --- State Queries ---

    pub fn state(&self) -> TransportState {
        self.state
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.state, TransportState::Playing | TransportState::Recording)
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.state, TransportState::Recording)
    }

    // --- Position ---

    pub fn position(&self) -> SampleTime {
        self.position
    }

    pub fn position_beats(&self) -> Beats {
        self.position.to_beats(self.tempo, self.sample_rate)
    }

    pub fn position_seconds(&self) -> Seconds {
        self.position.to_seconds(self.sample_rate)
    }

    pub fn position_bbt(&self) -> BarBeatTick {
        self.position_beats().to_bar_beat_tick(
            self.time_signature.numerator,
            self.time_signature.denominator,
        )
    }

    // --- Tempo and Time ---

    pub fn tempo(&self) -> f64 {
        self.tempo
    }

    pub fn time_signature(&self) -> TimeSignature {
        self.time_signature
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    // --- Loop ---

    pub fn loop_region(&self) -> &LoopRegion {
        &self.loop_region
    }

    pub fn is_looping(&self) -> bool {
        self.loop_region.enabled
    }

    // --- Commands (return new state) ---

    /// Start playback from current position.
    pub fn play(&self) -> Self {
        Self {
            state: TransportState::Playing,
            ..self.clone()
        }
    }

    /// Stop playback and return to position 0.
    pub fn stop(&self) -> Self {
        Self {
            state: TransportState::Stopped,
            position: SampleTime(0),
            ..self.clone()
        }
    }

    /// Start recording from current position.
    pub fn record(&self) -> Self {
        Self {
            state: TransportState::Recording,
            ..self.clone()
        }
    }

    /// Pause playback (keep position).
    pub fn pause(&self) -> Self {
        Self {
            state: TransportState::Stopped,
            ..self.clone()
        }
    }

    /// Seek to a position in beats.
    pub fn seek(&self, position: Beats) -> Self {
        let sample_pos = SampleTime::from_beats(position, self.tempo, self.sample_rate);
        Self {
            position: sample_pos,
            ..self.clone()
        }
    }

    /// Seek to a position in samples.
    pub fn seek_samples(&self, position: SampleTime) -> Self {
        Self {
            position,
            ..self.clone()
        }
    }

    /// Set tempo in BPM.
    pub fn set_tempo(&self, bpm: f64) -> Self {
        Self {
            tempo: bpm.max(20.0).min(999.0), // Reasonable limits
            ..self.clone()
        }
    }

    /// Set time signature.
    pub fn set_time_signature(&self, ts: TimeSignature) -> Self {
        Self {
            time_signature: ts,
            ..self.clone()
        }
    }

    /// Set loop region.
    pub fn set_loop(&self, region: LoopRegion) -> Self {
        Self {
            loop_region: region,
            ..self.clone()
        }
    }

    /// Enable or disable looping.
    pub fn set_loop_enabled(&self, enabled: bool) -> Self {
        Self {
            loop_region: LoopRegion {
                enabled,
                ..self.loop_region
            },
            ..self.clone()
        }
    }

    /// Advance position by N samples (called each process cycle).
    pub fn advance(&self, samples: u64) -> Self {
        if !self.is_playing() {
            return self.clone();
        }

        let new_pos = SampleTime(self.position.0 + samples);
        
        // Handle looping
        if self.loop_region.enabled {
            let loop_end_samples = SampleTime::from_beats(
                self.loop_region.end,
                self.tempo,
                self.sample_rate,
            );
            let loop_start_samples = SampleTime::from_beats(
                self.loop_region.start,
                self.tempo,
                self.sample_rate,
            );

            if new_pos.0 >= loop_end_samples.0 {
                let loop_length = loop_end_samples.0 - loop_start_samples.0;
                let overshoot = new_pos.0 - loop_end_samples.0;
                let wrapped_pos = loop_start_samples.0 + (overshoot % loop_length);
                return Self {
                    position: SampleTime(wrapped_pos),
                    ..self.clone()
                };
            }
        }

        Self {
            position: new_pos,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_starts_stopped() {
        let transport = Transport::new(44100);
        assert_eq!(transport.state(), TransportState::Stopped);
        assert_eq!(transport.position().0, 0);
    }

    #[test]
    fn play_changes_state() {
        let transport = Transport::new(44100).play();
        assert!(transport.is_playing());
        assert_eq!(transport.state(), TransportState::Playing);
    }

    #[test]
    fn stop_returns_to_zero() {
        let transport = Transport::new(44100)
            .play()
            .advance(44100) // 1 second
            .stop();
        assert_eq!(transport.position().0, 0);
        assert!(!transport.is_playing());
    }

    #[test]
    fn pause_keeps_position() {
        let transport = Transport::new(44100)
            .play()
            .advance(44100)
            .pause();
        assert_eq!(transport.position().0, 44100);
        assert!(!transport.is_playing());
    }

    #[test]
    fn advance_updates_position() {
        let transport = Transport::new(44100).play().advance(22050);
        assert_eq!(transport.position().0, 22050);
    }

    #[test]
    fn advance_does_nothing_when_stopped() {
        let transport = Transport::new(44100).advance(22050);
        assert_eq!(transport.position().0, 0);
    }

    #[test]
    fn seek_sets_position() {
        let transport = Transport::new(44100)
            .set_tempo(120.0)
            .seek(Beats(4.0));
        
        // 4 beats at 120 BPM = 2 seconds = 88200 samples
        assert_eq!(transport.position().0, 88200);
    }

    #[test]
    fn loop_region_wraps_position() {
        let transport = Transport::new(44100)
            .set_tempo(120.0)
            .set_loop(LoopRegion {
                start: Beats(0.0),
                end: Beats(4.0),
                enabled: true,
            })
            .play()
            .seek(Beats(3.9));
        
        // Advance past loop end
        let after = transport.advance(44100); // Way past loop end
        
        assert!(after.position_beats().0 < 4.0);
        assert!(after.position_beats().0 >= 0.0);
    }

    #[test]
    fn tempo_has_limits() {
        let slow = Transport::new(44100).set_tempo(1.0);
        assert!(slow.tempo() >= 20.0);

        let fast = Transport::new(44100).set_tempo(5000.0);
        assert!(fast.tempo() <= 999.0);
    }

    #[test]
    fn position_bbt_correct() {
        let transport = Transport::new(44100)
            .set_tempo(120.0)
            .seek(Beats(5.5)); // Bar 2, Beat 2, 50%
        
        let bbt = transport.position_bbt();
        assert_eq!(bbt.bar, 2);
        assert_eq!(bbt.beat, 2);
        assert!(bbt.tick > 400 && bbt.tick < 500);
    }
}
