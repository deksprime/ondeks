//! Commands that can be sent to the audio engine.

use crate::ids::{NodeId, TrackId};
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
    /// Insert a polyphonic `SynthNode` into the graph using the given id,
    /// wire its mono output to both master input ports.
    ///
    /// `track_id` is informational for logging/debugging — the engine keys by
    /// `node_id`. Callers (typically the UI dispatcher) pre-allocate the id
    /// on the project's `Track::instrument` so node identity is stable across
    /// undo/redo cycles.
    AddSynthNode {
        /// Id assigned to the new synth node. Must not already exist.
        node_id: NodeId,
        /// Track this synth belongs to (for logging/debug; engine ignores).
        track_id: TrackId,
    },
    /// Disconnect and remove a synth node from the graph.
    ///
    /// No-op if the node does not exist (undo-of-remove is allowed to arrive
    /// out of order during rapid-undo sequences).
    RemoveSynthNode {
        /// Id of the node to remove.
        node_id: NodeId,
    },
}
