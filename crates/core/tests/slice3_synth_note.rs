//! Slice 3: MIDI keyboard → SynthNode integration.
//!
//! Verifies the command → engine → node MIDI routing path end-to-end:
//! `Command::SendMidi` delivered to the engine must reach the target node's
//! `handle_midi` inbox and produce audio in the next `process()` call.

use ondeks_core::Command;
use ondeks_core::dsp::StereoBuffer;
use ondeks_core::{Engine, NodeId};
use ondeks_core::midi::{Channel, MidiEvent, Note, Velocity};

const SR: u32 = 44100;
const BLOCK: usize = 256;

fn on(n: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        channel: Channel::new(0).unwrap(),
        note: Note::new(n).unwrap(),
        velocity: Velocity::new(100).unwrap(),
    }
}

fn off(n: u8) -> MidiEvent {
    MidiEvent::note_off(Channel::new(0).unwrap(), Note::new(n).unwrap())
}

fn send(engine: &mut Engine, target: NodeId, event: MidiEvent) {
    engine.apply_command(Command::SendMidi { target, event, sample_offset: 0 });
}

#[test]
fn command_send_midi_routes_to_synth() {
    let mut engine = Engine::new(SR, BLOCK);
    let target = engine.synth_node_id();

    send(&mut engine, target, on(60));

    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);

    assert!(out.left().peak() > 0.0, "synth should respond to SendMidi");
}

#[test]
fn send_midi_to_unknown_node_is_noop() {
    let mut engine = Engine::new(SR, BLOCK);
    let bogus = NodeId::from_raw(u64::MAX);

    // Must not panic.
    send(&mut engine, bogus, on(60));

    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0, "engine should remain silent when the target is unknown");
}

#[test]
fn polyphony_multiple_notes_in_one_block() {
    let mut engine = Engine::new(SR, BLOCK);
    let target = engine.synth_node_id();

    // Hit a triad.
    send(&mut engine, target, on(60));
    send(&mut engine, target, on(64));
    send(&mut engine, target, on(67));

    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);

    // Compare against single-note amplitude — three voices stack, so RMS
    // should be meaningfully higher than a single note at the same block size.
    let mut solo_engine = Engine::new(SR, BLOCK);
    let solo_target = solo_engine.synth_node_id();
    send(&mut solo_engine, solo_target, on(60));
    let mut solo_out = StereoBuffer::allocate(BLOCK);
    solo_engine.process(&mut solo_out, BLOCK as u32);

    assert!(
        out.left().rms() > solo_out.left().rms(),
        "triad ({} RMS) should be louder than single note ({} RMS)",
        out.left().rms(),
        solo_out.left().rms(),
    );
}

#[test]
fn note_on_then_note_off_enters_release() {
    let mut engine = Engine::new(SR, BLOCK);
    let target = engine.synth_node_id();
    let mut out = StereoBuffer::allocate(BLOCK);

    send(&mut engine, target, on(60));
    engine.process(&mut out, BLOCK as u32);
    let attack_peak = out.left().peak();
    assert!(attack_peak > 0.01);

    send(&mut engine, target, off(60));
    // Many blocks later, amplitude should be noticeably reduced.
    for _ in 0..60 {
        out.silence();
        engine.process(&mut out, BLOCK as u32);
    }
    assert!(
        out.left().peak() < attack_peak * 0.5,
        "after release the peak should be at least halved: attack={attack_peak}, now={}",
        out.left().peak()
    );
}

#[test]
fn sample_offset_respected_within_block() {
    // Sending the same NoteOn at offset 0 vs. mid-block yields different RMS
    // (less integration time → less energy). This pins P0.2 sample-accurate
    // timing through the full Command path.
    let mut early = Engine::new(SR, BLOCK);
    let target_e = early.synth_node_id();
    early.apply_command(Command::SendMidi { target: target_e, event: on(60), sample_offset: 0 });
    let mut out_e = StereoBuffer::allocate(BLOCK);
    early.process(&mut out_e, BLOCK as u32);

    let mut late = Engine::new(SR, BLOCK);
    let target_l = late.synth_node_id();
    late.apply_command(Command::SendMidi {
        target: target_l,
        event: on(60),
        sample_offset: (BLOCK as u32) - 20,
    });
    let mut out_l = StereoBuffer::allocate(BLOCK);
    late.process(&mut out_l, BLOCK as u32);

    assert!(
        out_e.left().rms() > out_l.left().rms(),
        "earlier trigger should integrate more energy (early={}, late={})",
        out_e.left().rms(),
        out_l.left().rms()
    );
}
