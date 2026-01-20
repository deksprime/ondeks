use thiserror::Error;
use crate::ids::*;
use crate::graph::PortType;
use crate::transport::Beats;

/// Errors that can occur in audio graph operations.
#[derive(Debug, Error)]
pub enum GraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    #[error("Port not found on node {node}: {port}")]
    PortNotFound { node: NodeId, port: PortId },

    #[error("Connection would create a cycle in the audio graph")]
    CycleDetected,

    #[error("Port type mismatch: expected {expected:?}, got {got:?}")]
    PortTypeMismatch { expected: PortType, got: PortType },

    #[error("Port is already connected")]
    PortAlreadyConnected,

    #[error("Buffer size mismatch: expected {expected}, got {got}")]
    BufferSizeMismatch { expected: usize, got: usize },

    #[error("Cannot remove the output node")]
    CannotRemoveOutput,

    #[error("Connection not found")]
    ConnectionNotFound,
}

/// Errors that can occur in project operations.
#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("Track not found: {0}")]
    TrackNotFound(TrackId),

    #[error("Clip not found: {0}")]
    ClipNotFound(ClipId),

    #[error("Scene not found: {0}")]
    SceneNotFound(SceneId),

    #[error("Invalid time range: {start} to {end}")]
    InvalidTimeRange { start: Beats, end: Beats },

    #[error("Cannot remove master track")]
    CannotRemoveMaster,

    #[error("Track type mismatch: cannot place {clip_type} clip on {track_type} track")]
    TrackTypeMismatch { clip_type: String, track_type: String },
}

/// Errors that can occur in MIDI operations.
#[derive(Debug, Error)]
pub enum MidiError {
    #[error("Invalid MIDI channel: {0} (must be 0-15)")]
    InvalidChannel(u8),

    #[error("Invalid MIDI note: {0} (must be 0-127)")]
    InvalidNote(u8),

    #[error("Invalid MIDI velocity: {0} (must be 0-127)")]
    InvalidVelocity(u8),

    #[error("Invalid controller number: {0} (must be 0-127)")]
    InvalidController(u8),
}

/// Errors that can occur in automation operations.
#[derive(Debug, Error)]
pub enum AutomationError {
    #[error("Automation lane not found: {0}")]
    LaneNotFound(AutomationLaneId),

    #[error("Parameter not found: {0}")]
    ParameterNotFound(ParameterId),

    #[error("Invalid automation value: {value} (must be between {min} and {max})")]
    InvalidValue { value: f32, min: f32, max: f32 },
}

/// Top-level error type for the core engine.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Graph error: {0}")]
    Graph(#[from] GraphError),

    #[error("Project error: {0}")]
    Project(#[from] ProjectError),

    #[error("MIDI error: {0}")]
    Midi(#[from] MidiError),

    #[error("Automation error: {0}")]
    Automation(#[from] AutomationError),
}

/// Convenience type alias.
pub type CoreResult<T> = Result<T, CoreError>;
