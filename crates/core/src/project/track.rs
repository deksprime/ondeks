use std::collections::HashMap;
use crate::ids::{TrackId, ClipId, Color, NodeId};
use crate::dsp::Sample;
use crate::transport::Beats;

/// Type of track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackType {
    /// Audio track (plays audio clips).
    Audio,
    /// MIDI track (plays MIDI clips through instruments).
    Midi,
    /// Group track (sums child tracks).
    Group,
    /// Return track (receives sends).
    Return,
    /// Master track (final output).
    Master,
}

/// A clip placed on the arrangement timeline.
#[derive(Debug, Clone)]
pub struct ArrangementClip {
    pub clip_id: ClipId,
    pub position: Beats,
    pub length: Beats,
    pub offset: Beats,
    pub muted: bool,
}

/// A track in the project.
#[derive(Debug, Clone)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub track_type: TrackType,
    pub color: Color,

    // Mixer state
    pub volume_db: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    pub armed: bool,

    /// Graph node id of this track's instrument (MIDI tracks only).
    ///
    /// Assigned when the track is created so the UI dispatcher and the audio
    /// graph share the same id — lets undo/redo preserve node identity the
    /// same way it preserves `TrackId`. `None` for audio, group, return and
    /// master tracks.
    pub instrument: Option<NodeId>,

    // Arrangement clips
    pub arrangement_clips: Vec<ArrangementClip>,

    // Session clip slots (index = scene index, None = empty slot)
    pub session_slots: Vec<Option<ClipId>>,

    // Send levels to return tracks
    pub sends: HashMap<TrackId, f32>,

    // Parent group (if any)
    pub parent: Option<TrackId>,
}

impl Track {
    pub fn new(track_type: TrackType, name: impl Into<String>) -> Self {
        // Allocate a fresh instrument node id up front for MIDI tracks so the
        // graph node and the track share identity.
        let instrument = match track_type {
            TrackType::Midi => Some(NodeId::generate()),
            _ => None,
        };
        Self {
            id: TrackId::generate(),
            name: name.into(),
            track_type,
            color: Color::GRAY,
            volume_db: 0.0,
            pan: 0.0,
            muted: false,
            soloed: false,
            armed: false,
            instrument,
            arrangement_clips: Vec::new(),
            session_slots: Vec::new(),
            sends: HashMap::new(),
            parent: None,
        }
    }

    pub fn audio(name: impl Into<String>) -> Self {
        Self::new(TrackType::Audio, name)
    }

    pub fn midi(name: impl Into<String>) -> Self {
        Self::new(TrackType::Midi, name)
    }

    pub fn group(name: impl Into<String>) -> Self {
        Self::new(TrackType::Group, name)
    }

    pub fn return_track(name: impl Into<String>) -> Self {
        Self::new(TrackType::Return, name)
    }

    pub fn master() -> Self {
        Self::new(TrackType::Master, "Master")
    }

    /// Get effective volume considering mute state.
    pub fn effective_volume(&self) -> Sample {
        if self.muted {
            0.0
        } else {
            crate::dsp::db_to_linear(self.volume_db)
        }
    }

    /// Place a clip on the arrangement.
    pub fn place_clip(&mut self, clip_id: ClipId, position: Beats, length: Beats) {
        self.arrangement_clips.push(ArrangementClip {
            clip_id,
            position,
            length,
            offset: Beats(0.0),
            muted: false,
        });
        // Sort by position
        self.arrangement_clips.sort_by(|a, b| {
            a.position.0.partial_cmp(&b.position.0).unwrap()
        });
    }

    /// Remove a clip from the arrangement.
    pub fn remove_arrangement_clip(&mut self, clip_id: ClipId) {
        self.arrangement_clips.retain(|c| c.clip_id != clip_id);
    }

    /// Set clip in session slot.
    pub fn set_session_slot(&mut self, slot: usize, clip_id: Option<ClipId>) {
        while self.session_slots.len() <= slot {
            self.session_slots.push(None);
        }
        self.session_slots[slot] = clip_id;
    }
}
