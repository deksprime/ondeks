//! Session view commands.

use ondeks_core::Command as EngineCommand;
use ondeks_core::{TrackId, ClipId};
use ondeks_core::session::LaunchQuantize;

/// Commands for session view (clip launcher).
#[derive(Debug, Clone)]
pub enum SessionCommand {
    /// Launch a clip
    LaunchClip { track: usize, scene: usize },
    /// Stop clip on a track
    StopTrack { track: usize },
    /// Launch entire scene
    LaunchScene { scene: usize },
    /// Stop all clips
    StopAll,
    /// Set launch quantization
    SetLaunchQuantize(LaunchQuantize),
    /// Create empty clip in slot
    CreateClip { track: usize, scene: usize },
    /// Delete clip from slot
    DeleteClip { track: usize, scene: usize },
    /// Duplicate clip to another slot
    DuplicateClip {
        source_track: usize,
        source_scene: usize,
        dest_track: usize,
        dest_scene: usize,
    },
    /// Record into slot
    RecordIntoSlot { track: usize, scene: usize },
}

impl SessionCommand {
    pub fn to_engine_commands(&self) -> Vec<EngineCommand> {
        // TODO: Implement when EngineCommand is expanded
        vec![]
    }

    pub fn description(&self) -> &str {
        match self {
            Self::LaunchClip { .. } => "Launch Clip",
            Self::StopTrack { .. } => "Stop Track",
            Self::LaunchScene { .. } => "Launch Scene",
            Self::StopAll => "Stop All Clips",
            Self::SetLaunchQuantize(_) => "Set Launch Quantize",
            Self::CreateClip { .. } => "Create Clip",
            Self::DeleteClip { .. } => "Delete Clip",
            Self::DuplicateClip { .. } => "Duplicate Clip",
            Self::RecordIntoSlot { .. } => "Record Clip",
        }
    }
}
