use crate::dsp::{Sample, StereoBuffer, db_to_linear};
use crate::ids::TrackId;

/// Peak meter state.
#[derive(Debug, Clone, Default)]
pub struct MeterState {
    pub peak_left: Sample,
    pub peak_right: Sample,
    pub rms_left: Sample,
    pub rms_right: Sample,
}

/// A channel strip with volume, pan, and metering.
#[derive(Debug, Clone)]
pub struct ChannelStrip {
    pub track_id: TrackId,
    pub volume_db: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    meters: MeterState,
}

impl ChannelStrip {
    pub fn new(track_id: TrackId) -> Self {
        Self {
            track_id,
            volume_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            meters: MeterState::default(),
        }
    }

    /// Process audio through the channel strip.
    pub fn process(&mut self, input: &StereoBuffer, output: &mut StereoBuffer, solo_active: bool) {
        let gain = self.effective_gain(solo_active);

        output.copy_from(input);
        output.apply_gain(gain);
        output.apply_pan(self.pan);

        // Update meters
        self.meters.peak_left = output.left().peak();
        self.meters.peak_right = output.right().peak();
        self.meters.rms_left = output.left().rms();
        self.meters.rms_right = output.right().rms();
    }

    /// Calculate effective gain considering mute/solo.
    pub fn effective_gain(&self, solo_active: bool) -> Sample {
        // If any track is soloed and this isn't, mute it
        if solo_active && !self.soloed {
            return 0.0;
        }
        if self.muted {
            return 0.0;
        }
        db_to_linear(self.volume_db)
    }

    /// Get current meter readings.
    pub fn meters(&self) -> &MeterState {
        &self.meters
    }

    /// Reset meters (call periodically to implement decay).
    pub fn decay_meters(&mut self, decay: f32) {
        self.meters.peak_left *= decay;
        self.meters.peak_right *= decay;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_channel_strip_defaults() {
        let track_id = TrackId::generate();
        let strip = ChannelStrip::new(track_id);
        
        assert_eq!(strip.track_id, track_id);
        assert_eq!(strip.volume_db, 0.0);
        assert_eq!(strip.pan, 0.0);
        assert!(!strip.muted);
        assert!(!strip.soloed);
    }

    #[test]
    fn effective_gain_unity() {
        let track_id = TrackId::generate();
        let strip = ChannelStrip::new(track_id);
        
        // No solo active, not muted -> should be unity gain (0dB = 1.0)
        assert!((strip.effective_gain(false) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn effective_gain_muted() {
        let track_id = TrackId::generate();
        let mut strip = ChannelStrip::new(track_id);
        strip.muted = true;
        
        assert_eq!(strip.effective_gain(false), 0.0);
    }

    #[test]
    fn effective_gain_solo_active_not_soloed() {
        let track_id = TrackId::generate();
        let strip = ChannelStrip::new(track_id);
        
        // Solo active but this track isn't soloed -> muted
        assert_eq!(strip.effective_gain(true), 0.0);
    }

    #[test]
    fn effective_gain_soloed() {
        let track_id = TrackId::generate();
        let mut strip = ChannelStrip::new(track_id);
        strip.soloed = true;
        
        // Solo active and this track is soloed -> should have gain
        assert!((strip.effective_gain(true) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn process_applies_gain_and_pan() {
        let track_id = TrackId::generate();
        let mut strip = ChannelStrip::new(track_id);
        strip.volume_db = -6.0; // -6dB = ~0.5 gain
        strip.pan = -1.0; // Full left
        
        let input = {
            let mut buf = StereoBuffer::allocate(4);
            buf.left_mut().fill(1.0);
            buf.right_mut().fill(1.0);
            buf
        };
        let mut output = StereoBuffer::allocate(4);
        
        strip.process(&input, &mut output, false);
        
        // Should be panned left and gain applied
        assert!(output.left()[0] > 0.0);
        assert!(output.right()[0].abs() < 0.01); // Right should be near zero
    }

    #[test]
    fn meters_update_on_process() {
        let track_id = TrackId::generate();
        let mut strip = ChannelStrip::new(track_id);
        
        let input = {
            let mut buf = StereoBuffer::allocate(4);
            buf.left_mut().fill(0.5);
            buf.right_mut().fill(0.3);
            buf
        };
        let mut output = StereoBuffer::allocate(4);
        
        strip.process(&input, &mut output, false);
        
        let meters = strip.meters();
        assert!(meters.peak_left > 0.0);
        assert!(meters.peak_right > 0.0);
        assert!(meters.rms_left > 0.0);
        assert!(meters.rms_right > 0.0);
    }

    #[test]
    fn decay_meters() {
        let track_id = TrackId::generate();
        let mut strip = ChannelStrip::new(track_id);
        
        // Set some meter values
        strip.meters.peak_left = 1.0;
        strip.meters.peak_right = 0.5;
        
        strip.decay_meters(0.5);
        
        assert_eq!(strip.meters.peak_left, 0.5);
        assert_eq!(strip.meters.peak_right, 0.25);
    }
}
