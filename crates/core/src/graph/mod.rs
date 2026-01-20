//! Audio graph system.
//!
//! This module contains the node-based audio processing graph.

mod port;
mod node;
mod connection;
mod audio_graph;
mod topology;
mod processor;

pub mod nodes;

pub use port::{PortType, InputPort, OutputPort};
pub use node::{
    AudioNode, ProcessContext, NodeInputs, NodeOutputs, 
    ParameterDescriptor, ParameterUnit
};
pub use connection::{Connection, PortAddress};
pub use audio_graph::AudioGraph;
pub use processor::GraphProcessor;
