//! Slice 1 + 2 + 3 (+ 5): verify that audio flows through the engine graph.
//!
//! These tests pin the end-to-end invariant: `Engine::process()` routes node
//! output through the topological order into the stereo master. Slice 5
//! removed the default demo graph — the engine now starts with just the
//! master output. Tests construct a synth node explicitly via
//! `Command::AddSynthNode` before driving MIDI.

use ondeks_core::Command;
use ondeks_core::dsp::StereoBuffer;
use ondeks_core::{Engine, NodeId, TrackId};
use ondeks_core::midi::{Channel, MidiEvent, Note, Velocity};

const SR: u32 = 44100;
const BLOCK: usize = 512;

fn new_engine_with_synth() -> (Engine, NodeId) {
    let mut engine = Engine::new(SR, BLOCK);
    let synth_node_id = NodeId::generate();
    let strip_node_id = NodeId::generate();
    engine.apply_command(Command::AddInstrumentChannel {
        synth_node_id,
        strip_node_id,
        track_id: TrackId::generate(),
    });
    (engine, synth_node_id)
}

fn note_on(n: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        channel: Channel::new(0).unwrap(),
        note: Note::new(n).unwrap(),
        velocity: Velocity::new(100).unwrap(),
    }
}

fn note_off(n: u8) -> MidiEvent {
    MidiEvent::note_off(Channel::new(0).unwrap(), Note::new(n).unwrap())
}

#[test]
fn empty_graph_is_silent() {
    // Engine::new now creates an empty graph (just master). No synth until
    // the UI sends AddSynthNode.
    let mut engine = Engine::new(SR, BLOCK);
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0);
    assert_eq!(out.right().peak(), 0.0);
}

#[test]
fn silence_with_no_midi() {
    let (mut engine, _) = new_engine_with_synth();
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0, "silent synth without MIDI");
    assert_eq!(out.right().peak(), 0.0);
}

#[test]
fn playing_without_midi_is_still_silent() {
    let (mut engine, _) = new_engine_with_synth();
    engine.apply_command(Command::Play);
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0);
}

#[test]
fn note_on_produces_stereo_audio() {
    let (mut engine, target) = new_engine_with_synth();
    engine.apply_command(Command::SendMidi {
        target,
        event: note_on(60),
        sample_offset: 0,
    });

    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);

    let lp = out.left().peak();
    let rp = out.right().peak();

    assert!(lp > 0.01, "left peak should be non-zero, got {lp}");
    assert!(rp > 0.01, "right peak should be non-zero, got {rp}");
    assert!((lp - rp).abs() < 1e-6, "L and R should match (mono routed to both)");
}

#[test]
fn note_off_decays_over_time() {
    let (mut engine, target) = new_engine_with_synth();
    let mut out = StereoBuffer::allocate(BLOCK);

    engine.apply_command(Command::SendMidi { target, event: note_on(60), sample_offset: 0 });
    engine.process(&mut out, BLOCK as u32);
    let peak_with_note = out.left().peak();
    assert!(peak_with_note > 0.01);

    engine.apply_command(Command::SendMidi { target, event: note_off(60), sample_offset: 0 });
    for _ in 0..40 {
        out.silence();
        engine.process(&mut out, BLOCK as u32);
    }
    let peak_after_release = out.left().peak();
    assert!(
        peak_after_release < peak_with_note * 0.5,
        "release should reduce amplitude substantially: before={peak_with_note}, after={peak_after_release}"
    );
}

#[test]
fn transport_advances_only_when_playing() {
    let (mut engine, _) = new_engine_with_synth();
    let mut out = StereoBuffer::allocate(BLOCK);

    let pos_before = engine.transport().position().0;

    engine.process(&mut out, BLOCK as u32);
    assert_eq!(engine.transport().position().0, pos_before);

    engine.apply_command(Command::Play);
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(engine.transport().position().0, pos_before + BLOCK as u64);
}

#[test]
fn held_note_is_continuous_across_blocks() {
    let (mut engine, target) = new_engine_with_synth();
    engine.apply_command(Command::SendMidi { target, event: note_on(60), sample_offset: 0 });

    let mut out_a = StereoBuffer::allocate(BLOCK);
    let mut out_b = StereoBuffer::allocate(BLOCK);

    engine.process(&mut out_a, BLOCK as u32);
    engine.process(&mut out_b, BLOCK as u32);

    assert!(out_a.left().rms() > 0.01);
    assert!(out_b.left().rms() > 0.01);

    let last_a = out_a.left()[BLOCK - 1];
    let first_b = out_b.left()[0];
    assert!(
        (last_a - first_b).abs() < 0.2,
        "block boundary discontinuity: last_a={last_a}, first_b={first_b}"
    );
}
