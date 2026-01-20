//! Transport and timing primitives.
//!
//! This module contains types for representing musical time, tempo, and playback state.

mod time;
mod time_signature;

pub use time::{SampleTime, Seconds, Beats, BarBeatTick, PPQN};
pub use time_signature::TimeSignature;
