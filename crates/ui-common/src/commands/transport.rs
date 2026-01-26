//! Transport-related commands.

use ondeks_core::Command as EngineCommand;
use ondeks_core::transport::Beats;

/// Commands for transport control.
#[derive(Debug, Clone)]
pub enum TransportCommand {
    /// Start playback
    Play,
    /// Stop playback (return to start)
    Stop,
    /// Pause playback (keep position)
    Pause,
    /// Toggle play/pause
    TogglePlay,
    /// Start recording
    Record,
    /// Set tempo
    SetTempo(f64),
    /// Nudge tempo up/down
    NudgeTempo(f64),
    /// Seek to position
    Seek(Beats),
    /// Seek by delta
    SeekDelta(f64),
    /// Set loop region
    SetLoop { start: Beats, end: Beats },
    /// Toggle loop on/off
    ToggleLoop,
    /// Toggle metronome
    ToggleMetronome,
    /// Tap tempo
    TapTempo,
    /// Go to start
    GoToStart,
    /// Go to end
    GoToEnd,
    /// Go to next bar
    NextBar,
    /// Go to previous bar
    PrevBar,
}

impl TransportCommand {
    pub fn to_engine_commands(&self) -> Vec<EngineCommand> {
        match self {
            Self::Play => vec![EngineCommand::Play],
            Self::Stop => vec![EngineCommand::Stop],
            Self::SetTempo(bpm) => vec![EngineCommand::SetTempo(*bpm)],
            // Others would require expanded EngineCommand enum
            _ => vec![], // TODO: Expand EngineCommand
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Play => "Play",
            Self::Stop => "Stop",
            Self::Pause => "Pause",
            Self::TogglePlay => "Toggle Play",
            Self::Record => "Record",
            Self::SetTempo(_) => "Set Tempo",
            Self::NudgeTempo(_) => "Nudge Tempo",
            Self::Seek(_) => "Seek",
            Self::SeekDelta(_) => "Seek",
            Self::SetLoop { .. } => "Set Loop",
            Self::ToggleLoop => "Toggle Loop",
            Self::ToggleMetronome => "Toggle Metronome",
            Self::TapTempo => "Tap Tempo",
            Self::GoToStart => "Go to Start",
            Self::GoToEnd => "Go to End",
            Self::NextBar => "Next Bar",
            Self::PrevBar => "Previous Bar",
        }
    }
}
