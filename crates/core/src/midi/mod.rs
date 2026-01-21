//! MIDI event handling and sequences.

mod types;
mod event;
mod sequence;

pub use types::{Channel, Note, Velocity};
pub use event::{MidiEvent, TimestampedEvent};
pub use sequence::MidiSequence;
