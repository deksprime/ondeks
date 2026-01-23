//! Integration tests for Phase 13: Persistence & File Format
//!
//! These tests verify that project serialization works correctly.

use ondeks_core::project::{Project, TrackType, Clip, MidiClip};
#[cfg(feature = "serde")]
use ondeks_core::persistence::{project_to_file, project_to_json, ProjectFile, FORMAT_VERSION};
use ondeks_core::transport::Beats;
use ondeks_core::midi::{MidiEvent, TimestampedEvent, Channel, Note, Velocity};
use ondeks_core::{AudioPoolId};

#[cfg(feature = "serde")]
#[test]
fn project_to_file_basic() {
    let project = Project::new("Test Project");
    let file = project_to_file(&project);
    
    assert_eq!(file.version, FORMAT_VERSION);
    assert_eq!(file.meta.name, "Test Project");
    assert_eq!(file.meta.author, "");
    assert_eq!(file.tempo, 120.0);
    assert_eq!(file.time_signature, (4, 4));
    assert_eq!(file.tracks.len(), 1); // Master track
    assert_eq!(file.clips.len(), 0);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_tracks() {
    let mut project = Project::new("Test Project");
    project.meta.author = "Test Author".to_string();
    
    let track1_id = project.add_track(TrackType::Midi, "Bass");
    let track2_id = project.add_track(TrackType::Audio, "Drums");
    
    let file = project_to_file(&project);
    
    assert_eq!(file.meta.author, "Test Author");
    assert_eq!(file.tracks.len(), 3); // 2 tracks + master
    
    let bass_track = file.tracks.iter().find(|t| t.name == "Bass").unwrap();
    assert_eq!(bass_track.track_type, "Midi");
    assert_eq!(bass_track.volume_db, 0.0);
    assert_eq!(bass_track.pan, 0.0);
    assert!(!bass_track.muted);
    
    let drums_track = file.tracks.iter().find(|t| t.name == "Drums").unwrap();
    assert_eq!(drums_track.track_type, "Audio");
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_clips() {
    let mut project = Project::new("Test Project");
    let track_id = project.add_track(TrackType::Midi, "Lead");
    
    let clip = MidiClip::new("Pattern 1", Beats(4.0));
    let clip_id = project.add_clip(Clip::Midi(clip));
    
    project.place_clip(track_id, clip_id, Beats(0.0)).unwrap();
    
    let file = project_to_file(&project);
    
    assert_eq!(file.clips.len(), 1);
    assert_eq!(file.clips[0].name, "Pattern 1");
    assert_eq!(file.clips[0].clip_type, "midi");
    assert_eq!(file.clips[0].length, 4.0);
    
    let lead_track = file.tracks.iter().find(|t| t.name == "Lead").unwrap();
    assert_eq!(lead_track.arrangement_clips.len(), 1);
    assert_eq!(lead_track.arrangement_clips[0].clip_id, clip_id.raw());
    assert_eq!(lead_track.arrangement_clips[0].position, 0.0);
    assert_eq!(lead_track.arrangement_clips[0].length, 4.0);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_midi_events() {
    let mut project = Project::new("Test Project");
    
    let mut clip = MidiClip::new("Melody", Beats(4.0));
    let channel = Channel::new(0).unwrap();
    let note = Note::new(60).unwrap();
    let velocity = Velocity::new(100).unwrap();
    
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(0.0),
        MidiEvent::note_on(channel, note, velocity),
    ));
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(1.0),
        MidiEvent::note_off(channel, note),
    ));
    
    let clip_id = project.add_clip(Clip::Midi(clip));
    
    let file = project_to_file(&project);
    
    assert_eq!(file.clips.len(), 1);
    let midi_events = file.clips[0].midi_events.as_ref().unwrap();
    assert_eq!(midi_events.len(), 2);
    
    assert_eq!(midi_events[0].event_type, "note_on");
    assert_eq!(midi_events[0].time, 0.0);
    assert_eq!(midi_events[0].channel, 0);
    assert_eq!(midi_events[0].note, Some(60));
    assert_eq!(midi_events[0].velocity, Some(100));
    
    assert_eq!(midi_events[1].event_type, "note_off");
    assert_eq!(midi_events[1].time, 1.0);
    assert_eq!(midi_events[1].channel, 0);
    assert_eq!(midi_events[1].note, Some(60));
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_control_change() {
    let mut project = Project::new("Test Project");
    
    let mut clip = MidiClip::new("CC Test", Beats(4.0));
    let channel = Channel::new(1).unwrap();
    
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(2.0),
        MidiEvent::ControlChange {
            channel,
            controller: 7, // Volume
            value: 64,
        },
    ));
    
    let clip_id = project.add_clip(Clip::Midi(clip));
    
    let file = project_to_file(&project);
    
    let midi_events = file.clips[0].midi_events.as_ref().unwrap();
    assert_eq!(midi_events.len(), 1);
    assert_eq!(midi_events[0].event_type, "control_change");
    assert_eq!(midi_events[0].controller, Some(7));
    assert_eq!(midi_events[0].value, Some(64));
    assert_eq!(midi_events[0].note, None);
    assert_eq!(midi_events[0].velocity, None);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_program_change() {
    let mut project = Project::new("Test Project");
    
    let mut clip = MidiClip::new("PC Test", Beats(4.0));
    let channel = Channel::new(2).unwrap();
    
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(0.0),
        MidiEvent::ProgramChange {
            channel,
            program: 42,
        },
    ));
    
    let clip_id = project.add_clip(Clip::Midi(clip));
    
    let file = project_to_file(&project);
    
    let midi_events = file.clips[0].midi_events.as_ref().unwrap();
    assert_eq!(midi_events.len(), 1);
    assert_eq!(midi_events[0].event_type, "program_change");
    assert_eq!(midi_events[0].program, Some(42));
    assert_eq!(midi_events[0].channel, 2);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_pitch_bend() {
    let mut project = Project::new("Test Project");
    
    let mut clip = MidiClip::new("PB Test", Beats(4.0));
    let channel = Channel::new(3).unwrap();
    
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(1.5),
        MidiEvent::PitchBend {
            channel,
            value: 8191, // Maximum bend
        },
    ));
    
    let clip_id = project.add_clip(Clip::Midi(clip));
    
    let file = project_to_file(&project);
    
    let midi_events = file.clips[0].midi_events.as_ref().unwrap();
    assert_eq!(midi_events.len(), 1);
    assert_eq!(midi_events[0].event_type, "pitch_bend");
    assert_eq!(midi_events[0].pitch_bend, Some(8191));
    assert_eq!(midi_events[0].channel, 3);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_aftertouch() {
    let mut project = Project::new("Test Project");
    
    let mut clip = MidiClip::new("AT Test", Beats(4.0));
    let channel = Channel::new(4).unwrap();
    
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(2.0),
        MidiEvent::Aftertouch {
            channel,
            pressure: 100,
        },
    ));
    
    let clip_id = project.add_clip(Clip::Midi(clip));
    
    let file = project_to_file(&project);
    
    let midi_events = file.clips[0].midi_events.as_ref().unwrap();
    assert_eq!(midi_events.len(), 1);
    assert_eq!(midi_events[0].event_type, "aftertouch");
    assert_eq!(midi_events[0].pressure, Some(100));
    assert_eq!(midi_events[0].channel, 4);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_with_poly_aftertouch() {
    let mut project = Project::new("Test Project");
    
    let mut clip = MidiClip::new("PAT Test", Beats(4.0));
    let channel = Channel::new(5).unwrap();
    let note = Note::new(64).unwrap();
    
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(3.0),
        MidiEvent::PolyAftertouch {
            channel,
            note,
            pressure: 80,
        },
    ));
    
    let clip_id = project.add_clip(Clip::Midi(clip));
    
    let file = project_to_file(&project);
    
    let midi_events = file.clips[0].midi_events.as_ref().unwrap();
    assert_eq!(midi_events.len(), 1);
    assert_eq!(midi_events[0].event_type, "poly_aftertouch");
    assert_eq!(midi_events[0].note, Some(64));
    assert_eq!(midi_events[0].pressure, Some(80));
    assert_eq!(midi_events[0].channel, 5);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_json_basic() {
    let project = Project::new("Test Project");
    let json = project_to_json(&project).unwrap();
    
    // Verify it's valid JSON
    assert!(json.contains("\"version\""));
    assert!(json.contains("\"meta\""));
    assert!(json.contains("\"tracks\""));
    assert!(json.contains("\"clips\""));
    assert!(json.contains("Test Project"));
}

#[cfg(feature = "serde")]
#[test]
fn project_to_json_round_trip() {
    let mut project = Project::new("Round Trip Test");
    project.meta.author = "Test Author".to_string();
    
    let track_id = project.add_track(TrackType::Midi, "Test Track");
    
    let mut clip = MidiClip::new("Test Clip", Beats(8.0));
    let channel = Channel::new(0).unwrap();
    let note = Note::new(60).unwrap();
    let velocity = Velocity::new(100).unwrap();
    
    clip.sequence.add_event(TimestampedEvent::new(
        Beats(0.0),
        MidiEvent::note_on(channel, note, velocity),
    ));
    
    let clip_id = project.add_clip(Clip::Midi(clip));
    project.place_clip(track_id, clip_id, Beats(4.0)).unwrap();
    
    let json = project_to_json(&project).unwrap();
    
    // Parse it back to verify structure
    let parsed: ProjectFile = serde_json::from_str(&json).unwrap();
    
    assert_eq!(parsed.version, FORMAT_VERSION);
    assert_eq!(parsed.meta.name, "Round Trip Test");
    assert_eq!(parsed.meta.author, "Test Author");
    assert_eq!(parsed.tracks.len(), 2); // Test track + master
    assert_eq!(parsed.clips.len(), 1);
    assert_eq!(parsed.clips[0].name, "Test Clip");
    assert_eq!(parsed.clips[0].length, 8.0);
    
    let test_track = parsed.tracks.iter().find(|t| t.name == "Test Track").unwrap();
    assert_eq!(test_track.arrangement_clips.len(), 1);
    assert_eq!(test_track.arrangement_clips[0].position, 4.0);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_multiple_arrangement_clips() {
    let mut project = Project::new("Test Project");
    let track_id = project.add_track(TrackType::Midi, "Track");
    
    let clip1 = MidiClip::new("Clip 1", Beats(4.0));
    let clip1_id = project.add_clip(Clip::Midi(clip1));
    
    let clip2 = MidiClip::new("Clip 2", Beats(2.0));
    let clip2_id = project.add_clip(Clip::Midi(clip2));
    
    project.place_clip(track_id, clip1_id, Beats(0.0)).unwrap();
    project.place_clip(track_id, clip2_id, Beats(8.0)).unwrap();
    
    let file = project_to_file(&project);
    
    let track = file.tracks.iter().find(|t| t.name == "Track").unwrap();
    assert_eq!(track.arrangement_clips.len(), 2);
    assert_eq!(track.arrangement_clips[0].position, 0.0);
    assert_eq!(track.arrangement_clips[1].position, 8.0);
}

#[cfg(feature = "serde")]
#[test]
fn project_to_file_audio_clip_no_midi_events() {
    use ondeks_core::project::AudioClip;
    
    let mut project = Project::new("Test Project");
    
    let pool_id = AudioPoolId::generate();
    let audio_clip = AudioClip::new("Audio Clip", pool_id, Beats(16.0));
    let clip_id = project.add_clip(Clip::Audio(audio_clip));
    
    let file = project_to_file(&project);
    
    assert_eq!(file.clips.len(), 1);
    assert_eq!(file.clips[0].clip_type, "audio");
    assert_eq!(file.clips[0].midi_events, None);
}
