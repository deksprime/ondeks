//! Transport view model.

use ondeks_core::transport::{Transport, TransportState, BarBeatTick};

/// UI-friendly transport state.
#[derive(Debug, Clone)]
pub struct TransportViewModel {
    /// Current transport state
    pub state: TransportState,
    /// Is currently playing
    pub is_playing: bool,
    /// Is currently recording
    pub is_recording: bool,
    /// Current tempo in BPM
    pub tempo: f64,
    /// Current position in beats
    pub position_beats: f64,
    /// Current position formatted as bar:beat:tick
    pub position_bbt: BarBeatTick,
    /// Current position in seconds
    pub position_seconds: f64,
    /// Time signature numerator
    pub time_sig_numerator: u8,
    /// Time signature denominator
    pub time_sig_denominator: u8,
    /// Loop enabled
    pub loop_enabled: bool,
    /// Loop start in beats
    pub loop_start: f64,
    /// Loop end in beats
    pub loop_end: f64,
    /// Metronome enabled
    pub metronome_enabled: bool,
}

impl TransportViewModel {
    /// Create from core transport state.
    pub fn from_transport(transport: &Transport, metronome_enabled: bool) -> Self {
        let ts = transport.time_signature();
        let loop_region = transport.loop_region();

        Self {
            state: transport.state(),
            is_playing: transport.is_playing(),
            is_recording: transport.is_recording(),
            tempo: transport.tempo(),
            position_beats: transport.position_beats().0,
            position_bbt: transport.position_bbt(),
            position_seconds: transport.position_seconds().0,
            time_sig_numerator: ts.numerator,
            time_sig_denominator: ts.denominator,
            loop_enabled: loop_region.enabled,
            loop_start: loop_region.start.0,
            loop_end: loop_region.end.0,
            metronome_enabled,
        }
    }

    /// Format position as "BAR.BEAT.TICK" string.
    pub fn position_string(&self) -> String {
        format!(
            "{}.{}.{:03}",
            self.position_bbt.bar,
            self.position_bbt.beat,
            self.position_bbt.tick
        )
    }

    /// Format position as "MM:SS.mmm" string.
    pub fn time_string(&self) -> String {
        let total_seconds = self.position_seconds;
        let minutes = (total_seconds / 60.0).floor() as u32;
        let seconds = total_seconds % 60.0;
        format!("{:02}:{:05.2}", minutes, seconds)
    }

    /// Format tempo as string with one decimal.
    pub fn tempo_string(&self) -> String {
        format!("{:.1} BPM", self.tempo)
    }

    /// Format time signature as string.
    pub fn time_sig_string(&self) -> String {
        format!("{}/{}", self.time_sig_numerator, self.time_sig_denominator)
    }

    /// Get loop region display string.
    pub fn loop_string(&self) -> Option<String> {
        if self.loop_enabled {
            Some(format!("{:.1} - {:.1}", self.loop_start, self.loop_end))
        } else {
            None
        }
    }
}

/// Meter readings for display.
#[derive(Debug, Clone, Default)]
pub struct MeterViewModel {
    /// Left channel peak (0.0 to 1.0+)
    pub left_peak: f32,
    /// Right channel peak (0.0 to 1.0+)
    pub right_peak: f32,
    /// Left channel RMS (0.0 to 1.0)
    pub left_rms: f32,
    /// Right channel RMS (0.0 to 1.0)
    pub right_rms: f32,
    /// Left channel peak in dB
    pub left_db: f32,
    /// Right channel peak in dB
    pub right_db: f32,
    /// Is clipping (either channel > 1.0)
    pub is_clipping: bool,
}

impl MeterViewModel {
    /// Create from linear peak values.
    pub fn from_peaks(left: f32, right: f32) -> Self {
        Self {
            left_peak: left,
            right_peak: right,
            left_rms: 0.0, // TODO: Add RMS tracking
            right_rms: 0.0,
            left_db: linear_to_db(left),
            right_db: linear_to_db(right),
            is_clipping: left > 1.0 || right > 1.0,
        }
    }

    /// Update with new peaks (with decay).
    pub fn update(&mut self, left: f32, right: f32, decay: f32) {
        // Peak hold with decay
        self.left_peak = if left > self.left_peak {
            left
        } else {
            self.left_peak * decay
        };
        self.right_peak = if right > self.right_peak {
            right
        } else {
            self.right_peak * decay
        };

        self.left_db = linear_to_db(self.left_peak);
        self.right_db = linear_to_db(self.right_peak);
        self.is_clipping = self.is_clipping || left > 1.0 || right > 1.0;
    }

    /// Reset clipping indicator.
    pub fn reset_clip(&mut self) {
        self.is_clipping = false;
    }

    /// Get normalized value for meter display (0.0 to 1.0, where 1.0 = 0dB).
    pub fn left_normalized(&self) -> f32 {
        db_to_meter_position(self.left_db)
    }

    pub fn right_normalized(&self) -> f32 {
        db_to_meter_position(self.right_db)
    }
}

/// Convert linear amplitude to dB.
fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        -100.0
    } else {
        20.0 * linear.log10()
    }
}

/// Convert dB to meter position (0.0 to 1.0+).
/// Maps -60dB to 0.0, 0dB to 1.0, +6dB to ~1.1
fn db_to_meter_position(db: f32) -> f32 {
    // Use a scale where -60dB = 0, 0dB = 1.0
    ((db + 60.0) / 60.0).max(0.0)
}
