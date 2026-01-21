//! Events emitted by the audio engine.

/// Events emitted by the audio engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Transport state changed.
    TransportStateChanged {
        /// Whether the transport is currently playing.
        is_playing: bool,
    },
}
