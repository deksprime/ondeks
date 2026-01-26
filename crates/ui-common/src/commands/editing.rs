//! Editing commands (copy, paste, etc.).

use ondeks_core::Command as EngineCommand;
use ondeks_core::{TrackId, ClipId};

/// General editing commands.
#[derive(Debug, Clone)]
pub enum EditingCommand {
    /// Cut selection
    Cut,
    /// Copy selection
    Copy,
    /// Paste from clipboard
    Paste,
    /// Duplicate selection
    Duplicate,
    /// Delete selection
    Delete,
    /// Select all
    SelectAll,
    /// Deselect all
    DeselectAll,
    /// Undo
    Undo,
    /// Redo
    Redo,
    /// Quantize selected MIDI notes
    Quantize { strength: f32 },
}

impl EditingCommand {
    pub fn to_engine_commands(&self) -> Vec<EngineCommand> {
        // TODO: Implement when EngineCommand is expanded
        vec![]
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Cut => "Cut",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Duplicate => "Duplicate",
            Self::Delete => "Delete",
            Self::SelectAll => "Select All",
            Self::DeselectAll => "Deselect All",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Quantize { .. } => "Quantize",
        }
    }
}
