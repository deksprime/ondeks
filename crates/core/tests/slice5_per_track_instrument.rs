//! Slice 5: per-track instrument nodes.
//!
//! Exercises the engine's graph-mutation commands (`AddSynthNode`,
//! `RemoveSynthNode`) and the invariant that multiple synth nodes coexist
//! independently — sending MIDI to one doesn't leak into the other.

use ondeks_core::Command;
use ondeks_core::dsp::StereoBuffer;
use ondeks_core::{Engine, NodeId, TrackId};
use ondeks_core::midi::{Channel, MidiEvent, Note, Velocity};

const SR: u32 = 44100;
const BLOCK: usize = 256;

fn note_on(n: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        channel: Channel::new(0).unwrap(),
        note: Note::new(n).unwrap(),
        velocity: Velocity::new(100).unwrap(),
    }
}

fn add_synth(engine: &mut Engine) -> NodeId {
    let node_id = NodeId::generate();
    engine.apply_command(Command::AddSynthNode {
        node_id,
        track_id: TrackId::generate(),
    });
    node_id
}

#[test]
fn default_engine_has_empty_graph() {
    let engine = Engine::new(SR, BLOCK);
    // Graph should contain only the master output node.
    let graph = engine.graph();
    let count = graph.node_ids().count();
    assert_eq!(count, 1, "fresh engine should only have the master output, got {count}");
}

#[test]
fn add_synth_node_produces_audio_when_driven() {
    let mut engine = Engine::new(SR, BLOCK);
    let synth = add_synth(&mut engine);

    engine.apply_command(Command::SendMidi {
        target: synth,
        event: note_on(60),
        sample_offset: 0,
    });

    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert!(out.left().peak() > 0.01, "new synth should produce audio");
}

#[test]
fn remove_synth_node_silences_output() {
    let mut engine = Engine::new(SR, BLOCK);
    let synth = add_synth(&mut engine);

    engine.apply_command(Command::SendMidi {
        target: synth,
        event: note_on(60),
        sample_offset: 0,
    });
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert!(out.left().peak() > 0.01);

    engine.apply_command(Command::RemoveSynthNode { node_id: synth });
    out.silence();
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0, "removed node should not produce audio");
}

#[test]
fn two_synths_are_independent() {
    let mut engine = Engine::new(SR, BLOCK);
    let a = add_synth(&mut engine);
    let b = add_synth(&mut engine);
    assert_ne!(a, b);

    // MIDI to A only; B should stay silent (but sum into the master output).
    engine.apply_command(Command::SendMidi {
        target: a,
        event: note_on(60),
        sample_offset: 0,
    });
    let mut only_a = StereoBuffer::allocate(BLOCK);
    engine.process(&mut only_a, BLOCK as u32);
    let peak_a_only = only_a.left().peak();
    assert!(peak_a_only > 0.01);

    // Adding MIDI to B should increase the total signal (two voices summing).
    engine.apply_command(Command::SendMidi {
        target: b,
        event: note_on(64),
        sample_offset: 0,
    });
    let mut both = StereoBuffer::allocate(BLOCK);
    engine.process(&mut both, BLOCK as u32);
    assert!(
        both.left().rms() > only_a.left().rms(),
        "two active synths should sum louder than one (a_only_rms={}, both_rms={})",
        only_a.left().rms(),
        both.left().rms(),
    );
}

#[test]
fn add_synth_node_with_duplicate_id_is_noop() {
    let mut engine = Engine::new(SR, BLOCK);
    let id = NodeId::generate();
    engine.apply_command(Command::AddSynthNode { node_id: id, track_id: TrackId::generate() });
    // Adding again with the same id must not panic and must keep the node.
    engine.apply_command(Command::AddSynthNode { node_id: id, track_id: TrackId::generate() });

    engine.apply_command(Command::SendMidi {
        target: id,
        event: note_on(60),
        sample_offset: 0,
    });
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert!(out.left().peak() > 0.01);
}

#[test]
fn remove_unknown_synth_node_is_noop() {
    let mut engine = Engine::new(SR, BLOCK);
    let bogus = NodeId::from_raw(u64::MAX);
    // Should not panic.
    engine.apply_command(Command::RemoveSynthNode { node_id: bogus });
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0);
}
