//! File format structures for project serialization.

use serde::{Serialize, Deserialize};

/// Current format version.
pub const FORMAT_VERSION: u32 = 1;

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
}

/// Project metadata.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectMetaData {
    /// Project name.
    pub name: String,
    /// Author name.
    pub author: String,
}

/// Track data for serialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrackData {
    /// Track ID.
    pub id: u64,
    /// Track name.
    pub name: String,
    /// Track type as string.
    pub track_type: String,
    /// Volume in dB.
    pub volume_db: f32,
    /// Pan position (-1.0 to 1.0).
    pub pan: f32,
    /// Whether the track is muted.
    pub muted: bool,
    /// Clips placed on the arrangement timeline.
    pub arrangement_clips: Vec<ArrangementClipData>,
}

/// Arrangement clip data.
#[derive(Debug, Serialize, Deserialize)]
pub struct ArrangementClipData {
    /// Clip ID.
    pub clip_id: u64,
    /// Position in beats.
    pub position: f64,
    /// Length in beats.
    pub length: f64,
}

/// Clip data for serialization.
#[derive(Debug, Serialize, Deserialize)]
pub struct ClipData {
    /// Clip ID.
    pub id: u64,
    /// Clip name.
    pub name: String,
    /// Clip type ("midi" or "audio").
    pub clip_type: String,
    /// Clip length in beats.
    pub length: f64,
    /// MIDI events (only for MIDI clips).
    pub midi_events: Option<Vec<MidiEventData>>,
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
