//! Slice 8: dispatcher-level MIDI note CRUD round-trips with undo.

use ondeks_core::midi::{Note, Velocity};
use ondeks_core::project::{Clip, MidiClip, MidiNote, Project, TrackType};
use ondeks_core::transport::Beats;
use ondeks_ui_common::commands::ProjectCommand;
use ondeks_ui_common::dispatch::apply_project_command;

fn n(time: f64, length: f64, pitch: u8) -> MidiNote {
    MidiNote::new(
        Beats(time),
        Beats(length),
        Note::new(pitch).expect("valid pitch"),
    )
}

fn project_with_clip() -> (Project, ondeks_core::ClipId) {
    let mut p = Project::new("test");
    p.add_track(TrackType::Midi, "Lead");
    let clip = MidiClip::new("Pattern", Beats(4.0));
    let clip_id = clip.id();
    p.add_clip(Clip::Midi(clip));
    (p, clip_id)
}

#[test]
fn add_midi_note_appends_and_undo_removes() {
    let (mut p, clip_id) = project_with_clip();
    let note = n(0.0, 0.25, 60);

    let outcome = apply_project_command(
        &mut p,
        &ProjectCommand::AddMidiNote { clip_id, note },
    )
    .unwrap();

    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes.len(), 1);
        assert_eq!(midi.notes[0], note);
    } else {
        panic!("expected MIDI clip");
    }

    apply_project_command(&mut p, &outcome.undo).unwrap();
    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert!(midi.notes.is_empty());
    }
}

#[test]
fn add_undo_redo_restores_exact_note() {
    let (mut p, clip_id) = project_with_clip();
    let note = n(1.5, 0.5, 64);
    let outcome = apply_project_command(
        &mut p,
        &ProjectCommand::AddMidiNote { clip_id, note },
    )
    .unwrap();

    apply_project_command(&mut p, &outcome.undo).unwrap();
    apply_project_command(&mut p, &outcome.redo).unwrap();

    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes, vec![note]);
    }
}

#[test]
fn remove_midi_note_undo_restores_at_same_index() {
    let (mut p, clip_id) = project_with_clip();
    apply_project_command(&mut p, &ProjectCommand::AddMidiNote { clip_id, note: n(0.0, 0.25, 60) }).unwrap();
    apply_project_command(&mut p, &ProjectCommand::AddMidiNote { clip_id, note: n(1.0, 0.25, 64) }).unwrap();
    apply_project_command(&mut p, &ProjectCommand::AddMidiNote { clip_id, note: n(2.0, 0.25, 67) }).unwrap();

    let rm = apply_project_command(
        &mut p,
        &ProjectCommand::RemoveMidiNote { clip_id, note_index: 1 },
    )
    .unwrap();

    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes.len(), 2);
        assert_eq!(midi.notes[1].pitch.raw(), 67);
    }

    apply_project_command(&mut p, &rm.undo).unwrap();
    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes.len(), 3);
        assert_eq!(midi.notes[1].pitch.raw(), 64);
    }
}

#[test]
fn move_midi_note_undo_restores_position() {
    let (mut p, clip_id) = project_with_clip();
    apply_project_command(&mut p, &ProjectCommand::AddMidiNote { clip_id, note: n(0.0, 0.25, 60) }).unwrap();

    let mv = apply_project_command(
        &mut p,
        &ProjectCommand::MoveMidiNote {
            clip_id,
            note_index: 0,
            new_time: Beats(2.0),
            new_pitch: Note::new(72).unwrap(),
        },
    )
    .unwrap();

    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes[0].time.0, 2.0);
        assert_eq!(midi.notes[0].pitch.raw(), 72);
    }

    apply_project_command(&mut p, &mv.undo).unwrap();
    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes[0].time.0, 0.0);
        assert_eq!(midi.notes[0].pitch.raw(), 60);
    }
}

#[test]
fn resize_midi_note_undo_restores_length() {
    let (mut p, clip_id) = project_with_clip();
    apply_project_command(&mut p, &ProjectCommand::AddMidiNote { clip_id, note: n(0.0, 0.25, 60) }).unwrap();

    let rz = apply_project_command(
        &mut p,
        &ProjectCommand::ResizeMidiNote {
            clip_id,
            note_index: 0,
            new_length: Beats(1.5),
        },
    )
    .unwrap();

    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert!((midi.notes[0].length.0 - 1.5).abs() < 1e-6);
    }

    apply_project_command(&mut p, &rz.undo).unwrap();
    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert!((midi.notes[0].length.0 - 0.25).abs() < 1e-6);
    }
}

#[test]
fn set_note_velocity_undo_restores_value() {
    let (mut p, clip_id) = project_with_clip();
    apply_project_command(&mut p, &ProjectCommand::AddMidiNote { clip_id, note: n(0.0, 0.25, 60) }).unwrap();

    let v = apply_project_command(
        &mut p,
        &ProjectCommand::SetNoteVelocity {
            clip_id,
            note_index: 0,
            new_velocity: Velocity::new(40).unwrap(),
        },
    )
    .unwrap();

    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes[0].velocity.raw(), 40);
    }

    apply_project_command(&mut p, &v.undo).unwrap();
    if let Clip::Midi(midi) = p.get_clip(clip_id).unwrap() {
        assert_eq!(midi.notes[0].velocity.raw(), 100);
    }
}
