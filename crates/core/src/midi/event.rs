use super::types::{Channel, Note, Velocity};
use crate::transport::Beats;

/// A MIDI event.
#[derive(Debug, Clone, PartialEq)]
pub enum MidiEvent {
    /// Note on event.
    NoteOn {
        /// MIDI channel (0-15).
        channel: Channel,
        /// MIDI note number (0-127).
        note: Note,
        /// Note velocity (0-127).
        velocity: Velocity,
    },
    /// Note off event.
    NoteOff {
        /// MIDI channel (0-15).
        channel: Channel,
        /// MIDI note number (0-127).
        note: Note,
        /// Note velocity (0-127), typically 0 for note off.
        velocity: Velocity,
    },
    /// Control change event.
    ControlChange {
        /// MIDI channel (0-15).
        channel: Channel,
        /// Controller number (0-127).
        controller: u8,
        /// Controller value (0-127).
        value: u8,
    },
    /// Program change event.
    ProgramChange {
        /// MIDI channel (0-15).
        channel: Channel,
        /// Program number (0-127).
        program: u8,
    },
    /// Pitch bend event.
    PitchBend {
        /// MIDI channel (0-15).
        channel: Channel,
        /// Pitch bend value: -8192 to 8191 (0 = center).
        value: i16,
    },
    /// Channel aftertouch (pressure) event.
    Aftertouch {
        /// MIDI channel (0-15).
        channel: Channel,
        /// Pressure value (0-127).
        pressure: u8,
    },
    /// Polyphonic aftertouch (per-note pressure) event.
    PolyAftertouch {
        /// MIDI channel (0-15).
        channel: Channel,
        /// MIDI note number (0-127).
        note: Note,
        /// Pressure value (0-127).
        pressure: u8,
    },
}

impl MidiEvent {
    /// Create a note on event.
    pub fn note_on(channel: Channel, note: Note, velocity: Velocity) -> Self {
        Self::NoteOn { channel, note, velocity }
    }

    /// Create a note off event.
    pub fn note_off(channel: Channel, note: Note) -> Self {
        Self::NoteOff { 
            channel, 
            note, 
            velocity: Velocity::OFF 
        }
    }

    /// Get the MIDI channel for this event.
    pub fn channel(&self) -> Channel {
        match self {
            Self::NoteOn { channel, .. } => *channel,
            Self::NoteOff { channel, .. } => *channel,
            Self::ControlChange { channel, .. } => *channel,
            Self::ProgramChange { channel, .. } => *channel,
            Self::PitchBend { channel, .. } => *channel,
            Self::Aftertouch { channel, .. } => *channel,
            Self::PolyAftertouch { channel, .. } => *channel,
        }
    }
}

/// A MIDI event with a timestamp.
#[derive(Debug, Clone)]
pub struct TimestampedEvent {
    /// Time position in beats.
    pub time: Beats,
    /// The MIDI event.
    pub event: MidiEvent,
}

impl TimestampedEvent {
    /// Create a new timestamped MIDI event.
    pub fn new(time: Beats, event: MidiEvent) -> Self {
        Self { time, event }
    }
}
