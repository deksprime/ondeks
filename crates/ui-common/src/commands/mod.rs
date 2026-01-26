//! UI commands that map to engine operations.
//!
//! These commands represent user actions in the UI, which get
//! translated to one or more engine commands.

mod transport;
mod project;
mod session;
mod arrangement;
mod editing;

pub use transport::*;
pub use project::*;
pub use session::*;
pub use arrangement::*;
pub use editing::*;

use ondeks_core::Command as EngineCommand;

/// A UI command that can be executed.
#[derive(Debug, Clone)]
pub enum UiCommand {
    Transport(TransportCommand),
    Project(ProjectCommand),
    Session(SessionCommand),
    Arrangement(ArrangementCommand),
    Editing(EditingCommand),
}

impl UiCommand {
    /// Convert to engine commands.
    pub fn to_engine_commands(&self) -> Vec<EngineCommand> {
        match self {
            Self::Transport(cmd) => cmd.to_engine_commands(),
            Self::Project(cmd) => cmd.to_engine_commands(),
            Self::Session(cmd) => cmd.to_engine_commands(),
            Self::Arrangement(cmd) => cmd.to_engine_commands(),
            Self::Editing(cmd) => cmd.to_engine_commands(),
        }
    }

    /// Get a human-readable description for undo/redo.
    pub fn description(&self) -> &str {
        match self {
            Self::Transport(cmd) => cmd.description(),
            Self::Project(cmd) => cmd.description(),
            Self::Session(cmd) => cmd.description(),
            Self::Arrangement(cmd) => cmd.description(),
            Self::Editing(cmd) => cmd.description(),
        }
    }
}
