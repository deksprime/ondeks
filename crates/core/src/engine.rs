//! Audio engine that processes the audio graph.

use crate::graph::AudioGraph;

/// The main audio engine.
pub struct Engine {
    graph: AudioGraph,
}

impl Engine {
    /// Create a new engine with an empty graph.
    pub fn new() -> Self {
        Self {
            graph: AudioGraph::new(),
        }
    }

    /// Get a mutable reference to the audio graph.
    pub fn graph_mut(&mut self) -> &mut AudioGraph {
        &mut self.graph
    }

    /// Get a reference to the audio graph.
    pub fn graph(&self) -> &AudioGraph {
        &self.graph
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}
