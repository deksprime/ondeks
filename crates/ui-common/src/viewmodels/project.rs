//! Project and track view models.

use ondeks_core::{TrackId, ClipId, SceneId, Color};
use ondeks_core::project::{Project, Track, TrackType, Scene, Clip};
use ondeks_core::transport::Beats;

/// Summary of a track for UI display.
#[derive(Debug, Clone)]
pub struct TrackViewModel {
    pub id: TrackId,
    pub name: String,
    pub track_type: TrackType,
    pub color: Color,
    pub volume_db: f32,
    pub pan: f32,
    pub muted: bool,
    pub soloed: bool,
    pub armed: bool,
    /// Index in the track list
    pub index: usize,
    /// Number of clips in arrangement
    pub arrangement_clip_count: usize,
    /// Number of filled session slots
    pub session_slot_count: usize,
}

impl TrackViewModel {
    pub fn from_track(track: &Track, index: usize) -> Self {
        Self {
            id: track.id,
            name: track.name.clone(),
            track_type: track.track_type,
            color: track.color,
            volume_db: track.volume_db,
            pan: track.pan,
            muted: track.muted,
            soloed: track.soloed,
            armed: track.armed,
            index,
            arrangement_clip_count: track.arrangement_clips.len(),
            session_slot_count: track.session_slots.iter().filter(|s| s.is_some()).count(),
        }
    }

    /// Get track type icon character.
    pub fn type_icon(&self) -> &'static str {
        match self.track_type {
            TrackType::Audio => "🔊",
            TrackType::Midi => "🎹",
            TrackType::Group => "📁",
            TrackType::Return => "↩",
            TrackType::Master => "🎛",
        }
    }

    /// Get track type label.
    pub fn type_label(&self) -> &'static str {
        match self.track_type {
            TrackType::Audio => "Audio",
            TrackType::Midi => "MIDI",
            TrackType::Group => "Group",
            TrackType::Return => "Return",
            TrackType::Master => "Master",
        }
    }

    /// Format volume as dB string.
    pub fn volume_string(&self) -> String {
        if self.volume_db <= -60.0 {
            "-∞ dB".to_string()
        } else {
            format!("{:+.1} dB", self.volume_db)
        }
    }

    /// Format pan as L/C/R string.
    pub fn pan_string(&self) -> String {
        if self.pan.abs() < 0.01 {
            "C".to_string()
        } else if self.pan < 0.0 {
            format!("L{:.0}", -self.pan * 100.0)
        } else {
            format!("R{:.0}", self.pan * 100.0)
        }
    }
}

/// Summary of a clip for UI display.
#[derive(Debug, Clone)]
pub struct ClipViewModel {
    pub id: ClipId,
    pub name: String,
    pub color: Color,
    pub length_beats: f64,
    pub is_midi: bool,
}

impl ClipViewModel {
    pub fn from_clip(clip: &Clip) -> Self {
        Self {
            id: clip.id(),
            name: clip.header().name.clone(),
            color: clip.header().color,
            length_beats: clip.length().0,
            is_midi: matches!(clip, Clip::Midi(_)),
        }
    }

    /// Format length as bars.beats string.
    pub fn length_string(&self, beats_per_bar: u8) -> String {
        let bars = (self.length_beats / beats_per_bar as f64).floor() as u32;
        let beats = self.length_beats % beats_per_bar as f64;
        format!("{}.{:.1}", bars, beats)
    }
}

/// Summary of a scene for UI display.
#[derive(Debug, Clone)]
pub struct SceneViewModel {
    pub id: SceneId,
    pub name: String,
    pub index: usize,
    pub tempo: Option<f64>,
    pub time_signature: Option<(u8, u8)>,
}

impl SceneViewModel {
    pub fn from_scene(scene: &Scene, index: usize) -> Self {
        Self {
            id: scene.id,
            name: scene.name.clone(),
            index,
            tempo: scene.tempo,
            time_signature: scene.time_signature.map(|ts| (ts.numerator, ts.denominator)),
        }
    }
}

/// Complete project view model.
#[derive(Debug, Clone)]
pub struct ProjectViewModel {
    pub name: String,
    pub author: String,
    pub tempo: f64,
    pub time_signature: (u8, u8),
    pub tracks: Vec<TrackViewModel>,
    pub scenes: Vec<SceneViewModel>,
    pub clip_count: usize,
    /// Whether any track is soloed (affects other tracks' effective mute state)
    pub any_soloed: bool,
}

impl ProjectViewModel {
    pub fn from_project(project: &Project) -> Self {
        let tracks: Vec<_> = project.tracks()
            .iter()
            .enumerate()
            .map(|(i, t)| TrackViewModel::from_track(t, i))
            .collect();

        let any_soloed = tracks.iter().any(|t| t.soloed);

        Self {
            name: project.meta.name.clone(),
            author: project.meta.author.clone(),
            tempo: project.tempo_map.tempo_at(Beats(0.0)),
            time_signature: (
                project.time_signature.numerator,
                project.time_signature.denominator,
            ),
            tracks,
            scenes: project.scenes()
                .iter()
                .enumerate()
                .map(|(i, s)| SceneViewModel::from_scene(s, i))
                .collect(),
            clip_count: project.clips().count(),
            any_soloed,
        }
    }

    /// Get track by ID.
    pub fn get_track(&self, id: TrackId) -> Option<&TrackViewModel> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Get non-master tracks.
    pub fn regular_tracks(&self) -> impl Iterator<Item = &TrackViewModel> {
        self.tracks.iter().filter(|t| t.track_type != TrackType::Master)
    }

    /// Get master track.
    pub fn master_track(&self) -> Option<&TrackViewModel> {
        self.tracks.iter().find(|t| t.track_type == TrackType::Master)
    }

    /// Get return tracks.
    pub fn return_tracks(&self) -> impl Iterator<Item = &TrackViewModel> {
        self.tracks.iter().filter(|t| t.track_type == TrackType::Return)
    }
}
