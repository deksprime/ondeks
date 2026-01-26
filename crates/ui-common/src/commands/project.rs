//! Project management commands.

use ondeks_core::Command as EngineCommand;
use ondeks_core::{TrackId, Color};
use ondeks_core::project::TrackType;

/// Commands for project management.
#[derive(Debug, Clone)]
pub enum ProjectCommand {
    /// Create new project
    New { name: String },
    /// Save project
    Save { path: Option<String> },
    /// Load project
    Load { path: String },
    /// Add a track
    AddTrack { track_type: TrackType, name: String },
    /// Remove a track
    RemoveTrack { track_id: TrackId },
    /// Rename a track
    RenameTrack { track_id: TrackId, name: String },
    /// Duplicate a track
    DuplicateTrack { track_id: TrackId },
    /// Move track position
    MoveTrack { track_id: TrackId, new_index: usize },
    /// Set track color
    SetTrackColor { track_id: TrackId, color: Color },
    /// Add a scene
    AddScene { name: String },
    /// Remove a scene
    RemoveScene { index: usize },
    /// Rename a scene
    RenameScene { index: usize, name: String },
}

impl ProjectCommand {
    pub fn to_engine_commands(&self) -> Vec<EngineCommand> {
        // TODO: Implement when EngineCommand is expanded
        vec![]
    }

    pub fn description(&self) -> &str {
        match self {
            Self::New { .. } => "New Project",
            Self::Save { .. } => "Save Project",
            Self::Load { .. } => "Load Project",
            Self::AddTrack { .. } => "Add Track",
            Self::RemoveTrack { .. } => "Remove Track",
            Self::RenameTrack { .. } => "Rename Track",
            Self::DuplicateTrack { .. } => "Duplicate Track",
            Self::MoveTrack { .. } => "Move Track",
            Self::SetTrackColor { .. } => "Set Track Color",
            Self::AddScene { .. } => "Add Scene",
            Self::RemoveScene { .. } => "Remove Scene",
            Self::RenameScene { .. } => "Rename Scene",
        }
    }
}
