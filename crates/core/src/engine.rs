//! Audio engine: owns the graph, processor, and transport. Exposes a single
//! `process()` entry point that the runtime's audio callback invokes each block.

use crate::command::Command;
use crate::dsp::StereoBuffer;
use crate::ids::NodeId;
use crate::graph::{AudioGraph, AudioNode, GraphProcessor, PortAddress, ProcessContext};
use crate::graph::nodes::SynthNode;
use crate::transport::Transport;

/// The audio engine. One instance lives on the audio thread.
pub struct Engine {
    graph: AudioGraph,
    processor: GraphProcessor,
    transport: Transport,
    sample_rate: u32,
    /// Node ID of the built-in synth in the default graph. The UI targets
    /// MIDI at this node. Will be replaced by per-track instrument lookup
    /// in Slice 4 (track CRUD) and beyond.
    synth_node_id: NodeId,
}

impl Engine {
    /// Create a new engine with a default "hello-world" graph: a built-in
    /// polyphonic synth connected to the stereo output (both channels).
    /// The synth responds to MIDI whether transport is playing or not.
    pub fn new(sample_rate: u32, buffer_size: usize) -> Self {
        let mut graph = AudioGraph::new();

        // Default demo graph: synth → output (L and R).
        // Replaced in Slice 4 when users can build their own graphs.
        let synth = SynthNode::new(sample_rate);
        let synth_node_id = synth.id();
        let synth_out_port = synth.outputs()[0].id;
        graph.add_node_with_id(synth_node_id, Box::new(synth));

        let output_id = graph.output();
        let (left_in, right_in) = {
            let output_node = graph
                .get_node(output_id)
                .expect("output node must exist after graph construction");
            let inputs = output_node.inputs();
            (inputs[0].id, inputs[1].id)
        };

        graph
            .connect(
                PortAddress::new(synth_node_id, synth_out_port),
                PortAddress::new(output_id, left_in),
            )
            .expect("default connection (synth -> left) failed");
        graph
            .connect(
                PortAddress::new(synth_node_id, synth_out_port),
                PortAddress::new(output_id, right_in),
            )
            .expect("default connection (synth -> right) failed");

        Self {
            graph,
            processor: GraphProcessor::new(buffer_size),
            transport: Transport::new(sample_rate),
            sample_rate,
            synth_node_id,
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
        }
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

    /// Node ID of the built-in synth in the default graph.
    ///
    /// UI code uses this to target `Command::SendMidi`. Temporary: Slice 4
    /// replaces this with per-track instrument lookup.
    pub fn synth_node_id(&self) -> NodeId {
        self.synth_node_id
    }
}

impl Default for Engine {
    fn default() -> Self {
        // Conservative defaults: CD sample rate, typical block size.
        Self::new(44100, 512)
    }
}
