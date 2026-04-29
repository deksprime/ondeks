//! File format structures for project serialization.
//!
//! Versioning: every persisted file carries `version: u32`. The framework in
//! [`migrate`](super::migrate) chains migrations from older versions up to
//! [`CURRENT_FORMAT_VERSION`]. Adding a new field that's optional in the
//! existing version stays at the current version; removing or renaming
//! fields, or adding required fields, bumps the version and adds a migration.

use serde::{Deserialize, Serialize};

/// The current format version. Bump on every breaking change and add a
/// migration in `persistence::migrate`.
pub const CURRENT_FORMAT_VERSION: u32 = 1;

/// Backwards-compatible alias for the old name. New code should reference
/// [`CURRENT_FORMAT_VERSION`].
#[deprecated(since = "0.2.0", note = "use CURRENT_FORMAT_VERSION")]
pub const FORMAT_VERSION: u32 = CURRENT_FORMAT_VERSION;

/// Top-level project file structure.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectFile {
    /// Format version.
    pub version: u32,
    /// Project metadata.
    pub meta: ProjectMetaData,
    /// Default tempo in BPM.
    pub tempo: f64,
    /// Time signature (numerator, denominator).
    pub time_signature: (u8, u8),
    /// All tracks in the project.
    pub tracks: Vec<TrackData>,
    /// All clips in the project.
    pub clips: Vec<ClipData>,
    /// Scenes (session view rows). Optional for forward-compat with older
    /// files that didn't persist scenes; defaults to a single "Scene 1".
    #[serde(default)]
    pub scenes: Vec<SceneData>,
}

/// Project metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectMetaData {
    /// Project name.
    pub name: String,
    /// Author name.
    pub author: String,
}

/// Color used for UI display (RGB, 0-255 each).
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub struct ColorData {
    /// Red component.
    pub r: u8,
    /// Green component.
    pub g: u8,
    /// Blue component.
    pub b: u8,
}

/// Scene metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct SceneData {
    /// Scene id.
    pub id: u64,
    /// Display name.
    pub name: String,
    /// Optional tempo override.
    #[serde(default)]
    pub tempo: Option<f64>,
}

/// Track data for serialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackData {
    /// Track id.
    pub id: u64,
    /// Track name.
    pub name: String,
    /// Track type as string ("Audio", "Midi", "Group", "Return", "Master").
    pub track_type: String,
    /// Volume in dB.
    pub volume_db: f32,
    /// Pan position (-1.0 to 1.0).
    pub pan: f32,
    /// Whether the track is muted.
    pub muted: bool,
    /// Whether the track is soloed.
    #[serde(default)]
    pub soloed: bool,
    /// Whether the track is armed for recording / live MIDI input.
    #[serde(default)]
    pub armed: bool,
    /// Display color (RGB). Default gray for older files.
    #[serde(default)]
    pub color: ColorData,
    /// Instrument node id (MIDI tracks only). `None` for non-MIDI.
    #[serde(default)]
    pub instrument: Option<u64>,
    /// Channel-strip node id (MIDI tracks only). `None` for non-MIDI.
    #[serde(default)]
    pub channel_strip: Option<u64>,
    /// Session slots (one entry per scene). `None` = empty slot,
    /// `Some(clip_id)` = clip placed in that slot.
    #[serde(default)]
    pub session_slots: Vec<Option<u64>>,
    /// Send levels keyed by destination track id. Empty for older files.
    #[serde(default)]
    pub sends: std::collections::HashMap<u64, f32>,
    /// Parent group track id, if any.
    #[serde(default)]
    pub parent: Option<u64>,
    /// Clips placed on the arrangement timeline.
    pub arrangement_clips: Vec<ArrangementClipData>,
}

/// Arrangement clip data.
#[derive(Debug, Serialize, Deserialize)]
pub struct ArrangementClipData {
    /// Clip id.
    pub clip_id: u64,
    /// Position in beats.
    pub position: f64,
    /// Length in beats.
    pub length: f64,
}

/// Clip data for serialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClipData {
    /// Clip id.
    pub id: u64,
    /// Clip name.
    pub name: String,
    /// Clip type ("midi" or "audio").
    pub clip_type: String,
    /// Clip length in beats.
    pub length: f64,
    /// MIDI events (only for MIDI clips). Used for control change / pitch
    /// bend / aftertouch on new files; older v1 files use this to encode
    /// notes as `NoteOn`/`NoteOff` pairs which the loader lowers into the
    /// `notes` field if `notes` is absent.
    pub midi_events: Option<Vec<MidiEventData>>,
    /// Editable notes (Slice 8+). Optional so older files without it still
    /// parse; loader prefers `notes` and falls back to lowering
    /// `midi_events` pairs.
    #[serde(default)]
    pub notes: Option<Vec<NoteData>>,
}

/// One editable note in a MIDI clip.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct NoteData {
    /// Start time in beats from the clip start.
    pub time: f64,
    /// Length in beats.
    pub length: f64,
    /// MIDI pitch (0-127).
    pub pitch: u8,
    /// Note-on velocity (0-127).
    pub velocity: u8,
    /// MIDI channel (0-15).
    pub channel: u8,
}

/// MIDI event data for serialization.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MidiEventData {
    /// Time position in beats.
    pub time: f64,
    /// Event type ("note_on", "note_off", "control_change", etc.).
    pub event_type: String,
    /// MIDI channel (0-15).
    pub channel: u8,
    /// MIDI note number (0-127), if applicable.
    pub note: Option<u8>,
    /// Velocity (0-127), if applicable.
    pub velocity: Option<u8>,
    /// Controller number (for control_change), if applicable.
    pub controller: Option<u8>,
    /// Controller value (for control_change), if applicable.
    pub value: Option<u8>,
    /// Program number (for program_change), if applicable.
    pub program: Option<u8>,
    /// Pitch bend value (for pitch_bend), if applicable.
    pub pitch_bend: Option<i16>,
    /// Pressure value (for aftertouch), if applicable.
    pub pressure: Option<u8>,
}
