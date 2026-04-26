//! Audio engine: owns the graph, processor, and transport. Exposes a single
//! `process()` entry point that the runtime's audio callback invokes each block.

use crate::command::Command;
use crate::dsp::StereoBuffer;
use crate::graph::{AudioGraph, AudioNode, GraphProcessor, PortAddress, ProcessContext};
use crate::graph::nodes::SynthNode;
use crate::ids::NodeId;
use crate::transport::Transport;

/// The audio engine. One instance lives on the audio thread.
pub struct Engine {
    graph: AudioGraph,
    processor: GraphProcessor,
    transport: Transport,
    sample_rate: u32,
}

impl Engine {
    /// Create a new engine with an **empty** graph — just the master output
    /// node. Instrument nodes are added on demand via `Command::AddSynthNode`
    /// as the UI creates MIDI tracks (Slice 5+).
    pub fn new(sample_rate: u32, buffer_size: usize) -> Self {
        let graph = AudioGraph::new();

        Self {
            graph,
            processor: GraphProcessor::new(buffer_size),
            transport: Transport::new(sample_rate),
            sample_rate,
        }
    }

    /// Process one block of audio, writing to the given stereo output buffer.
    /// Advances the transport by `frames` samples.
    pub fn process(&mut self, output: &mut StereoBuffer, frames: u32) {
        let ctx = ProcessContext {
            buffer_size: frames as usize,
            sample_rate: self.sample_rate,
            tempo: self.transport.tempo(),
            position: self.transport.position(),
            is_playing: self.transport.is_playing(),
        };

        let result = self.processor.process(&mut self.graph, &ctx, &[]);
        output.copy_from(result);

        if self.transport.is_playing() {
            self.transport = self.transport.advance(frames as u64);
        }
    }

    /// Apply a command to engine state. Called by the audio callback after
    /// draining the command queue.
    pub fn apply_command(&mut self, command: Command) {
        match command {
            Command::Play => {
                self.transport = self.transport.play();
            }
            Command::Stop => {
                self.transport = self.transport.stop();
            }
            Command::SetTempo(bpm) => {
                self.transport = self.transport.set_tempo(bpm);
            }
            Command::SendMidi { target, event, sample_offset } => {
                if let Some(node) = self.graph.get_node_mut(target) {
                    node.handle_midi(&event, sample_offset);
                }
            }
            Command::AddSynthNode { node_id, track_id: _ } => {
                self.add_synth_node(node_id);
            }
            Command::RemoveSynthNode { node_id } => {
                self.remove_synth_node(node_id);
            }
        }
    }

    /// Construct a `SynthNode` with the given id and wire its mono output to
    /// both master inputs (L + R). No-op if the id already exists.
    fn add_synth_node(&mut self, node_id: NodeId) {
        if self.graph.get_node(node_id).is_some() {
            return;
        }
        let synth = SynthNode::with_id(node_id, self.sample_rate);
        let synth_out_port = synth.outputs()[0].id;
        self.graph.add_node_with_id(node_id, Box::new(synth));

        let output_id = self.graph.output();
        let (left_in, right_in) = {
            let output_node = self
                .graph
                .get_node(output_id)
                .expect("output node must exist");
            let inputs = output_node.inputs();
            (inputs[0].id, inputs[1].id)
        };

        // Connection errors at this point indicate a bug (duplicate ports,
        // cycle detected, etc.) — log and continue rather than panic so the
        // audio thread stays alive.
        let _ = self.graph.connect(
            PortAddress::new(node_id, synth_out_port),
            PortAddress::new(output_id, left_in),
        );
        let _ = self.graph.connect(
            PortAddress::new(node_id, synth_out_port),
            PortAddress::new(output_id, right_in),
        );
    }

    /// Remove a node and all its connections. No-op if absent.
    fn remove_synth_node(&mut self, node_id: NodeId) {
        let _ = self.graph.remove_node(node_id);
    }

    /// Change the buffer size used by the processor. Typically called from
    /// the audio thread when CPAL delivers a different block size.
    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.processor.set_buffer_size(buffer_size);
    }

    /// Get the transport state.
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    /// Get the current sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
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
        // Conservative defaults: CD sample rate, typical block size.
        Self::new(44100, 512)
    }
}
