use crate::ids::NodeId;
use crate::graph::{AudioNode, ProcessContext, NodeInputs, NodeOutputs};
use crate::graph::port::{InputPort, OutputPort};

/// Mixes multiple audio inputs to a single output.
pub struct MixerNode {
    id: NodeId,
    input_count: usize,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
}

impl MixerNode {
    /// Create a mixer with the specified number of inputs.
    pub fn new(input_count: usize) -> Self {
        let inputs = (0..input_count)
            .map(|i| InputPort::audio(format!("in_{}", i + 1)))
            .collect();
        
        Self {
            id: NodeId::generate(),
            input_count,
            inputs,
            outputs: vec![OutputPort::audio("out")],
        }
    }

    /// Get the unique identifier for this mixer node.
    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl AudioNode for MixerNode {
    fn name(&self) -> &str {
        "Mixer"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, _context: &ProcessContext, inputs: NodeInputs, mut outputs: NodeOutputs) {
        if let Some(output) = outputs.audio_mut(0) {
            output.silence();
            
            for i in 0..self.input_count {
                if let Some(input) = inputs.audio(i) {
                    output.add_from(input);
                }
            }
        }
    }

    fn reset(&mut self) {}
}
