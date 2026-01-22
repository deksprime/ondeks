//! Mixer system (channel strips, sends, metering).

mod channel_strip;
mod send;
mod mixer;

pub use channel_strip::{ChannelStrip, MeterState};
pub use send::{Send, SendMatrix};
pub use mixer::Mixer;
