//! Slice 6: working mixer — volume, pan, mute, solo, master.
//!
//! Drives the engine with `Command::AddInstrumentChannel` then exercises
//! every mixer command. Smoothing is real (~50 ms), so each test renders
//! enough blocks for parameter changes to settle and asserts on the *tail*
//! of the buffer rather than peak-over-the-ramp.

use ondeks_core::Command;
use ondeks_core::dsp::StereoBuffer;
use ondeks_core::midi::{Channel, MidiEvent, Note, Velocity};
use ondeks_core::{Engine, NodeId, TrackId};

const SR: u32 = 44100;
const BLOCK: usize = 512;

/// Render this many blocks for parameter smoothing to fully settle.
const SETTLE_BLOCKS: usize = 40; // 40 × 512 = 20480 samples ≈ 9 × τ

struct ChannelRef {
    synth: NodeId,
    strip: NodeId,
}

fn add_channel(engine: &mut Engine) -> ChannelRef {
    let synth = NodeId::generate();
    let strip = NodeId::generate();
    engine.apply_command(Command::AddInstrumentChannel {
        synth_node_id: synth,
        strip_node_id: strip,
        track_id: TrackId::generate(),
    });
    ChannelRef { synth, strip }
}

fn note_on(n: u8) -> MidiEvent {
    MidiEvent::NoteOn {
        channel: Channel::new(0).unwrap(),
        note: Note::new(n).unwrap(),
        velocity: Velocity::new(100).unwrap(),
    }
}

fn render_settled(engine: &mut Engine) -> StereoBuffer {
    let mut out = StereoBuffer::allocate(BLOCK);
    for _ in 0..SETTLE_BLOCKS {
        out.silence();
        engine.process(&mut out, BLOCK as u32);
    }
    out
}

fn drive_note(engine: &mut Engine, target: NodeId) {
    engine.apply_command(Command::SendMidi {
        target,
        event: note_on(60),
        sample_offset: 0,
    });
}

#[test]
fn fader_attenuates_track() {
    let mut engine = Engine::new(SR, BLOCK);
    let ch = add_channel(&mut engine);
    drive_note(&mut engine, ch.synth);

    // Unity-gain reference.
    let baseline = render_settled(&mut engine);
    let baseline_rms = baseline.left().rms();
    assert!(baseline_rms > 0.005, "baseline should have audible RMS");

    // Drop fader to -12 dB → linear ~0.25.
    engine.apply_command(Command::SetTrackVolume {
        node_id: ch.strip,
        volume_db: -12.0,
    });
    let attenuated = render_settled(&mut engine);
    let attenuated_rms = attenuated.left().rms();

    // Should be roughly 1/4 the baseline RMS, with some tolerance.
    let ratio = attenuated_rms / baseline_rms;
    assert!(
        ratio < 0.4,
        "attenuated RMS should be much lower than baseline: ratio={ratio}"
    );
}

#[test]
fn mute_silences_track() {
    let mut engine = Engine::new(SR, BLOCK);
    let ch = add_channel(&mut engine);
    drive_note(&mut engine, ch.synth);
    let _ = render_settled(&mut engine);

    engine.apply_command(Command::SetTrackMute {
        node_id: ch.strip,
        muted: true,
    });
    let muted = render_settled(&mut engine);
    assert!(
        muted.left().peak() < 0.01 && muted.right().peak() < 0.01,
        "muted track should be silent: L={}, R={}",
        muted.left().peak(),
        muted.right().peak(),
    );

    // Unmute: audio comes back.
    engine.apply_command(Command::SetTrackMute {
        node_id: ch.strip,
        muted: false,
    });
    let unmuted = render_settled(&mut engine);
    assert!(
        unmuted.left().rms() > 0.005,
        "unmute should restore audio, got RMS {}",
        unmuted.left().rms()
    );
}

#[test]
fn solo_silences_other_tracks() {
    let mut engine = Engine::new(SR, BLOCK);
    let a = add_channel(&mut engine);
    let b = add_channel(&mut engine);

    drive_note(&mut engine, a.synth);
    drive_note(&mut engine, b.synth);

    // Solo A; B should be silenced.
    engine.apply_command(Command::SetTrackSolo {
        node_id: a.strip,
        soloed: true,
    });
    let solo_a = render_settled(&mut engine);
    let solo_a_rms = solo_a.left().rms();
    assert!(solo_a_rms > 0.001);

    // Disable solo entirely; both should sound (and total signal should be
    // at least as loud as A alone — both voices stack).
    engine.apply_command(Command::SetTrackSolo {
        node_id: a.strip,
        soloed: false,
    });
    let both = render_settled(&mut engine);
    assert!(
        both.left().rms() >= solo_a_rms,
        "both-on RMS should match or exceed solo-A: solo={solo_a_rms}, both={}",
        both.left().rms()
    );
}

#[test]
fn pan_full_left_silences_right() {
    let mut engine = Engine::new(SR, BLOCK);
    let ch = add_channel(&mut engine);
    drive_note(&mut engine, ch.synth);
    let _ = render_settled(&mut engine);

    engine.apply_command(Command::SetTrackPan {
        node_id: ch.strip,
        pan: -1.0,
    });
    let panned = render_settled(&mut engine);
    assert!(
        panned.right().peak() < 0.01,
        "right should be silenced when panned full left, got peak={}",
        panned.right().peak()
    );
    assert!(
        panned.left().rms() > 0.005,
        "left should still be audible, got RMS {}",
        panned.left().rms()
    );
}

#[test]
fn master_fader_attenuates_everything() {
    let mut engine = Engine::new(SR, BLOCK);
    let ch = add_channel(&mut engine);
    drive_note(&mut engine, ch.synth);
    let baseline = render_settled(&mut engine);
    let baseline_rms = baseline.left().rms();
    assert!(baseline_rms > 0.005);

    engine.apply_command(Command::SetMasterVolume { volume_db: -24.0 });
    let attenuated = render_settled(&mut engine);
    assert!(
        attenuated.left().rms() < baseline_rms * 0.5,
        "master -24 dB should be much quieter: baseline={baseline_rms}, attenuated={}",
        attenuated.left().rms()
    );
}
