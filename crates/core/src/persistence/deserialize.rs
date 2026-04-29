//! Project deserialization (`ProjectFile` → `Project`).
//!
//! Used by the GUI's Open path. JSON → version-probe → migrate → typed
//! `ProjectFile` → reconstructed `Project` with all node ids restored.

use std::collections::HashMap;

use thiserror::Error;

use crate::ids::{ClipId, NodeId, SceneId, TrackId};
use crate::midi::{Channel, MidiEvent, MidiSequence, Note, TimestampedEvent, Velocity};
use crate::project::{
    ArrangementClip, Clip, MidiClip, Project, ProjectMeta, Scene, Track, TrackType,
};
use crate::transport::{Beats, TempoMap, TimeSignature};
use crate::Color;

use super::format::{
    ArrangementClipData, ClipData, ColorData, MidiEventData, ProjectFile, SceneData, TrackData,
};
use super::migrate::{migrate_to_current, version_of, MigrateError};

/// Errors that can occur loading a project file.
#[derive(Debug, Error)]
pub enum LoadError {
    /// JSON parsing failed before we could even read the version field.
    #[error("invalid project JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    /// Migration framework rejected the file.
    #[error("migration error: {0}")]
    Migrate(#[from] MigrateError),
    /// A track had a `track_type` string we don't recognise.
    #[error("unknown track type: {0}")]
    UnknownTrackType(String),
    /// A clip had an unknown `clip_type` string.
    #[error("unknown clip type: {0}")]
    UnknownClipType(String),
    /// A MIDI event referenced a value outside the valid MIDI range.
    #[error("invalid MIDI value in clip {clip_id}: {detail}")]
    InvalidMidi {
        /// Clip the bad event belongs to.
        clip_id: u64,
        /// Human-readable description.
        detail: String,
    },
    /// A clip type required fields it didn't have (e.g. midi clip with no events list).
    #[error("malformed clip {0}: {1}")]
    MalformedClip(u64, &'static str),
    /// Project structure invariant violated (e.g. missing master track).
    #[error("project structure: {0}")]
    Project(#[from] crate::ProjectError),
}

/// Deserialize a project from JSON. Walks migrations and reconstructs all
/// node ids so the engine graph can be rebuilt deterministically.
pub fn project_from_json(json: &str) -> Result<Project, LoadError> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    let from_version = version_of(&value)?;
    let file = migrate_to_current(value, from_version)?;
    file_to_project(file)
}

/// Reconstruct a `Project` from a typed `ProjectFile`.
pub fn file_to_project(file: ProjectFile) -> Result<Project, LoadError> {
    let mut project = Project::new(file.meta.name.clone());
    project.meta = ProjectMeta {
        name: file.meta.name,
        author: file.meta.author,
        ..ProjectMeta::default()
    };
    let (numerator, denominator) = file.time_signature;
    project.time_signature =
        TimeSignature::new(numerator, denominator).unwrap_or(TimeSignature::FOUR_FOUR);
    project.tempo_map = TempoMap::constant(file.tempo);

    // Replace the auto-generated default scene with whatever the file holds.
    // `Project::new` always seeds one scene, so we drain it via raw access.
    project.replace_scenes(scenes_from_data(&file.scenes));

    // Tracks: master is auto-created by Project::new; we either replace it or
    // supplement with the rest. Walk loaded tracks; for non-master, insert
    // before master. For the master entry, sync its non-id state.
    let loaded_tracks: Vec<Track> = file
        .tracks
        .into_iter()
        .map(track_from_data)
        .collect::<Result<Vec<_>, _>>()?;
    project.replace_tracks(loaded_tracks)?;

    // Clips.
    for clip_data in file.clips {
        let clip = clip_from_data(&clip_data)?;
        project.add_clip(clip);
    }

    Ok(project)
}

fn track_from_data(data: TrackData) -> Result<Track, LoadError> {
    let track_type = match data.track_type.as_str() {
        "Audio" => TrackType::Audio,
        "Midi" => TrackType::Midi,
        "Group" => TrackType::Group,
        "Return" => TrackType::Return,
        "Master" => TrackType::Master,
        other => return Err(LoadError::UnknownTrackType(other.to_string())),
    };

    let mut track = Track {
        id: TrackId::from_raw(data.id),
        name: data.name,
        track_type,
        color: color_from_data(&data.color),
        volume_db: data.volume_db,
        pan: data.pan,
        muted: data.muted,
        soloed: data.soloed,
        armed: data.armed,
        instrument: data.instrument.map(NodeId::from_raw),
        channel_strip: data.channel_strip.map(NodeId::from_raw),
        arrangement_clips: data
            .arrangement_clips
            .into_iter()
            .map(arrangement_clip_from_data)
            .collect(),
        session_slots: data
            .session_slots
            .into_iter()
            .map(|slot| slot.map(ClipId::from_raw))
            .collect(),
        sends: data
            .sends
            .into_iter()
            .map(|(k, v)| (TrackId::from_raw(k), v))
            .collect::<HashMap<_, _>>(),
        parent: data.parent.map(TrackId::from_raw),
    };

    // Older files might predate the channel_strip field but still be MIDI;
    // grant a fresh strip id so the engine graph can wire one up. Same for
    // instrument. (No-op if both already set.)
    if track.track_type == TrackType::Midi {
        if track.instrument.is_none() {
            track.instrument = Some(NodeId::generate());
        }
        if track.channel_strip.is_none() {
            track.channel_strip = Some(NodeId::generate());
        }
    }

    Ok(track)
}

fn arrangement_clip_from_data(data: ArrangementClipData) -> ArrangementClip {
    ArrangementClip {
        clip_id: ClipId::from_raw(data.clip_id),
        position: Beats(data.position),
        length: Beats(data.length),
        offset: Beats(0.0),
        muted: false,
    }
}

fn scenes_from_data(scenes: &[SceneData]) -> Vec<Scene> {
    if scenes.is_empty() {
        return vec![Scene::new("Scene 1")];
    }
    scenes
        .iter()
        .map(|s| Scene {
            id: SceneId::from_raw(s.id),
            name: s.name.clone(),
            tempo: s.tempo,
            time_signature: None,
        })
        .collect()
}

fn color_from_data(data: &ColorData) -> Color {
    Color::new(data.r, data.g, data.b)
}

fn clip_from_data(data: &ClipData) -> Result<Clip, LoadError> {
    match data.clip_type.as_str() {
        "midi" => {
            let events = data
                .midi_events
                .as_ref()
                .ok_or(LoadError::MalformedClip(data.id, "midi clip missing events"))?;
            let mut clip = MidiClip::new(data.name.clone(), Beats(data.length));
            clip.header.id = ClipId::from_raw(data.id);
            // Replace the default empty sequence with the loaded one.
            let mut sequence = MidiSequence::with_length(Beats(data.length));
            for event in events {
                let parsed = midi_event_from_data(event, data.id)?;
                sequence.add_event(TimestampedEvent::new(Beats(event.time), parsed));
            }
            clip.sequence = sequence;
            Ok(Clip::Midi(clip))
        }
        "audio" => Err(LoadError::MalformedClip(
            data.id,
            "audio clip persistence not implemented yet",
        )),
        other => Err(LoadError::UnknownClipType(other.to_string())),
    }
}

fn midi_event_from_data(data: &MidiEventData, clip_id: u64) -> Result<MidiEvent, LoadError> {
    let bad = |reason: &str| LoadError::InvalidMidi {
        clip_id,
        detail: reason.to_string(),
    };
    let channel = Channel::new(data.channel).map_err(|_| bad("channel out of range"))?;
    match data.event_type.as_str() {
        "note_on" => {
            let note_raw = data.note.ok_or_else(|| bad("note_on missing note"))?;
            let vel_raw = data
                .velocity
                .ok_or_else(|| bad("note_on missing velocity"))?;
            Ok(MidiEvent::NoteOn {
                channel,
                note: Note::new(note_raw).map_err(|_| bad("invalid note"))?,
                velocity: Velocity::new(vel_raw).map_err(|_| bad("invalid velocity"))?,
            })
        }
        "note_off" => {
            let note_raw = data.note.ok_or_else(|| bad("note_off missing note"))?;
            let vel_raw = data.velocity.unwrap_or(0);
            Ok(MidiEvent::NoteOff {
                channel,
                note: Note::new(note_raw).map_err(|_| bad("invalid note"))?,
                velocity: Velocity::new(vel_raw).map_err(|_| bad("invalid velocity"))?,
            })
        }
        "control_change" => Ok(MidiEvent::ControlChange {
            channel,
            controller: data
                .controller
                .ok_or_else(|| bad("control_change missing controller"))?,
            value: data
                .value
                .ok_or_else(|| bad("control_change missing value"))?,
        }),
        "program_change" => Ok(MidiEvent::ProgramChange {
            channel,
            program: data
                .program
                .ok_or_else(|| bad("program_change missing program"))?,
        }),
        "pitch_bend" => Ok(MidiEvent::PitchBend {
            channel,
            value: data
                .pitch_bend
                .ok_or_else(|| bad("pitch_bend missing value"))?,
        }),
        "aftertouch" => Ok(MidiEvent::Aftertouch {
            channel,
            pressure: data
                .pressure
                .ok_or_else(|| bad("aftertouch missing pressure"))?,
        }),
        "poly_aftertouch" => {
            let note_raw = data.note.ok_or_else(|| bad("poly_aftertouch missing note"))?;
            Ok(MidiEvent::PolyAftertouch {
                channel,
                note: Note::new(note_raw).map_err(|_| bad("invalid note"))?,
                pressure: data
                    .pressure
                    .ok_or_else(|| bad("poly_aftertouch missing pressure"))?,
            })
        }
        other => Err(bad(&format!("unknown event type {other}"))),
    }
}
