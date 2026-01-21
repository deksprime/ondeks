//! Ondeks Runtime — Real-world audio integration
//!
//! This crate handles audio/MIDI IO, threading, and communication
//! between the real-time audio thread and the rest of the application.

pub mod audio;
pub mod midi;
pub mod queue;
pub mod scheduler;

mod error;
mod host;

pub use error::*;
pub use host::{Host, HostError};
