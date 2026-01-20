use std::collections::HashMap;
use crate::ids::NodeId;
use crate::dsp::{Buffer, StereoBuffer};
use crate::midi::MidiEvent;
use super::audio_graph::AudioGraph;
use super::node::{ProcessContext, NodeInputs, NodeOutputs};

/// Pre-allocated buffers to avoid allocation during processing.
struct BufferPool {
    buffers: Vec<Buffer>,
    buffer_size: usize,
}

impl BufferPool {
    fn new(count: usize, buffer_size: usize) -> Self {
        let buffers = (0..count).map(|_| Buffer::allocate(buffer_size)).collect();
        Self { buffers, buffer_size }
    }

    fn resize(&mut self, count: usize, buffer_size: usize) {
        self.buffer_size = buffer_size;
        self.buffers = (0..count).map(|_| Buffer::allocate(buffer_size)).collect();
    }

    fn get_mut(&mut self, index: usize) -> Option<&mut Buffer> {
        self.buffers.get_mut(index)
    }

    fn clear_all(&mut self) {
        for buf in &mut self.buffers {
            buf.silence();
        }
    }
}

/// Processes an AudioGraph for one buffer cycle.
pub struct GraphProcessor {
    buffer_pool: BufferPool,
    /// Maps (node_id, output_port_index) -> buffer_pool_index
    buffer_assignments: HashMap<(NodeId, usize), usize>,
    output_buffer: StereoBuffer,
}

impl GraphProcessor {
    /// Create a new processor.
    /// 
    /// # Arguments
    /// * `max_buffers` - Maximum number of buffers to pre-allocate
    /// * `buffer_size` - Number of samples per buffer
    pub fn new(max_buffers: usize, buffer_size: usize) -> Self {
        Self {
            buffer_pool: BufferPool::new(max_buffers, buffer_size),
            buffer_assignments: HashMap::new(),
            output_buffer: StereoBuffer::allocate(buffer_size),
        }
    }

    /// Process the graph for one cycle.
    /// Returns the final stereo output.
    pub fn process(
        &mut self,
        graph: &mut AudioGraph,
        context: &ProcessContext,
        midi_events: &[MidiEvent],
    ) -> &StereoBuffer {
        // Get processing order (may recalculate if graph changed)
        let order = match graph.processing_order() {
            Ok(o) => o.to_vec(),
            Err(_) => {
                self.output_buffer.silence();
                return &self.output_buffer;
            }
        };

        // Clear all buffers
        self.buffer_pool.clear_all();
        
        // Assign buffers to node outputs
        self.assign_buffers(graph, &order);

        // Process each node in order
        for &node_id in &order {
            self.process_node(graph, node_id, context, midi_events);
        }

        // Copy output node's inputs to output buffer
        self.collect_output(graph);

        &self.output_buffer
    }

    /// Change buffer size (requires reallocation).
    pub fn set_buffer_size(&mut self, size: usize) {
        let count = self.buffer_pool.buffers.len();
        self.buffer_pool.resize(count, size);
        self.output_buffer = StereoBuffer::allocate(size);
    }

    fn assign_buffers(&mut self, graph: &AudioGraph, order: &[NodeId]) {
        self.buffer_assignments.clear();
        let mut buffer_index = 0;

        for &node_id in order {
            if let Some(node) = graph.get_node(node_id) {
                for (port_idx, _) in node.outputs().iter().enumerate() {
                    self.buffer_assignments.insert((node_id, port_idx), buffer_index);
                    buffer_index += 1;
                    
                    // Grow pool if needed
                    if buffer_index >= self.buffer_pool.buffers.len() {
                        self.buffer_pool.buffers.push(
                            Buffer::allocate(self.buffer_pool.buffer_size)
                        );
                    }
                }
            }
        }
    }

    fn process_node(
        &mut self,
        graph: &mut AudioGraph,
        node_id: NodeId,
        context: &ProcessContext,
        midi_events: &[MidiEvent],
    ) {
        // This is a simplified version - a real implementation would need
        // more careful handling of buffer references
        
        let node = match graph.get_node_mut(node_id) {
            Some(n) => n,
            None => return,
        };

        // For now, just process with empty inputs for non-output nodes
        // A full implementation would gather inputs from connected nodes
        
        let empty_inputs = NodeInputs {
            audio: &[],
            midi: midi_events,
            controls: &[],
        };

        let mut midi_out = Vec::new();
        let outputs = NodeOutputs {
            audio: &mut [],
            midi: &mut midi_out,
        };

        node.process(context, empty_inputs, outputs);
    }

    fn collect_output(&mut self, _graph: &AudioGraph) {
        // Get audio from connections to output node
        // For now, just silence
        self.output_buffer.silence();
        
        // In a full implementation:
        // 1. Find connections to output node's left and right inputs
        // 2. Copy those buffers to output_buffer
    }
}
