//! Commands that can be sent to the audio engine.

/// Commands that control the audio engine.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Start playback.
    Play,
    /// Stop playback.
    Stop,
    /// Set the tempo in BPM.
    SetTempo(f64),
}
