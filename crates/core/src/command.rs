//! Commands that can be sent to the audio engine.

use crate::ids::NodeId;
use crate::midi::MidiEvent;

/// Commands that control the audio engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Start playback.
    Play,
    /// Stop playback.
    Stop,
    /// Set the tempo in BPM.
    SetTempo(f64),
    /// Deliver a MIDI event to a specific node's MIDI inbox.
    ///
    /// `sample_offset` is the offset within the current audio block at which
    /// the event should take effect (P0.2, sample-accurate event timing). For
    /// UI-originated events where sub-block precision is meaningless, use 0.
    SendMidi {
        /// The node to receive the event.
        target: NodeId,
        /// The MIDI event to deliver.
        event: MidiEvent,
        /// Sample offset within the current block (0 = block start).
        sample_offset: u32,
    },
}
