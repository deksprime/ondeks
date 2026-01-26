//! Session view model (clip launcher grid).

use ondeks_core::TrackId;
use ondeks_core::session::{SlotState, LaunchQuantize};
use ondeks_core::project::Project;
use super::project::{TrackViewModel, ClipViewModel};

/// State of a single slot in the session grid.
#[derive(Debug, Clone)]
pub struct SlotViewModel {
    /// Track index
    pub track_index: usize,
    /// Scene index
    pub scene_index: usize,
    /// Track ID
    pub track_id: TrackId,
    /// Current state of the slot
    pub state: SlotState,
    /// Clip in this slot (if any)
    pub clip: Option<ClipViewModel>,
    /// Is this slot selected
    pub is_selected: bool,
}

impl SlotViewModel {
    /// Get display character for slot state.
    pub fn state_icon(&self) -> &'static str {
        match self.state {
            SlotState::Empty => "○",
            SlotState::Stopped => "■",
            SlotState::Queued => "◆",
            SlotState::Playing => "▶",
            SlotState::Recording => "●",
        }
    }

    /// Get color hint for slot state.
    pub fn state_color_hint(&self) -> SlotColorHint {
        match self.state {
            SlotState::Empty => SlotColorHint::Dim,
            SlotState::Stopped => SlotColorHint::Normal,
            SlotState::Queued => SlotColorHint::Warning,
            SlotState::Playing => SlotColorHint::Active,
            SlotState::Recording => SlotColorHint::Recording,
        }
    }
}

/// Color hint for UI rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotColorHint {
    Dim,
    Normal,
    Active,
    Warning,
    Recording,
}

/// Complete session grid view model.
#[derive(Debug, Clone)]
pub struct SessionViewModel {
    /// Number of tracks (columns)
    pub track_count: usize,
    /// Number of scenes (rows)
    pub scene_count: usize,
    /// All slots in the grid (row-major: [scene][track])
    pub slots: Vec<Vec<SlotViewModel>>,
    /// Track headers for the columns
    pub track_headers: Vec<TrackViewModel>,
    /// Scene names for the rows
    pub scene_names: Vec<String>,
    /// Current launch quantization
    pub launch_quantize: LaunchQuantize,
}

impl SessionViewModel {
    /// Build session view from project and launcher state.
    pub fn from_project(
        project: &Project,
        slot_states: impl Fn(usize, usize) -> SlotState,
        selected_slots: impl Fn(usize, usize) -> bool,
    ) -> Self {
        let tracks: Vec<_> = project.tracks()
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(t.track_type, ondeks_core::project::TrackType::Master))
            .map(|(i, t)| TrackViewModel::from_track(t, i))
            .collect();

        let track_count = tracks.len();
        let scene_count = project.scene_count();

        let mut slots = Vec::with_capacity(scene_count);

        for scene_idx in 0..scene_count {
            let mut row = Vec::with_capacity(track_count);

            for (track_idx, track) in tracks.iter().enumerate() {
                // Get clip in this slot (if any)
                let clip = project.tracks()
                    .iter()
                    .find(|t| t.id == track.id)
                    .and_then(|t| t.session_slots.get(scene_idx))
                    .and_then(|slot| slot.as_ref())
                    .and_then(|clip_id| project.get_clip(*clip_id))
                    .map(|c| ClipViewModel::from_clip(c));

                let state = if clip.is_some() {
                    slot_states(track_idx, scene_idx)
                } else {
                    SlotState::Empty
                };

                row.push(SlotViewModel {
                    track_index: track_idx,
                    scene_index: scene_idx,
                    track_id: track.id,
                    state,
                    clip,
                    is_selected: selected_slots(track_idx, scene_idx),
                });
            }

            slots.push(row);
        }

        Self {
            track_count,
            scene_count,
            slots,
            track_headers: tracks,
            scene_names: project.scenes().iter().map(|s| s.name.clone()).collect(),
            launch_quantize: LaunchQuantize::Bar,
        }
    }

    /// Get a specific slot.
    pub fn get_slot(&self, track: usize, scene: usize) -> Option<&SlotViewModel> {
        self.slots.get(scene).and_then(|row| row.get(track))
    }

    /// Get all slots in a scene (row).
    pub fn scene_slots(&self, scene: usize) -> Option<&[SlotViewModel]> {
        self.slots.get(scene).map(|v| v.as_slice())
    }

    /// Get all slots for a track (column).
    pub fn track_slots(&self, track: usize) -> impl Iterator<Item = &SlotViewModel> {
        self.slots.iter().filter_map(move |row| row.get(track))
    }

    /// Find slots that are currently playing.
    pub fn playing_slots(&self) -> impl Iterator<Item = &SlotViewModel> {
        self.slots.iter()
            .flat_map(|row| row.iter())
            .filter(|slot| slot.state == SlotState::Playing)
    }
}
