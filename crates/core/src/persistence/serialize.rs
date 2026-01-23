//! Project serialization functions.

use crate::project::Project;
use super::format::*;

/// Convert a project to a ProjectFile structure.
pub fn project_to_file(project: &Project) -> ProjectFile {
    ProjectFile {
        version: FORMAT_VERSION,
        meta: ProjectMetaData {
            name: project.meta.name.clone(),
            author: project.meta.author.clone(),
        },
        tempo: project.tempo_map.default_tempo(),
        time_signature: (
            project.time_signature.numerator,
            project.time_signature.denominator,
        ),
        tracks: project.tracks().iter().map(|t| TrackData {
            id: t.id.raw(),
            name: t.name.clone(),
            track_type: format!("{:?}", t.track_type),
            volume_db: t.volume_db,
            pan: t.pan,
            muted: t.muted,
            arrangement_clips: t.arrangement_clips.iter().map(|c| ArrangementClipData {
                clip_id: c.clip_id.raw(),
                position: c.position.0,
                length: c.length.0,
            }).collect(),
        }).collect(),
        clips: project.clips().map(|c| {
            let header = c.header();
            let mut clip_data = ClipData {
                id: header.id.raw(),
                name: header.name.clone(),
                clip_type: match c {
                    crate::project::Clip::Midi(_) => "midi".to_string(),
                    crate::project::Clip::Audio(_) => "audio".to_string(),
                },
                length: header.length.0,
                midi_events: None,
            };
            
            // Serialize MIDI events if this is a MIDI clip
            if let crate::project::Clip::Midi(midi_clip) = c {
                let events: Vec<MidiEventData> = midi_clip.sequence.events().iter().map(|e| {
                    match &e.event {
                        crate::midi::MidiEvent::NoteOn { channel, note, velocity } => {
                            MidiEventData {
                                time: e.time.0,
                                event_type: "note_on".to_string(),
                                channel: channel.raw(),
                                note: Some(note.raw()),
                                velocity: Some(velocity.raw()),
                                controller: None,
                                value: None,
                                program: None,
                                pitch_bend: None,
                                pressure: None,
                            }
                        }
                        crate::midi::MidiEvent::NoteOff { channel, note, velocity } => {
                            MidiEventData {
                                time: e.time.0,
                                event_type: "note_off".to_string(),
                                channel: channel.raw(),
                                note: Some(note.raw()),
                                velocity: Some(velocity.raw()),
                                controller: None,
                                value: None,
                                program: None,
                                pitch_bend: None,
                                pressure: None,
                            }
                        }
                        crate::midi::MidiEvent::ControlChange { channel, controller, value } => {
                            MidiEventData {
                                time: e.time.0,
                                event_type: "control_change".to_string(),
                                channel: channel.raw(),
                                note: None,
                                velocity: None,
                                controller: Some(*controller),
                                value: Some(*value),
                                program: None,
                                pitch_bend: None,
                                pressure: None,
                            }
                        }
                        crate::midi::MidiEvent::ProgramChange { channel, program } => {
                            MidiEventData {
                                time: e.time.0,
                                event_type: "program_change".to_string(),
                                channel: channel.raw(),
                                note: None,
                                velocity: None,
                                controller: None,
                                value: None,
                                program: Some(*program),
                                pitch_bend: None,
                                pressure: None,
                            }
                        }
                        crate::midi::MidiEvent::PitchBend { channel, value } => {
                            MidiEventData {
                                time: e.time.0,
                                event_type: "pitch_bend".to_string(),
                                channel: channel.raw(),
                                note: None,
                                velocity: None,
                                controller: None,
                                value: None,
                                program: None,
                                pitch_bend: Some(*value),
                                pressure: None,
                            }
                        }
                        crate::midi::MidiEvent::Aftertouch { channel, pressure } => {
                            MidiEventData {
                                time: e.time.0,
                                event_type: "aftertouch".to_string(),
                                channel: channel.raw(),
                                note: None,
                                velocity: None,
                                controller: None,
                                value: None,
                                program: None,
                                pitch_bend: None,
                                pressure: Some(*pressure),
                            }
                        }
                        crate::midi::MidiEvent::PolyAftertouch { channel, note, pressure } => {
                            MidiEventData {
                                time: e.time.0,
                                event_type: "poly_aftertouch".to_string(),
                                channel: channel.raw(),
                                note: Some(note.raw()),
                                velocity: None,
                                controller: None,
                                value: None,
                                program: None,
                                pitch_bend: None,
                                pressure: Some(*pressure),
                            }
                        }
                    }
                }).collect();
                clip_data.midi_events = Some(events);
            }
            
            clip_data
        }).collect(),
    }
}

/// Convert a project to JSON string.
pub fn project_to_json(project: &Project) -> Result<String, serde_json::Error> {
    let file = project_to_file(project);
    serde_json::to_string_pretty(&file)
}
