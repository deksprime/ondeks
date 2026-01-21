//! MIDI event handling.
//!
//! This module contains types for representing MIDI events.

/// A MIDI event.
///
/// This is a minimal stub for Phase 2. Full implementation will come in Phase 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiEvent {
    /// MIDI channel (0-15)
    pub channel: u8,
    /// MIDI message type
    pub message: MidiMessage,
}

/// MIDI message types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiMessage {
    /// Note on: (note, velocity)
    NoteOn {
        /// MIDI note number (0-127).
        note: u8,
        /// Note velocity (0-127).
        velocity: u8,
    },
    /// Note off: (note, velocity)
    NoteOff {
        /// MIDI note number (0-127).
        note: u8,
        /// Note velocity (0-127).
        velocity: u8,
    },
    /// Control change: (controller, value)
    ControlChange {
        /// Controller number (0-127).
        controller: u8,
        /// Controller value (0-127).
        value: u8,
    },
}

impl MidiEvent {
    /// Create a note on event.
    pub fn note_on(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            channel,
            message: MidiMessage::NoteOn { note, velocity },
        }
    }

    /// Create a note off event.
    pub fn note_off(channel: u8, note: u8, velocity: u8) -> Self {
        Self {
            channel,
            message: MidiMessage::NoteOff { note, velocity },
        }
    }

    /// Create a control change event.
    pub fn control_change(channel: u8, controller: u8, value: u8) -> Self {
        Self {
            channel,
            message: MidiMessage::ControlChange { controller, value },
        }
    }
}
