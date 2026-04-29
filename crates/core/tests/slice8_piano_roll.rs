//! Slice 8: piano roll persistence — notes round-trip + v1 fallback.
//!
//! The dispatcher-level note CRUD tests live in
//! `crates/ui-common/tests/slice8_piano_roll.rs` (they need the
//! `ProjectCommand` enum which lives there).

#![cfg(feature = "serde")]

use ondeks_core::midi::{Channel, MidiEvent, Note, Velocity};
use ondeks_core::persistence::{project_from_json, project_to_json};
use ondeks_core::project::{Clip, MidiClip, MidiNote, Project, TrackType};
use ondeks_core::transport::Beats;

fn n(time: f64, length: f64, pitch: u8) -> MidiNote {
    MidiNote::new(
        Beats(time),
        Beats(length),
        Note::new(pitch).expect("valid pitch"),
    )
}

#[test]
fn notes_round_trip_through_persistence() {
    let mut p = Project::new("test");
    p.add_track(TrackType::Midi, "Lead");
    let mut clip = MidiClip::new("Pattern", Beats(4.0));
    clip.notes.push(n(0.0, 0.5, 60));
    clip.notes.push(n(0.5, 0.25, 64));
    clip.notes.push(n(1.0, 0.5, 67));
    let clip_id = clip.id();
    p.add_clip(Clip::Midi(clip));

    let json = project_to_json(&p).expect("serialize");
    let loaded = project_from_json(&json).expect("deserialize");

    let loaded_clip = loaded.get_clip(clip_id).expect("clip exists in loaded");
    if let Clip::Midi(midi) = loaded_clip {
        assert_eq!(midi.notes.len(), 3);
        assert_eq!(midi.notes[0].pitch.raw(), 60);
        assert_eq!(midi.notes[1].pitch.raw(), 64);
        assert_eq!(midi.notes[2].pitch.raw(), 67);
        assert!((midi.notes[0].length.0 - 0.5).abs() < 1e-6);
    } else {
        panic!("expected MIDI clip");
    }
}

#[test]
fn old_v1_file_with_events_lowers_to_notes() {
    // A v1 file that only has midi_events (no `notes` field) — loader
    // pairs NoteOn/NoteOff into editable MidiNotes.
    let json = serde_json::json!({
        "version": 1,
        "meta": { "name": "old", "author": "" },
        "tempo": 120.0,
        "time_signature": [4, 4],
        "tracks": [
            { "id": 1, "name": "Master", "track_type": "Master",
              "volume_db": 0.0, "pan": 0.0, "muted": false,
              "arrangement_clips": [] }
        ],
        "clips": [
            {
                "id": 99,
                "name": "Old Pattern",
                "clip_type": "midi",
                "length": 4.0,
                "midi_events": [
                    { "time": 0.0, "event_type": "note_on", "channel": 0, "note": 60, "velocity": 100 },
                    { "time": 0.5, "event_type": "note_off", "channel": 0, "note": 60, "velocity": 0 },
                    { "time": 1.0, "event_type": "note_on", "channel": 0, "note": 64, "velocity": 90 },
                    { "time": 1.25, "event_type": "note_off", "channel": 0, "note": 64, "velocity": 0 }
                ]
            }
        ],
        "scenes": []
    })
    .to_string();

    let project = project_from_json(&json).expect("loads");
    let clip = project
        .clips()
        .find(|c| c.header().name == "Old Pattern")
        .expect("clip present");

    if let Clip::Midi(midi) = clip {
        assert_eq!(midi.notes.len(), 2, "two paired notes");
        assert_eq!(midi.notes[0].pitch.raw(), 60);
        assert!((midi.notes[0].length.0 - 0.5).abs() < 1e-6);
        assert_eq!(midi.notes[1].pitch.raw(), 64);
        assert!((midi.notes[1].length.0 - 0.25).abs() < 1e-6);
        // Note events should be stripped from the legacy sequence after
        // lowering — only non-note events survive.
        let note_event_count = midi
            .sequence
            .events()
            .iter()
            .filter(|e| matches!(e.event, MidiEvent::NoteOn { .. } | MidiEvent::NoteOff { .. }))
            .count();
        assert_eq!(note_event_count, 0);
    } else {
        panic!("expected MIDI clip");
    }
}

#[test]
fn unmatched_note_on_gets_default_length() {
    let json = serde_json::json!({
        "version": 1,
        "meta": { "name": "unmatched", "author": "" },
        "tempo": 120.0,
        "time_signature": [4, 4],
        "tracks": [
            { "id": 1, "name": "Master", "track_type": "Master",
              "volume_db": 0.0, "pan": 0.0, "muted": false,
              "arrangement_clips": [] }
        ],
        "clips": [
            {
                "id": 1, "name": "Stuck", "clip_type": "midi", "length": 4.0,
                "midi_events": [
                    { "time": 0.0, "event_type": "note_on", "channel": 0, "note": 60, "velocity": 100 }
                ]
            }
        ],
        "scenes": []
    })
    .to_string();

    let project = project_from_json(&json).expect("loads");
    let clip = project.clips().next().unwrap();
    if let Clip::Midi(midi) = clip {
        assert_eq!(midi.notes.len(), 1);
        assert!(midi.notes[0].length.0 > 0.0);
    }
}

#[test]
fn channel_and_velocity_round_trip() {
    let mut p = Project::new("test");
    p.add_track(TrackType::Midi, "Lead");
    let mut clip = MidiClip::new("Pattern", Beats(4.0));
    let mut note = n(0.0, 0.25, 60);
    note.velocity = Velocity::new(42).unwrap();
    note.channel = Channel::new(7).unwrap();
    clip.notes.push(note);
    let clip_id = clip.id();
    p.add_clip(Clip::Midi(clip));

    let json = project_to_json(&p).unwrap();
    let loaded = project_from_json(&json).unwrap();
    let loaded_clip = loaded.get_clip(clip_id).unwrap();
    if let Clip::Midi(midi) = loaded_clip {
        assert_eq!(midi.notes[0].velocity.raw(), 42);
        assert_eq!(midi.notes[0].channel.raw(), 7);
    }
}
