use crate::graph::AudioNode;

/// Plugin category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginCategory {
    /// An instrument plugin that generates audio.
    Instrument,
    /// An effect plugin that processes audio.
    Effect,
    /// A MIDI effect plugin that processes MIDI events.
    MidiEffect,
    /// A utility plugin for routing or other purposes.
    Utility,
}

/// Plugin metadata.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// Unique identifier for the plugin.
    pub id: String,
    /// Human-readable name of the plugin.
    pub name: String,
    /// Vendor/author of the plugin.
    pub vendor: String,
    /// Version string of the plugin.
    pub version: String,
    /// Category of the plugin.
    pub category: PluginCategory,
}

/// Error in plugin operations.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    /// The requested plugin was not found.
    #[error("Plugin not found: {0}")]
    NotFound(String),
    /// Failed to load or deserialize plugin state.
    #[error("Failed to load state: {0}")]
    StateError(String),
    /// Invalid parameter value or ID.
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
}

/// A plugin extends AudioNode with persistence.
pub trait Plugin: AudioNode {
    /// Get plugin metadata.
    fn info(&self) -> &PluginInfo;

    /// Serialize plugin state.
    fn get_state(&self) -> Vec<u8>;

    /// Deserialize plugin state.
    fn set_state(&mut self, state: &[u8]) -> Result<(), PluginError>;
}
