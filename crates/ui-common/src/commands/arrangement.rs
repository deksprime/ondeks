//! Arrangement view commands.

use ondeks_core::Command as EngineCommand;
use ondeks_core::{TrackId, ClipId};
use ondeks_core::transport::Beats;
use crate::types::TimeRange;

/// Commands for arrangement view.
#[derive(Debug, Clone)]
pub enum ArrangementCommand {
    /// Place a clip on the arrangement
    PlaceClip { track_id: TrackId, clip_id: ClipId, position: Beats },
    /// Move a clip
    MoveClip { clip_id: ClipId, new_track: TrackId, new_position: Beats },
    /// Resize a clip
    ResizeClip { clip_id: ClipId, new_length: Beats },
    /// Trim clip start
    TrimClipStart { clip_id: ClipId, new_start: Beats },
    /// Split clip at position
    SplitClip { clip_id: ClipId, position: Beats },
    /// Duplicate clip
    DuplicateClipInPlace { clip_id: ClipId },
    /// Delete clip from arrangement
    DeleteClip { clip_id: ClipId, track_id: TrackId },
    /// Mute/unmute clip
    ToggleClipMute { clip_id: ClipId },
    /// Consolidate selection into new clip
    Consolidate { track_id: TrackId, range: TimeRange },
    /// Set time selection
    SetTimeSelection(Option<TimeRange>),
    /// Insert time (push clips right)
    InsertTime { position: Beats, duration: Beats },
    /// Delete time (pull clips left)
    DeleteTime { range: TimeRange },
}

impl ArrangementCommand {
    pub fn to_engine_commands(&self) -> Vec<EngineCommand> {
        // TODO: Implement when EngineCommand is expanded
        vec![]
    }

    pub fn description(&self) -> &str {
        match self {
            Self::PlaceClip { .. } => "Place Clip",
            Self::MoveClip { .. } => "Move Clip",
            Self::ResizeClip { .. } => "Resize Clip",
            Self::TrimClipStart { .. } => "Trim Clip",
            Self::SplitClip { .. } => "Split Clip",
            Self::DuplicateClipInPlace { .. } => "Duplicate Clip",
            Self::DeleteClip { .. } => "Delete Clip",
            Self::ToggleClipMute { .. } => "Toggle Clip Mute",
            Self::Consolidate { .. } => "Consolidate",
            Self::SetTimeSelection(_) => "Set Selection",
            Self::InsertTime { .. } => "Insert Time",
            Self::DeleteTime { .. } => "Delete Time",
        }
    }
}
