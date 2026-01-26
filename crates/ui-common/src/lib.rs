//! Shared UI models for Ondeks DAW.
//!
//! This crate provides UI-agnostic view models, selection state,
//! and command dispatching shared across egui, TUI, and web interfaces.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod types;
pub mod selection;
pub mod traits;
pub mod viewmodels;
pub mod commands;
pub mod history;
pub mod shortcuts;
pub mod theme;
pub mod preferences;

// Re-exports
pub use types::*;
pub use selection::*;
pub use traits::*;
pub use viewmodels::*;
pub use commands::UiCommand;
pub use history::{History, HistoryEntry};
pub use shortcuts::{Shortcut, ShortcutMap, ShortcutAction};
pub use theme::{Theme, ThemeColor};
pub use preferences::Preferences;
