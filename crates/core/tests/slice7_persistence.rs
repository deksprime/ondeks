//! Slice 7: project persistence round-trips.
//!
//! Builds a non-trivial project (multiple MIDI/audio tracks with mixer
//! state, custom colors, scenes), serializes to JSON, deserializes, asserts
//! structural equality. Also pins the migration framework: current-version
//! files load directly, future versions error cleanly.

#![cfg(feature = "serde")]

use ondeks_core::persistence::{
    migrate_to_current, project_from_json, project_to_json, version_of, MigrateError,
};
use ondeks_core::project::{Project, TrackType};
use ondeks_core::Color;

fn build_complex_project() -> Project {
    let mut p = Project::new("Round-Trip Test");
    p.meta.author = "tester".to_string();

    let bass_id = p.add_track(TrackType::Midi, "Bass");
    let lead_id = p.add_track(TrackType::Midi, "Lead");
    let drums_id = p.add_track(TrackType::Audio, "Drums");

    {
        let bass = p.get_track_mut(bass_id).unwrap();
        bass.volume_db = -3.0;
        bass.pan = -0.5;
        bass.muted = false;
        bass.color = Color::RED;
    }
    {
        let lead = p.get_track_mut(lead_id).unwrap();
        lead.volume_db = -6.0;
        lead.pan = 0.25;
        lead.soloed = true;
        lead.armed = true;
        lead.color = Color::BLUE;
    }
    {
        let drums = p.get_track_mut(drums_id).unwrap();
        drums.volume_db = 0.0;
        drums.muted = true;
        drums.color = Color::YELLOW;
    }
    {
        let master = p.master_mut();
        master.volume_db = -2.5;
    }

    p.add_scene("Intro");
    p.add_scene("Verse");

    p
}

fn assert_track_equivalent(a: &ondeks_core::project::Track, b: &ondeks_core::project::Track) {
    assert_eq!(a.id, b.id, "track id");
    assert_eq!(a.name, b.name, "track name");
    assert_eq!(a.track_type, b.track_type, "track type");
    assert_eq!(a.color, b.color, "track color");
    assert!((a.volume_db - b.volume_db).abs() < 1e-3, "volume_db");
    assert!((a.pan - b.pan).abs() < 1e-3, "pan");
    assert_eq!(a.muted, b.muted, "muted");
    assert_eq!(a.soloed, b.soloed, "soloed");
    assert_eq!(a.armed, b.armed, "armed");
    assert_eq!(a.instrument, b.instrument, "instrument node id");
    assert_eq!(a.channel_strip, b.channel_strip, "channel_strip node id");
}

#[test]
fn round_trip_preserves_track_state() {
    let original = build_complex_project();
    let json = project_to_json(&original).expect("serialize");
    let loaded = project_from_json(&json).expect("deserialize");

    assert_eq!(loaded.tracks().len(), original.tracks().len());
    assert_eq!(loaded.meta.name, original.meta.name);
    assert_eq!(loaded.meta.author, original.meta.author);

    for (a, b) in original.tracks().iter().zip(loaded.tracks().iter()) {
        assert_track_equivalent(a, b);
    }
}

#[test]
fn round_trip_preserves_master_volume() {
    let original = build_complex_project();
    let json = project_to_json(&original).expect("serialize");
    let loaded = project_from_json(&json).expect("deserialize");
    assert!((loaded.master().volume_db - original.master().volume_db).abs() < 1e-3);
}

#[test]
fn round_trip_preserves_scenes_in_order() {
    let original = build_complex_project();
    let json = project_to_json(&original).expect("serialize");
    let loaded = project_from_json(&json).expect("deserialize");

    let original_names: Vec<&str> = original.scenes().iter().map(|s| s.name.as_str()).collect();
    let loaded_names: Vec<&str> = loaded.scenes().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(original_names, loaded_names);
}

#[test]
fn round_trip_preserves_track_order() {
    let original = build_complex_project();
    let json = project_to_json(&original).expect("serialize");
    let loaded = project_from_json(&json).expect("deserialize");

    let original_ids: Vec<_> = original.tracks().iter().map(|t| t.id).collect();
    let loaded_ids: Vec<_> = loaded.tracks().iter().map(|t| t.id).collect();
    assert_eq!(original_ids, loaded_ids);
}

#[test]
fn empty_project_round_trips() {
    let original = Project::new("empty");
    let json = project_to_json(&original).expect("serialize");
    let loaded = project_from_json(&json).expect("deserialize");
    assert_eq!(loaded.tracks().len(), 1); // master only
    assert_eq!(loaded.master().track_type, TrackType::Master);
    // Project::new seeds Scene 1; round-trip should preserve.
    assert_eq!(loaded.scenes().len(), 1);
}

#[test]
fn version_probe_reads_current_version() {
    let original = build_complex_project();
    let json = project_to_json(&original).expect("serialize");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(version_of(&value).unwrap(), 1);
}

#[test]
fn unknown_future_version_rejected() {
    let v = serde_json::json!({ "version": 999 });
    let err = migrate_to_current(v, 999).unwrap_err();
    assert!(matches!(err, MigrateError::UnsupportedFutureVersion(999, 1)));
}

#[test]
fn malformed_json_returns_load_error() {
    let result = project_from_json("{ not valid json");
    assert!(result.is_err(), "expected parse error, got {:?}", result.map(|_| "Ok"));
}

#[test]
fn unknown_track_type_returns_load_error() {
    // Hand-craft a v1 file with a bogus track type.
    let json = serde_json::json!({
        "version": 1,
        "meta": { "name": "t", "author": "" },
        "tempo": 120.0,
        "time_signature": [4, 4],
        "tracks": [
            {
                "id": 1,
                "name": "?",
                "track_type": "Quantum",
                "volume_db": 0.0,
                "pan": 0.0,
                "muted": false,
                "arrangement_clips": []
            },
            {
                "id": 2,
                "name": "Master",
                "track_type": "Master",
                "volume_db": 0.0,
                "pan": 0.0,
                "muted": false,
                "arrangement_clips": []
            }
        ],
        "clips": [],
        "scenes": []
    })
    .to_string();
    let err = project_from_json(&json).unwrap_err();
    assert!(format!("{err}").contains("unknown track type"));
}
