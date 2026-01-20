/// Pulses per quarter note (PPQN), used for tick resolution.
/// 960 is standard for high-resolution MIDI.
pub const PPQN: u16 = 960;

/// Time measured in samples (absolute, depends on sample rate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SampleTime(pub u64);

impl SampleTime {
    /// Create from seconds and sample rate.
    pub fn from_seconds(seconds: Seconds, sample_rate: u32) -> Self {
        Self((seconds.0 * sample_rate as f64) as u64)
    }

    /// Convert to seconds.
    pub fn to_seconds(self, sample_rate: u32) -> Seconds {
        Seconds(self.0 as f64 / sample_rate as f64)
    }

    /// Create from beats, tempo, and sample rate.
    pub fn from_beats(beats: Beats, tempo_bpm: f64, sample_rate: u32) -> Self {
        let seconds = Seconds::from_beats(beats, tempo_bpm);
        Self::from_seconds(seconds, sample_rate)
    }

    /// Convert to beats.
    pub fn to_beats(self, tempo_bpm: f64, sample_rate: u32) -> Beats {
        let seconds = self.to_seconds(sample_rate);
        seconds.to_beats(tempo_bpm)
    }
}

impl std::ops::Add for SampleTime {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for SampleTime {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// Time measured in seconds (absolute, independent of sample rate).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Seconds(pub f64);

impl Seconds {
    /// Create from beats and tempo.
    pub fn from_beats(beats: Beats, tempo_bpm: f64) -> Self {
        // beats / (beats_per_minute / 60) = beats * 60 / bpm
        Self(beats.0 * 60.0 / tempo_bpm)
    }

    /// Convert to beats.
    pub fn to_beats(self, tempo_bpm: f64) -> Beats {
        Beats(self.0 * tempo_bpm / 60.0)
    }
}

/// Time measured in musical beats (depends on tempo).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct Beats(pub f64);

impl std::fmt::Display for Beats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Beats {
    /// Convert to bar:beat:tick representation.
    pub fn to_bar_beat_tick(self, numerator: u8, denominator: u8) -> BarBeatTick {
        // Adjust for time signature denominator
        // In 4/4, 1 beat = 1 quarter note
        // In 6/8, 1 beat = 1 eighth note, so 2 beats = 1 quarter
        let quarter_notes = self.0;
        
        let total_beats = quarter_notes / (4.0 / denominator as f64);
        let bar = (total_beats / numerator as f64).floor() as u32 + 1; // 1-indexed
        let beat_in_bar = (total_beats % numerator as f64).floor() as u8 + 1; // 1-indexed
        let beat_fraction = total_beats.fract();
        let tick = (beat_fraction * PPQN as f64) as u16;
        
        BarBeatTick { bar, beat: beat_in_bar, tick }
    }

    /// Create from bar:beat:tick representation.
    pub fn from_bar_beat_tick(bbt: BarBeatTick, numerator: u8, denominator: u8) -> Self {
        let beats_per_bar = numerator as f64;
        let beat_to_quarter = 4.0 / denominator as f64;
        
        let total_beats = (bbt.bar - 1) as f64 * beats_per_bar 
            + (bbt.beat - 1) as f64 
            + bbt.tick as f64 / PPQN as f64;
        
        Beats(total_beats * beat_to_quarter)
    }
}

impl std::ops::Add for Beats {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Beats {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self((self.0 - rhs.0).max(0.0))
    }
}

/// Time measured in bars:beats:ticks (musical, human-readable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarBeatTick {
    /// Bar number (1-indexed).
    pub bar: u32,
    /// Beat within bar (1-indexed, up to time signature numerator).
    pub beat: u8,
    /// Tick within beat (0-959 for 960 PPQN).
    pub tick: u16,
}

impl std::fmt::Display for BarBeatTick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}.{:03}", self.bar, self.beat, self.tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_time_to_seconds() {
        let samples = SampleTime(44100);
        let seconds = samples.to_seconds(44100);
        assert!((seconds.0 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn seconds_to_sample_time() {
        let seconds = Seconds(2.5);
        let samples = SampleTime::from_seconds(seconds, 48000);
        assert_eq!(samples.0, 120000);
    }

    #[test]
    fn beats_to_seconds_at_120bpm() {
        let beats = Beats(2.0);
        let seconds = Seconds::from_beats(beats, 120.0);
        // 2 beats at 120 BPM = 1 second
        assert!((seconds.0 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn seconds_to_beats_at_60bpm() {
        let seconds = Seconds(2.0);
        let beats = seconds.to_beats(60.0);
        // 2 seconds at 60 BPM = 2 beats
        assert!((beats.0 - 2.0).abs() < 1e-9);
    }

    #[test]
    fn sample_time_to_beats_chain() {
        let sample_rate = 44100;
        let tempo = 120.0;
        
        let samples = SampleTime(44100); // 1 second
        let beats = samples.to_beats(tempo, sample_rate);
        // 1 second at 120 BPM = 2 beats
        assert!((beats.0 - 2.0).abs() < 0.001);
    }

    #[test]
    fn beats_to_bar_beat_tick_4_4_simple() {
        // Beat 0 = Bar 1, Beat 1
        let bbt = Beats(0.0).to_bar_beat_tick(4, 4);
        assert_eq!(bbt.bar, 1);
        assert_eq!(bbt.beat, 1);
        assert_eq!(bbt.tick, 0);
    }

    #[test]
    fn beats_to_bar_beat_tick_4_4_bar_2() {
        // 4 quarter notes = bar 2, beat 1
        let bbt = Beats(4.0).to_bar_beat_tick(4, 4);
        assert_eq!(bbt.bar, 2);
        assert_eq!(bbt.beat, 1);
        assert_eq!(bbt.tick, 0);
    }

    #[test]
    fn beats_to_bar_beat_tick_4_4_with_tick() {
        // 5.5 quarter notes in 4/4
        let bbt = Beats(5.5).to_bar_beat_tick(4, 4);
        assert_eq!(bbt.bar, 2);
        assert_eq!(bbt.beat, 2);
        assert_eq!(bbt.tick, 480); // Half of 960
    }

    #[test]
    fn bar_beat_tick_roundtrip() {
        let original = Beats(7.25);
        let bbt = original.to_bar_beat_tick(4, 4);
        let roundtrip = Beats::from_bar_beat_tick(bbt, 4, 4);
        assert!((original.0 - roundtrip.0).abs() < 0.01);
    }

    #[test]
    fn bar_beat_tick_display() {
        let bbt = BarBeatTick { bar: 5, beat: 3, tick: 240 };
        assert_eq!(format!("{}", bbt), "5:3.240");
    }

    #[test]
    fn sample_time_arithmetic() {
        let a = SampleTime(1000);
        let b = SampleTime(500);
        assert_eq!((a + b).0, 1500);
        assert_eq!((a - b).0, 500);
        assert_eq!((b - a).0, 0); // Saturating
    }

    #[test]
    fn beats_arithmetic() {
        let a = Beats(4.0);
        let b = Beats(1.5);
        assert!((a + b).0 - 5.5 < 1e-9);
        assert!((a - b).0 - 2.5 < 1e-9);
    }
}
