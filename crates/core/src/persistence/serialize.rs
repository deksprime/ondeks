//! Project serialization functions.

use crate::project::Project;
use super::format::*;

fn track_type_to_str(t: &crate::project::TrackType) -> &'static str {
    match t {
        crate::project::TrackType::Audio => "Audio",
        crate::project::TrackType::Midi => "Midi",
        crate::project::TrackType::Group => "Group",
        crate::project::TrackType::Return => "Return",
        crate::project::TrackType::Master => "Master",
    }
}

fn color_to_data(c: &crate::Color) -> ColorData {
    ColorData { r: c.r, g: c.g, b: c.b }
}

/// Convert a project to a [`ProjectFile`] structure.
pub fn project_to_file(project: &Project) -> ProjectFile {
    ProjectFile {
        version: CURRENT_FORMAT_VERSION,
        meta: ProjectMetaData {
            name: project.meta.name.clone(),
            author: project.meta.author.clone(),
        },
        tempo: project.tempo_map.default_tempo(),
        time_signature: (
            project.time_signature.numerator,
            project.time_signature.denominator,
        ),
        tracks: project
            .tracks()
            .iter()
            .map(|t| TrackData {
                id: t.id.raw(),
                name: t.name.clone(),
                track_type: track_type_to_str(&t.track_type).to_string(),
                volume_db: t.volume_db,
                pan: t.pan,
                muted: t.muted,
                soloed: t.soloed,
                armed: t.armed,
                color: color_to_data(&t.color),
                instrument: t.instrument.map(|n| n.raw()),
                channel_strip: t.channel_strip.map(|n| n.raw()),
                session_slots: t
                    .session_slots
                    .iter()
                    .map(|s| s.map(|c| c.raw()))
                    .collect(),
                sends: t
                    .sends
                    .iter()
                    .map(|(k, v)| (k.raw(), *v))
                    .collect(),
                parent: t.parent.map(|p| p.raw()),
                arrangement_clips: t
                    .arrangement_clips
                    .iter()
                    .map(|c| ArrangementClipData {
                        clip_id: c.clip_id.raw(),
                        position: c.position.0,
                        length: c.length.0,
                    })
                    .collect(),
            })
            .collect(),
        clips: project
            .clips()
            .map(|c| {
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
                    notes: None,
                };

                if let crate::project::Clip::Midi(midi_clip) = c {
                    // Notes (primary editable surface).
                    let notes: Vec<NoteData> = midi_clip
                        .notes
                        .iter()
                        .map(|n| NoteData {
                            time: n.time.0,
                            length: n.length.0,
                            pitch: n.pitch.raw(),
                            velocity: n.velocity.raw(),
                            channel: n.channel.raw(),
                        })
                        .collect();
                    clip_data.notes = Some(notes);

                    // Sequence events (control change / pitch bend / etc.).
                    let events: Vec<MidiEventData> = midi_clip
                        .sequence
                        .events()
                        .iter()
                        .map(|e| midi_event_to_data(e.time.0, &e.event))
                        .collect();
                    clip_data.midi_events = Some(events);
                }

                clip_data
            })
            .collect(),
        scenes: project
            .scenes()
            .iter()
            .map(|s| SceneData {
                id: s.id.raw(),
                name: s.name.clone(),
                tempo: s.tempo,
            })
            .collect(),
    }
}

/// Convert a project to JSON string.
pub fn project_to_json(project: &Project) -> Result<String, serde_json::Error> {
    let file = project_to_file(project);
    serde_json::to_string_pretty(&file)
}

fn midi_event_to_data(time: f64, event: &crate::midi::MidiEvent) -> MidiEventData {
    use crate::midi::MidiEvent::*;
    match event {
        NoteOn { channel, note, velocity } => MidiEventData {
            time,
            event_type: "note_on".to_string(),
            channel: channel.raw(),
            note: Some(note.raw()),
            velocity: Some(velocity.raw()),
            controller: None,
            value: None,
            program: None,
            pitch_bend: None,
            pressure: None,
        },
        NoteOff { channel, note, velocity } => MidiEventData {
            time,
            event_type: "note_off".to_string(),
            channel: channel.raw(),
            note: Some(note.raw()),
            velocity: Some(velocity.raw()),
            controller: None,
            value: None,
            program: None,
            pitch_bend: None,
            pressure: None,
        },
        ControlChange { channel, controller, value } => MidiEventData {
            time,
            event_type: "control_change".to_string(),
            channel: channel.raw(),
            note: None,
            velocity: None,
            controller: Some(*controller),
            value: Some(*value),
            program: None,
            pitch_bend: None,
            pressure: None,
        },
        ProgramChange { channel, program } => MidiEventData {
            time,
            event_type: "program_change".to_string(),
            channel: channel.raw(),
            note: None,
            velocity: None,
            controller: None,
            value: None,
            program: Some(*program),
            pitch_bend: None,
            pressure: None,
        },
        PitchBend { channel, value } => MidiEventData {
            time,
            event_type: "pitch_bend".to_string(),
            channel: channel.raw(),
            note: None,
            velocity: None,
            controller: None,
            value: None,
            program: None,
            pitch_bend: Some(*value),
            pressure: None,
        },
        Aftertouch { channel, pressure } => MidiEventData {
            time,
            event_type: "aftertouch".to_string(),
            channel: channel.raw(),
            note: None,
            velocity: None,
            controller: None,
            value: None,
            program: None,
            pitch_bend: None,
            pressure: Some(*pressure),
        },
        PolyAftertouch { channel, note, pressure } => MidiEventData {
            time,
            event_type: "poly_aftertouch".to_string(),
            channel: channel.raw(),
            note: Some(note.raw()),
            velocity: None,
            controller: None,
            value: None,
            program: None,
            pitch_bend: None,
            pressure: Some(*pressure),
        },
    }
}
