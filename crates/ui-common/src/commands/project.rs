//! Project management commands.
//!
//! Forward-facing commands (`AddTrack`, `RemoveTrack`, ...) are what the UI
//! originates. Inverse commands (`RestoreTrack`, ...) are what the dispatcher
//! produces after capturing pre-mutation state, pushed into the undo stack.

use ondeks_core::Command as EngineCommand;
use ondeks_core::{TrackId, Color};
use ondeks_core::project::{Track, TrackType};

/// Commands for project management.
#[derive(Debug, Clone)]
pub enum ProjectCommand {
    // --- File ---
    /// Create new project
    New { name: String },
    /// Save project
    Save { path: Option<String> },
    /// Load project
    Load { path: String },

    // --- Forward-facing track CRUD (originated by UI interactions) ---
    /// Add a new track with auto-generated id; append before master.
    AddTrack { track_type: TrackType, name: String },
    /// Remove a track by id.
    RemoveTrack { track_id: TrackId },
    /// Rename a track to `name`.
    RenameTrack { track_id: TrackId, name: String },
    /// Duplicate a track (clones data, generates a fresh id).
    DuplicateTrack { track_id: TrackId },
    /// Move a track to a new index within the non-master range.
    MoveTrack { track_id: TrackId, new_index: usize },
    /// Set a track's display color.
    SetTrackColor { track_id: TrackId, color: Color },
    /// Arm a track exclusively for MIDI input. All other tracks are disarmed.
    /// **Not undoable** — arm is an ephemeral routing flag, like transport
    /// Play/Stop.
    ArmTrack { track_id: TrackId },

    // --- Inverse / internal (produced by the dispatcher for undo) ---
    /// Restore a previously-removed track at its original index. Used as the
    /// undo of `RemoveTrack` and `DuplicateTrack`. Boxed to keep the enum
    /// compact since `Track` is ~hundreds of bytes.
    RestoreTrack { track: Box<Track>, insert_at: usize },

    // --- Scenes ---
    /// Add a scene with the given name.
    AddScene { name: String },
    /// Remove a scene at the given index.
    RemoveScene { index: usize },
    /// Rename a scene at the given index.
    RenameScene { index: usize, name: String },
}

impl ProjectCommand {
    /// Convert to engine commands. Track CRUD is a UI-thread mutation for
    /// Slice 4; engine wiring lands with per-track instruments in Slice 5.
    pub fn to_engine_commands(&self) -> Vec<EngineCommand> {
        vec![]
    }

    /// Short description used for undo labels.
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
            Self::ArmTrack { .. } => "Arm Track",
            Self::RestoreTrack { .. } => "Restore Track",
            Self::AddScene { .. } => "Add Scene",
            Self::RemoveScene { .. } => "Remove Scene",
            Self::RenameScene { .. } => "Rename Scene",
        }
    }
}
