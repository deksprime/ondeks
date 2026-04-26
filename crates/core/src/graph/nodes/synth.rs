//! Polyphonic synthesizer node wrapping [`SimpleSynth`].
//!
//! Unlike [`OscillatorNode`](super::OscillatorNode), `SynthNode` is a real
//! instrument — it responds to MIDI whether the transport is rolling or not,
//! so musicians can noodle live.
//!
//! MIDI events arrive via [`AudioNode::handle_midi`]; each event is buffered
//! with its in-block sample offset and applied at that offset during the next
//! `process()` call, giving sample-accurate note timing (P0.2).

use crate::dsp::SimpleSynth;
use crate::ids::NodeId;
use crate::graph::{AudioNode, ProcessContext, NodeInputs, NodeOutputs};
use crate::graph::port::{InputPort, OutputPort};
use crate::midi::MidiEvent;

/// Fixed capacity of the MIDI inbox. Excess events within a single block are
/// dropped (rt-safe: no allocation). 128 is comfortably above anything a user
/// can generate via UI or a reasonable MIDI stream in a ~10 ms block.
const MIDI_INBOX_CAPACITY: usize = 128;

/// A synth voice bank as an audio graph node. Single mono output.
pub struct SynthNode {
    id: NodeId,
    synth: SimpleSynth,
    /// Pre-allocated inbox: (sample_offset, event). Never grows past capacity.
    inbox: Vec<(u32, MidiEvent)>,
    outputs: Vec<OutputPort>,
}

impl SynthNode {
    /// Create a new synth node with the given sample rate.
    pub fn new(sample_rate: u32) -> Self {
        Self::with_id(NodeId::generate(), sample_rate)
    }

    /// Create a synth node using a caller-supplied id.
    ///
    /// Lets the UI dispatcher pre-allocate the node id on the `Track` so the
    /// graph node and the project's `Track::instrument` share identity — the
    /// same determinism trick used for `TrackId` across undo/redo cycles.
    pub fn with_id(id: NodeId, sample_rate: u32) -> Self {
        Self {
            id,
            synth: SimpleSynth::new(sample_rate),
            inbox: Vec::with_capacity(MIDI_INBOX_CAPACITY),
            outputs: vec![OutputPort::audio("out")],
        }
    }

    /// Get this node's unique identifier.
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Access the underlying [`SimpleSynth`] for parameter tweaks.
    pub fn synth_mut(&mut self) -> &mut SimpleSynth {
        &mut self.synth
    }
}

impl AudioNode for SynthNode {
    fn name(&self) -> &str {
        "Synth"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, context: &ProcessContext, _inputs: NodeInputs, mut outputs: NodeOutputs) {
        let Some(output) = outputs.audio_mut(0) else {
            self.inbox.clear();
            return;
        };

        let frames = context.buffer_size.min(output.len());
        let out_slice = &mut output.as_mut_slice()[..frames];
        out_slice.fill(0.0);

        if self.inbox.is_empty() {
            // Fast path: no events, render straight through.
            self.synth.process(out_slice);
            return;
        }

        // Sample-accurate dispatch: sort events by offset, render between them.
        // `sort_unstable_by_key` on an owned slice does not allocate.
        self.inbox.sort_unstable_by_key(|(offset, _)| *offset);

        let mut cursor: usize = 0;
        // Iterate without cloning events.
        let event_count = self.inbox.len();
        for i in 0..event_count {
            let offset = self.inbox[i].0 as usize;
            let offset = offset.min(frames);
            if offset > cursor {
                self.synth.process(&mut out_slice[cursor..offset]);
                cursor = offset;
            }
            // Apply the event at this point.
            self.synth.handle_midi(&self.inbox[i].1);
        }
        if cursor < frames {
            self.synth.process(&mut out_slice[cursor..frames]);
        }

        self.inbox.clear();
    }

    fn reset(&mut self) {
        self.synth.reset();
        self.inbox.clear();
    }

    fn handle_midi(&mut self, event: &MidiEvent, sample_offset: u32) {
        // RT-safe: drop silently if the inbox is at capacity rather than
        // allocate. With capacity 128 this is effectively unreachable for
        // UI-driven input, but the contract holds under pathological load.
        if self.inbox.len() >= MIDI_INBOX_CAPACITY {
            return;
        }
        self.inbox.push((sample_offset, event.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Buffer;
    use crate::midi::{Channel, Note, Velocity};

    fn ctx(frames: usize) -> ProcessContext {
        ProcessContext::new(frames, 44100, 120.0)
    }

    fn note_on(note: u8) -> MidiEvent {
        MidiEvent::NoteOn {
            channel: Channel::new(0).unwrap(),
            note: Note::new(note).unwrap(),
            velocity: Velocity::new(100).unwrap(),
        }
    }

    fn note_off(note: u8) -> MidiEvent {
        MidiEvent::note_off(Channel::new(0).unwrap(), Note::new(note).unwrap())
    }

    fn run_block(node: &mut SynthNode, frames: usize) -> Buffer {
        let mut output = Buffer::allocate(frames);
        let inputs = NodeInputs { audio: &[], midi: &[], controls: &[] };
        let output_ref: &mut Buffer = &mut output;
        let outputs = NodeOutputs { audio: &mut [output_ref], midi: &mut vec![] };
        node.process(&ctx(frames), inputs, outputs);
        output
    }

    #[test]
    fn silence_with_no_events() {
        let mut node = SynthNode::new(44100);
        let out = run_block(&mut node, 256);
        assert_eq!(out.peak(), 0.0, "no notes should produce silence");
    }

    #[test]
    fn note_on_produces_audio() {
        let mut node = SynthNode::new(44100);
        node.handle_midi(&note_on(60), 0);
        let out = run_block(&mut node, 512);
        assert!(out.peak() > 0.001, "expected audible output, peak={}", out.peak());
    }

    #[test]
    fn note_off_triggers_release() {
        let mut node = SynthNode::new(44100);
        node.handle_midi(&note_on(60), 0);
        // Let the amp envelope progress.
        let _ = run_block(&mut node, 512);
        node.handle_midi(&note_off(60), 0);
        // Keep processing; release tail should still be audible briefly then decay.
        let during_release = run_block(&mut node, 512);
        assert!(during_release.peak() > 0.0, "release tail should still be audible");
    }

    #[test]
    fn sample_accurate_offset_changes_output() {
        // With the note on at offset 0 vs. near end of block, a block-length
        // window captures meaningfully different amplitudes (the later event
        // has less time to ramp through the ADSR attack).
        let mut early = SynthNode::new(44100);
        early.handle_midi(&note_on(60), 0);
        let early_out = run_block(&mut early, 512);

        let mut late = SynthNode::new(44100);
        late.handle_midi(&note_on(60), 500);
        let late_out = run_block(&mut late, 512);

        assert!(
            early_out.rms() > late_out.rms(),
            "early trigger should accumulate more energy over the block (early={}, late={})",
            early_out.rms(),
            late_out.rms(),
        );
    }

    #[test]
    fn inbox_overflow_drops_silently() {
        let mut node = SynthNode::new(44100);
        // Shove MIDI_INBOX_CAPACITY + 50 events in; must not panic or reallocate
        // past capacity. We can't observe allocation here, but we can observe
        // that the node still functions and reports a bounded inbox size.
        for i in 0..(MIDI_INBOX_CAPACITY + 50) {
            node.handle_midi(&note_on(((i % 60) + 36) as u8), 0);
        }
        assert_eq!(node.inbox.len(), MIDI_INBOX_CAPACITY);
        let out = run_block(&mut node, 128);
        assert!(out.peak() > 0.0);
    }

    #[test]
    fn reset_clears_inbox() {
        let mut node = SynthNode::new(44100);
        node.handle_midi(&note_on(60), 0);
        node.reset();
        assert!(node.inbox.is_empty());
    }
}
