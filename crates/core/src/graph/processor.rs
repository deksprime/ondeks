use std::collections::HashMap;
use crate::ids::{NodeId, PortId};
use crate::dsp::{Buffer, StereoBuffer};
use crate::midi::MidiEvent;
use super::audio_graph::AudioGraph;
use super::port::PortType;
use super::node::{ProcessContext, NodeInputs, NodeOutputs};

/// Pool of pre-allocated buffers, one per (node, audio-output-port) pair.
///
/// Allocated at graph-rebuild time. Never reallocated during `process()`.
struct BufferPool {
    buffers: Vec<Buffer>,
    buffer_size: usize,
}

impl BufferPool {
    fn new(buffer_size: usize) -> Self {
        Self { buffers: Vec::new(), buffer_size }
    }

    fn ensure_capacity(&mut self, count: usize) {
        while self.buffers.len() < count {
            self.buffers.push(Buffer::allocate(self.buffer_size));
        }
    }

    fn clear_all(&mut self) {
        for buf in &mut self.buffers {
            buf.silence();
        }
    }

    fn resize(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size;
        for buf in &mut self.buffers {
            *buf = Buffer::allocate(buffer_size);
        }
    }
}

/// Processes an [`AudioGraph`] for one buffer cycle. Routes audio buffers
/// through nodes in topological order, collecting the final stereo output.
///
/// The processor pre-allocates all buffers it needs. The `process()` call
/// is allocation-free (modulo graph-rebuild on topology changes).
pub struct GraphProcessor {
    pool: BufferPool,
    /// Maps (node_id, audio_output_port_index) → pool buffer index.
    assignments: HashMap<(NodeId, usize), usize>,
    /// Scratch buffers for gathering node inputs (grown on demand at build time).
    input_scratch: Vec<Buffer>,
    /// Scratch buffers for receiving node outputs (grown on demand at build time).
    output_scratch: Vec<Buffer>,
    /// Final stereo output buffer.
    output: StereoBuffer,
    buffer_size: usize,
}

impl GraphProcessor {
    /// Create a new processor with the given per-channel buffer size.
    pub fn new(buffer_size: usize) -> Self {
        Self {
            pool: BufferPool::new(buffer_size),
            assignments: HashMap::new(),
            input_scratch: Vec::new(),
            output_scratch: Vec::new(),
            output: StereoBuffer::allocate(buffer_size),
            buffer_size,
        }
    }

    /// Change the buffer size. Reallocates all internal buffers.
    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.buffer_size = buffer_size;
        self.pool.resize(buffer_size);
        for buf in &mut self.input_scratch {
            *buf = Buffer::allocate(buffer_size);
        }
        for buf in &mut self.output_scratch {
            *buf = Buffer::allocate(buffer_size);
        }
        self.output = StereoBuffer::allocate(buffer_size);
    }

    /// Process the graph for one cycle. Returns a reference to the final stereo output.
    pub fn process(
        &mut self,
        graph: &mut AudioGraph,
        context: &ProcessContext,
        midi_events: &[MidiEvent],
    ) -> &StereoBuffer {
        let order = match graph.processing_order() {
            Ok(o) => o.to_vec(),
            Err(_) => {
                self.output.silence();
                return &self.output;
            }
        };

        self.rebuild_assignments(graph, &order);
        self.pool.clear_all();

        for &node_id in &order {
            self.process_one_node(graph, node_id, context, midi_events);
        }

        self.collect_output(graph);
        &self.output
    }

    /// Rebuild the (node, port) → pool-index map for the current topology.
    fn rebuild_assignments(&mut self, graph: &AudioGraph, order: &[NodeId]) {
        self.assignments.clear();
        let mut next_idx = 0usize;

        for &node_id in order {
            let Some(node) = graph.get_node(node_id) else { continue };
            for (port_idx, port) in node.outputs().iter().enumerate() {
                if port.port_type != PortType::Audio {
                    continue;
                }
                self.assignments.insert((node_id, port_idx), next_idx);
                next_idx += 1;
            }
        }

        self.pool.ensure_capacity(next_idx);
    }

    /// Process a single node: gather inputs from connected sources into scratch
    /// buffers, invoke `node.process()`, then copy outputs into the pool.
    fn process_one_node(
        &mut self,
        graph: &mut AudioGraph,
        node_id: NodeId,
        context: &ProcessContext,
        midi_events: &[MidiEvent],
    ) {
        // Phase 1: Inspect the node's ports (immutable borrow).
        let (audio_input_port_ids, audio_output_port_ids) = {
            let Some(node) = graph.get_node(node_id) else { return };
            let ins: Vec<(usize, PortId)> = node
                .inputs()
                .iter()
                .enumerate()
                .filter(|(_, p)| p.port_type == PortType::Audio)
                .map(|(i, p)| (i, p.id))
                .collect();
            let outs: Vec<(usize, PortId)> = node
                .outputs()
                .iter()
                .enumerate()
                .filter(|(_, p)| p.port_type == PortType::Audio)
                .map(|(i, p)| (i, p.id))
                .collect();
            (ins, outs)
        };

        let input_count = audio_input_port_ids.len();
        let output_count = audio_output_port_ids.len();

        // Phase 2: Resolve each audio input port to source pool indices.
        // Multiple connections to the same input port are summed (mix).
        let mut input_sources: Vec<Vec<usize>> = Vec::with_capacity(input_count);
        for &(_, port_id) in &audio_input_port_ids {
            let mut sources: Vec<usize> = Vec::new();
            for conn in graph.connections_to(node_id) {
                if conn.target.port != port_id {
                    continue;
                }
                let Some(src_node) = graph.get_node(conn.source.node) else { continue };
                let Some(src_port_idx) = src_node
                    .outputs()
                    .iter()
                    .position(|p| p.id == conn.source.port)
                else { continue };
                if let Some(&pool_idx) = self.assignments.get(&(conn.source.node, src_port_idx)) {
                    sources.push(pool_idx);
                }
            }
            input_sources.push(sources);
        }

        // Phase 3: Ensure scratch capacity (allocation happens at graph-build time,
        // not during steady-state processing once the graph stabilizes).
        while self.input_scratch.len() < input_count {
            self.input_scratch.push(Buffer::allocate(self.buffer_size));
        }
        while self.output_scratch.len() < output_count {
            self.output_scratch.push(Buffer::allocate(self.buffer_size));
        }

        // Phase 4: Copy inputs from pool into scratch (summing multiple connections).
        for (i, sources) in input_sources.iter().enumerate() {
            let scratch = &mut self.input_scratch[i];
            scratch.silence();
            for &pool_idx in sources {
                scratch.add_from(&self.pool.buffers[pool_idx]);
            }
        }

        // Phase 5: Clear output scratch (nodes must write complete output).
        for i in 0..output_count {
            self.output_scratch[i].silence();
        }

        // Phase 6: Build references and invoke node.process().
        {
            let input_refs: Vec<&Buffer> = self.input_scratch[..input_count].iter().collect();
            let (out_slice, _) = self.output_scratch.split_at_mut(output_count);
            let mut output_refs: Vec<&mut Buffer> = out_slice.iter_mut().collect();

            let inputs = NodeInputs {
                audio: &input_refs,
                midi: midi_events,
                controls: &[],
            };

            let mut midi_out: Vec<MidiEvent> = Vec::new();
            let outputs = NodeOutputs {
                audio: output_refs.as_mut_slice(),
                midi: &mut midi_out,
            };

            if let Some(node) = graph.get_node_mut(node_id) {
                node.process(context, inputs, outputs);
            }
        }

        // Phase 7: Copy output scratch back into the pool at this node's slots.
        for (i, _) in audio_output_port_ids.iter().enumerate() {
            if let Some(&pool_idx) = self.assignments.get(&(node_id, i)) {
                let (src, dst) = (&self.output_scratch[i], &mut self.pool.buffers[pool_idx]);
                dst.copy_from(src);
            }
        }
    }

    /// Copy buffers connected to the graph's output node into the final stereo output.
    /// Left input port → stereo left; right input port → stereo right.
    /// Multiple connections to the same input are summed.
    fn collect_output(&mut self, graph: &AudioGraph) {
        self.output.silence();

        let output_node_id = graph.output();
        let Some(output_node) = graph.get_node(output_node_id) else { return };

        let audio_inputs: Vec<(usize, PortId)> = output_node
            .inputs()
            .iter()
            .enumerate()
            .filter(|(_, p)| p.port_type == PortType::Audio)
            .map(|(i, p)| (i, p.id))
            .collect();

        for (input_idx, port_id) in audio_inputs {
            for conn in graph.connections_to(output_node_id) {
                if conn.target.port != port_id {
                    continue;
                }
                let Some(src_node) = graph.get_node(conn.source.node) else { continue };
                let Some(src_port_idx) = src_node
                    .outputs()
                    .iter()
                    .position(|p| p.id == conn.source.port)
                else { continue };
                let Some(&pool_idx) = self.assignments.get(&(conn.source.node, src_port_idx)) else {
                    continue;
                };

                // Map input port index to stereo channel: 0 = left, 1 = right,
                // anything higher is ignored (future N-channel support).
                let target = match input_idx {
                    0 => self.output.left_mut(),
                    1 => self.output.right_mut(),
                    _ => continue,
                };
                target.add_from(&self.pool.buffers[pool_idx]);
            }
        }
    }
}
