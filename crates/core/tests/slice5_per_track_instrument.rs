//! Slice 5 (+ 6): per-track instrument channels.
//!
//! Exercises the engine's `AddInstrumentChannel` / `RemoveInstrumentChannel`
//! commands and the invariant that multiple synth nodes coexist independently
//! — sending MIDI to one doesn't leak into another.
//!
//! Slice 6 changed the topology to `Synth → ChannelStrip → MasterStrip →
//! Output`, so the default engine has two nodes in its graph (master strip +
//! output), and adding a channel adds two more (synth + strip).

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

fn add_channel(engine: &mut Engine) -> NodeId {
    let synth_node_id = NodeId::generate();
    let strip_node_id = NodeId::generate();
    engine.apply_command(Command::AddInstrumentChannel {
        synth_node_id,
        strip_node_id,
        track_id: TrackId::generate(),
    });
    synth_node_id
}

#[test]
fn default_engine_has_master_strip_and_output() {
    let engine = Engine::new(SR, BLOCK);
    let graph = engine.graph();
    let count = graph.node_ids().count();
    assert_eq!(count, 2, "fresh engine should have master strip + output, got {count}");
}

#[test]
fn add_instrument_channel_produces_audio_when_driven() {
    let mut engine = Engine::new(SR, BLOCK);
    let synth = add_channel(&mut engine);

    engine.apply_command(Command::SendMidi {
        target: synth,
        event: note_on(60),
        sample_offset: 0,
    });

    // Render enough samples for the channel-strip smoothing to settle to
    // steady state. The strip starts at unity gain → unity gain (no change),
    // so smoothing is a no-op here, but a small warm-up is harmless.
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert!(out.left().peak() > 0.01, "new channel should produce audio");
}

#[test]
fn remove_instrument_channel_silences_output() {
    let mut engine = Engine::new(SR, BLOCK);
    let synth_node_id = NodeId::generate();
    let strip_node_id = NodeId::generate();
    engine.apply_command(Command::AddInstrumentChannel {
        synth_node_id,
        strip_node_id,
        track_id: TrackId::generate(),
    });

    engine.apply_command(Command::SendMidi {
        target: synth_node_id,
        event: note_on(60),
        sample_offset: 0,
    });
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert!(out.left().peak() > 0.01);

    engine.apply_command(Command::RemoveInstrumentChannel { synth_node_id, strip_node_id });
    out.silence();
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0, "removed channel should not produce audio");
}

#[test]
fn two_channels_are_independent() {
    let mut engine = Engine::new(SR, BLOCK);
    let a = add_channel(&mut engine);
    let b = add_channel(&mut engine);
    assert_ne!(a, b);

    // MIDI to A only.
    engine.apply_command(Command::SendMidi {
        target: a,
        event: note_on(60),
        sample_offset: 0,
    });
    let mut only_a = StereoBuffer::allocate(BLOCK);
    engine.process(&mut only_a, BLOCK as u32);
    let peak_a_only = only_a.left().peak();
    assert!(peak_a_only > 0.01);

    // Adding MIDI to B should increase the total signal.
    engine.apply_command(Command::SendMidi {
        target: b,
        event: note_on(64),
        sample_offset: 0,
    });
    let mut both = StereoBuffer::allocate(BLOCK);
    engine.process(&mut both, BLOCK as u32);
    assert!(
        both.left().rms() > only_a.left().rms(),
        "two active channels should sum louder than one (a_only_rms={}, both_rms={})",
        only_a.left().rms(),
        both.left().rms(),
    );
}

#[test]
fn add_channel_with_duplicate_ids_is_noop() {
    let mut engine = Engine::new(SR, BLOCK);
    let synth_node_id = NodeId::generate();
    let strip_node_id = NodeId::generate();
    engine.apply_command(Command::AddInstrumentChannel {
        synth_node_id,
        strip_node_id,
        track_id: TrackId::generate(),
    });
    // Re-applying with the same ids must not panic.
    engine.apply_command(Command::AddInstrumentChannel {
        synth_node_id,
        strip_node_id,
        track_id: TrackId::generate(),
    });

    engine.apply_command(Command::SendMidi {
        target: synth_node_id,
        event: note_on(60),
        sample_offset: 0,
    });
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert!(out.left().peak() > 0.01);
}

#[test]
fn remove_unknown_channel_is_noop() {
    let mut engine = Engine::new(SR, BLOCK);
    let bogus_synth = NodeId::from_raw(u64::MAX);
    let bogus_strip = NodeId::from_raw(u64::MAX - 1);
    engine.apply_command(Command::RemoveInstrumentChannel {
        synth_node_id: bogus_synth,
        strip_node_id: bogus_strip,
    });
    let mut out = StereoBuffer::allocate(BLOCK);
    engine.process(&mut out, BLOCK as u32);
    assert_eq!(out.left().peak(), 0.0);
}
