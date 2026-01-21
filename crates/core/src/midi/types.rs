use crate::error::MidiError;

/// MIDI channel (0-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Channel(u8);

impl Channel {
    /// Create a new MIDI channel.
    ///
    /// # Errors
    /// Returns `MidiError::InvalidChannel` if the value is greater than 15.
    pub fn new(value: u8) -> Result<Self, MidiError> {
        if value > 15 {
            return Err(MidiError::InvalidChannel(value));
        }
        Ok(Self(value))
    }

    /// Get the raw channel value (0-15).
    pub fn raw(self) -> u8 {
        self.0
    }
}

/// MIDI note number (0-127, 60 = Middle C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Note(u8);

impl Note {
    /// Middle C (MIDI note 60).
    pub const MIDDLE_C: Self = Self(60);

    /// Create a new MIDI note.
    ///
    /// # Errors
    /// Returns `MidiError::InvalidNote` if the value is greater than 127.
    pub fn new(value: u8) -> Result<Self, MidiError> {
        if value > 127 {
            return Err(MidiError::InvalidNote(value));
        }
        Ok(Self(value))
    }

    /// Get the raw note value (0-127).
    pub fn raw(self) -> u8 {
        self.0
    }

    /// Get note name (e.g., "C4", "F#5").
    pub fn name(self) -> String {
        const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
        let octave = (self.0 / 12) as i32 - 1;
        let note = self.0 % 12;
        format!("{}{}", NAMES[note as usize], octave)
    }

    /// Get frequency in Hz (A4 = 440 Hz).
    pub fn frequency(self) -> f32 {
        440.0 * 2.0_f32.powf((self.0 as f32 - 69.0) / 12.0)
    }
}

/// MIDI velocity (0-127).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Velocity(u8);

impl Velocity {
    /// Zero velocity (note off).
    pub const OFF: Self = Self(0);
    /// Maximum velocity (127).
    pub const MAX: Self = Self(127);

    /// Create a new MIDI velocity.
    ///
    /// # Errors
    /// Returns `MidiError::InvalidVelocity` if the value is greater than 127.
    pub fn new(value: u8) -> Result<Self, MidiError> {
        if value > 127 {
            return Err(MidiError::InvalidVelocity(value));
        }
        Ok(Self(value))
    }

    /// Get the raw velocity value (0-127).
    pub fn raw(self) -> u8 {
        self.0
    }

    /// Convert to 0.0-1.0 range.
    pub fn normalized(self) -> f32 {
        self.0 as f32 / 127.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_validation() {
        assert!(Channel::new(0).is_ok());
        assert!(Channel::new(15).is_ok());
        assert!(Channel::new(16).is_err());
    }

    #[test]
    fn note_names() {
        assert_eq!(Note::MIDDLE_C.name(), "C4");
        assert_eq!(Note::new(69).unwrap().name(), "A4");
    }

    #[test]
    fn note_frequency() {
        let a4 = Note::new(69).unwrap();
        assert!((a4.frequency() - 440.0).abs() < 0.01);
    }

    #[test]
    fn velocity_normalized() {
        assert_eq!(Velocity::OFF.normalized(), 0.0);
        assert!((Velocity::MAX.normalized() - 1.0).abs() < 0.01);
    }
}
