//! Audio graph system.
//!
//! This module contains the node-based audio processing graph.
//!
//! Note: Full implementation is in Phase 2. This is a minimal stub for Phase 1.

/// The type of data a port carries.
///
/// This is a minimal stub for Phase 1. Full implementation in Phase 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    /// Audio signal (buffer of samples).
    Audio,
    /// MIDI events.
    Midi,
    /// Control value (single f32, for automation).
    Control,
}
