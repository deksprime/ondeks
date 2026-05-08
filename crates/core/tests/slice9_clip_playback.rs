//! Slice 9: clip launcher → engine integration.
//!
//! Unit-level launcher correctness lives in
//! `crates/core/src/session/launcher.rs` (sample-accurate event emission,
//! loop wrap, displacing clips). These tests exercise the engine command
//! surface end-to-end: `LaunchClip` queues, the launcher dequeues during
//! `Engine::process`, slot-state transitions are drained for the runtime, and
//! `StopAll` cancels everything.

use ondeks_core::dsp::StereoBuffer;
use ondeks_core::midi::{Channel, Note, Velocity};
use ondeks_core::project::{Clip, MidiClip, MidiNote, Project, TrackType};
use ondeks_core::session::{ClipNote, ClipPlayback, LaunchQuantize, SlotState};
use ondeks_core::transport::Beats;
use ondeks_core::{Command, Engine, NodeId};

const SAMPLE_RATE: u32 = 44100;
const BLOCK: u32 = 256;

fn boot_engine_with_synth() -> (Engine, NodeId) {
    let mut engine = Engine::new(SAMPLE_RATE, BLOCK as usize);
    // Register a synth + strip pair so launches have somewhere to send MIDI.
    let synth_id = NodeId::generate();
    let strip_id = NodeId::generate();
    let track_id = ondeks_core::TrackId::generate();
    engine.apply_command(Command::AddInstrumentChannel {
        synth_node_id: synth_id,
        strip_node_id: strip_id,
        track_id,
    });
    (engine, synth_id)
}

fn playback(target: NodeId, length: f64, notes: Vec<(f64, f64, u8)>) -> ClipPlayback {
    ClipPlayback {
        target_node: target,
        length_beats: length,
        notes: notes
            .into_iter()
            .map(|(start, len, pitch)| ClipNote {
                start_beats: start,
                length_beats: len,
                pitch,
                velocity: 100,
                channel: 0,
            })
            .collect(),
    }
}

fn run_block(engine: &mut Engine) {
    let mut buf = StereoBuffer::allocate(BLOCK as usize);
    engine.process(&mut buf, BLOCK);
}

#[test]
fn launching_a_clip_emits_queued_then_playing_state_change() {
    let (mut engine, synth_id) = boot_engine_with_synth();
    // Skip quantization: trigger immediately.
    engine.apply_command(Command::SetLaunchQuantize(LaunchQuantize::None));

    engine.apply_command(Command::LaunchClip {
        track: 0,
        scene: 0,
        playback: playback(synth_id, 4.0, vec![(0.0, 1.0, 60)]),
    });

    let initial = engine.drain_slot_state_changes();
    assert!(initial
        .iter()
        .any(|c| c.track == 0 && c.scene == 0 && matches!(c.state, SlotState::Queued)));

    // Transport is stopped → engine.process should not advance the launcher.
    run_block(&mut engine);
    assert!(engine.drain_slot_state_changes().is_empty());
    assert_eq!(
        engine.clip_launcher().slot_state(0, 0),
        SlotState::Queued,
        "launcher must not dequeue while transport is stopped"
    );

    // Start the transport. Next block should dequeue the clip.
    engine.apply_command(Command::Play);
    run_block(&mut engine);

    let post_play = engine.drain_slot_state_changes();
    assert!(post_play
        .iter()
        .any(|c| c.track == 0 && c.scene == 0 && matches!(c.state, SlotState::Playing)));
    assert_eq!(
        engine.clip_launcher().slot_state(0, 0),
        SlotState::Playing
    );
}

#[test]
fn launching_a_second_slot_on_track_displaces_the_first() {
    let (mut engine, synth_id) = boot_engine_with_synth();
    engine.apply_command(Command::SetLaunchQuantize(LaunchQuantize::None));

    engine.apply_command(Command::LaunchClip {
        track: 0,
        scene: 0,
        playback: playback(synth_id, 4.0, vec![(0.0, 4.0, 60)]),
    });
    let _ = engine.drain_slot_state_changes();

    engine.apply_command(Command::Play);
    run_block(&mut engine);
    // Now slot (0,0) is playing with a long note.
    assert_eq!(
        engine.clip_launcher().slot_state(0, 0),
        SlotState::Playing
    );
    let _ = engine.drain_slot_state_changes();

    // Launch a new clip on the same track — the first should be stopped.
    engine.apply_command(Command::LaunchClip {
        track: 0,
        scene: 1,
        playback: playback(synth_id, 4.0, vec![(0.0, 1.0, 64)]),
    });
    let changes = engine.drain_slot_state_changes();
    assert!(
        changes
            .iter()
            .any(|c| c.scene == 0 && matches!(c.state, SlotState::Stopped)),
        "old slot should be stopped on launch of a new slot on the same track"
    );
    assert!(changes
        .iter()
        .any(|c| c.scene == 1 && matches!(c.state, SlotState::Queued)));
    assert_eq!(
        engine.clip_launcher().slot_state(0, 0),
        SlotState::Stopped
    );
}

#[test]
fn stop_all_clears_active_clips() {
    let (mut engine, synth_id) = boot_engine_with_synth();
    engine.apply_command(Command::SetLaunchQuantize(LaunchQuantize::None));
    engine.apply_command(Command::LaunchClip {
        track: 0,
        scene: 0,
        playback: playback(synth_id, 4.0, vec![(0.0, 4.0, 60)]),
    });
    engine.apply_command(Command::Play);
    run_block(&mut engine);
    let _ = engine.drain_slot_state_changes();

    engine.apply_command(Command::StopAll);
    let changes = engine.drain_slot_state_changes();
    assert!(changes
        .iter()
        .any(|c| matches!(c.state, SlotState::Stopped)));
    assert_eq!(
        engine.clip_launcher().slot_state(0, 0),
        SlotState::Stopped
    );
}

#[test]
fn stop_track_only_stops_one_track() {
    let (mut engine, synth_id) = boot_engine_with_synth();
    engine.apply_command(Command::SetLaunchQuantize(LaunchQuantize::None));
    engine.apply_command(Command::LaunchClip {
        track: 0,
        scene: 0,
        playback: playback(synth_id, 4.0, vec![]),
    });
    engine.apply_command(Command::LaunchClip {
        track: 1,
        scene: 0,
        playback: playback(synth_id, 4.0, vec![]),
    });
    engine.apply_command(Command::Play);
    run_block(&mut engine);
    let _ = engine.drain_slot_state_changes();

    engine.apply_command(Command::StopTrack { track: 0 });
    assert_eq!(
        engine.clip_launcher().slot_state(0, 0),
        SlotState::Stopped
    );
    assert_eq!(
        engine.clip_launcher().slot_state(1, 0),
        SlotState::Playing
    );
}

#[test]
fn launch_scene_starts_every_slot_in_the_row() {
    let (mut engine, synth_id) = boot_engine_with_synth();
    engine.apply_command(Command::SetLaunchQuantize(LaunchQuantize::None));
    engine.apply_command(Command::LaunchScene {
        scene: 2,
        playbacks: vec![
            (0, playback(synth_id, 4.0, vec![])),
            (1, playback(synth_id, 4.0, vec![])),
            (3, playback(synth_id, 4.0, vec![])),
        ],
    });
    engine.apply_command(Command::Play);
    run_block(&mut engine);

    assert_eq!(
        engine.clip_launcher().slot_state(0, 2),
        SlotState::Playing
    );
    assert_eq!(
        engine.clip_launcher().slot_state(1, 2),
        SlotState::Playing
    );
    assert_eq!(
        engine.clip_launcher().slot_state(3, 2),
        SlotState::Playing
    );
    // Track 2 was never launched.
    assert_eq!(
        engine.clip_launcher().slot_state(2, 2),
        SlotState::Empty
    );
}

#[test]
fn project_clip_playback_for_slot_extracts_notes() {
    let mut p = Project::new("test");
    let track_id = p.add_track(TrackType::Midi, "Lead");
    let mut clip = MidiClip::new("Pattern", Beats(2.0));
    clip.notes.push(MidiNote {
        time: Beats(0.0),
        length: Beats(0.5),
        pitch: Note::new(60).unwrap(),
        velocity: Velocity::new(110).unwrap(),
        channel: Channel::new(0).unwrap(),
    });
    let clip_id = clip.id();
    p.add_clip(Clip::Midi(clip));
    p.get_track_mut(track_id)
        .unwrap()
        .set_session_slot(0, Some(clip_id));

    // Master is at index 1 in a fresh project (it was added first); the new
    // MIDI track is at index 0 in the *non-master* ordering used by the
    // session view.
    let pb = p
        .clip_playback_for_slot(0, 0)
        .expect("playback for non-empty slot");
    assert_eq!(pb.length_beats, 2.0);
    assert_eq!(pb.notes.len(), 1);
    assert_eq!(pb.notes[0].pitch, 60);
    assert_eq!(pb.notes[0].velocity, 110);
    let track_node = p.get_track(track_id).unwrap().instrument.unwrap();
    assert_eq!(pb.target_node, track_node);
}

#[test]
fn project_scene_playbacks_skips_empty_and_audio_slots() {
    let mut p = Project::new("test");
    let midi_track = p.add_track(TrackType::Midi, "Lead");
    let _audio_track = p.add_track(TrackType::Audio, "Drums");
    let mut clip = MidiClip::new("Pattern", Beats(4.0));
    clip.notes.push(MidiNote::new(Beats(0.0), Beats(1.0), Note::new(60).unwrap()));
    let clip_id = clip.id();
    p.add_clip(Clip::Midi(clip));
    p.get_track_mut(midi_track)
        .unwrap()
        .set_session_slot(0, Some(clip_id));

    let scene = p.scene_playbacks(0);
    assert_eq!(scene.len(), 1, "only the MIDI slot with a clip should appear");
    assert_eq!(scene[0].0, 0);
}
