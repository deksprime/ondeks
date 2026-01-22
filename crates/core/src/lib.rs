//! Ondeks Core — Pure audio engine logic
//!
//! This crate contains all deterministic, IO-free audio processing logic.
//! It has no dependencies on audio hardware, file systems, or threading.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod arrangement;
pub mod automation;
pub mod dsp;
pub mod graph;
pub mod midi;
pub mod mixer;
pub mod project;
pub mod session;
pub mod transport;

mod command;
mod engine;
mod error;
mod event;
mod ids;
mod state;

pub use command::Command;
pub use engine::Engine;
pub use error::*;
pub use event::Event;
pub use ids::*;
pub use state::State;
