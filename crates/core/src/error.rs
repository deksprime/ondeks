use thiserror::Error;
use crate::ids::*;
use crate::graph::PortType;
use crate::transport::Beats;
use crate::plugin::PluginError;

/// Errors that can occur in audio graph operations.
#[derive(Debug, Error)]
pub enum GraphError {
    /// The specified node does not exist in the graph.
    #[error("Node not found: {0}")]
    NodeNotFound(NodeId),

    /// The specified port does not exist on the given node.
    #[error("Port not found on node {node}: {port}")]
    PortNotFound {
        /// The node ID where the port was expected.
        node: NodeId,
        /// The port ID that was not found.
        port: PortId,
    },

    /// Attempting to create this connection would create a cycle in the graph.
    #[error("Connection would create a cycle in the audio graph")]
    CycleDetected,

    /// The port types do not match (e.g., trying to connect audio to MIDI).
    #[error("Port type mismatch: expected {expected:?}, got {got:?}")]
    PortTypeMismatch {
        /// The expected port type.
        expected: PortType,
        /// The actual port type.
        got: PortType,
    },

    /// The port is already connected and cannot accept another connection.
    #[error("Port is already connected")]
    PortAlreadyConnected,

    /// Buffer sizes do not match between source and destination.
    #[error("Buffer size mismatch: expected {expected}, got {got}")]
    BufferSizeMismatch {
        /// Expected buffer size.
        expected: usize,
        /// Actual buffer size.
        got: usize,
    },

    /// Attempted to remove the output node, which is not allowed.
    #[error("Cannot remove the output node")]
    CannotRemoveOutput,

    /// The specified connection does not exist.
    #[error("Connection not found")]
    ConnectionNotFound,
}

/// Errors that can occur in project operations.
#[derive(Debug, Error)]
pub enum ProjectError {
    /// The specified track does not exist.
    #[error("Track not found: {0}")]
    TrackNotFound(TrackId),

    /// The specified clip does not exist.
    #[error("Clip not found: {0}")]
    ClipNotFound(ClipId),

    /// The specified scene does not exist.
    #[error("Scene not found: {0}")]
    SceneNotFound(SceneId),

    /// The time range is invalid (e.g., end before start).
    #[error("Invalid time range: {start} to {end}")]
    InvalidTimeRange {
        /// Start time of the range.
        start: Beats,
        /// End time of the range.
        end: Beats,
    },

    /// Attempted to remove the master track, which is not allowed.
    #[error("Cannot remove master track")]
    CannotRemoveMaster,

    /// The clip type is incompatible with the track type.
    #[error("Track type mismatch: cannot place {clip_type} clip on {track_type} track")]
    TrackTypeMismatch {
        /// Type of the clip being placed.
        clip_type: String,
        /// Type of the target track.
        track_type: String,
    },

    /// The requested operation is not yet implemented for this command variant.
    #[error("Unsupported project operation: {0}")]
    Unsupported(String),
}

/// Errors that can occur in MIDI operations.
#[derive(Debug, Error)]
pub enum MidiError {
    /// MIDI channel is out of valid range (0-15).
    #[error("Invalid MIDI channel: {0} (must be 0-15)")]
    InvalidChannel(u8),

    /// MIDI note number is out of valid range (0-127).
    #[error("Invalid MIDI note: {0} (must be 0-127)")]
    InvalidNote(u8),

    /// MIDI velocity is out of valid range (0-127).
    #[error("Invalid MIDI velocity: {0} (must be 0-127)")]
    InvalidVelocity(u8),

    /// MIDI controller number is out of valid range (0-127).
    #[error("Invalid controller number: {0} (must be 0-127)")]
    InvalidController(u8),
}

/// Errors that can occur in automation operations.
#[derive(Debug, Error)]
pub enum AutomationError {
    /// The specified automation lane does not exist.
    #[error("Automation lane not found: {0}")]
    LaneNotFound(AutomationLaneId),

    /// The specified parameter does not exist.
    #[error("Parameter not found: {0}")]
    ParameterNotFound(ParameterId),

    /// Automation value is outside the valid range for the parameter.
    #[error("Invalid automation value: {value} (must be between {min} and {max})")]
    InvalidValue {
        /// The invalid value that was provided.
        value: f32,
        /// Minimum allowed value.
        min: f32,
        /// Maximum allowed value.
        max: f32,
    },
}

/// Top-level error type for the core engine.
#[derive(Debug, Error)]
pub enum CoreError {
    /// An error occurred in the audio graph.
    #[error("Graph error: {0}")]
    Graph(#[from] GraphError),

    /// An error occurred in project operations.
    #[error("Project error: {0}")]
    Project(#[from] ProjectError),

    /// An error occurred in MIDI operations.
    #[error("MIDI error: {0}")]
    Midi(#[from] MidiError),

    /// An error occurred in automation operations.
    #[error("Automation error: {0}")]
    Automation(#[from] AutomationError),

    /// An error occurred in plugin operations.
    #[error("Plugin error: {0}")]
    Plugin(#[from] PluginError),
}

/// Convenience type alias.
pub type CoreResult<T> = Result<T, CoreError>;
