//! Shared UI models for Ondeks DAW.
//!
//! This crate provides UI-agnostic view models, selection state,
//! and command dispatching shared across egui, TUI, and web interfaces.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod types;
pub mod selection;
pub mod traits;

// Re-exports
pub use types::*;
pub use selection::*;
pub use traits::*;
