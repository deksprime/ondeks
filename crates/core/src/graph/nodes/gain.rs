use crate::dsp::{Sample, db_to_linear};
use crate::ids::{NodeId, ParameterId};
use crate::graph::{AudioNode, ProcessContext, NodeInputs, NodeOutputs};
use crate::graph::port::{InputPort, OutputPort};

/// A simple gain (volume) control node.
pub struct GainNode {
    id: NodeId,
    gain: Sample,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,
    gain_param_id: ParameterId,
}

impl GainNode {
    /// Create a new gain node with unity gain.
    pub fn new() -> Self {
        Self {
            id: NodeId::generate(),
            gain: 1.0,
            inputs: vec![InputPort::audio("in")],
            outputs: vec![OutputPort::audio("out")],
            gain_param_id: ParameterId::generate(),
        }
    }

    /// Set gain in decibels.
    pub fn with_gain_db(mut self, db: f32) -> Self {
        self.gain = db_to_linear(db);
        self
    }

    /// Set linear gain.
    pub fn with_gain(mut self, gain: Sample) -> Self {
        self.gain = gain;
        self
    }

    /// Get the node ID.
    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl Default for GainNode {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioNode for GainNode {
    fn name(&self) -> &str {
        "Gain"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, _context: &ProcessContext, inputs: NodeInputs, mut outputs: NodeOutputs) {
        if let (Some(input), Some(output)) = (inputs.audio(0), outputs.audio_mut(0)) {
            output.copy_from(input);
            output.apply_gain(self.gain);
        } else if let Some(output) = outputs.audio_mut(0) {
            output.silence();
        }
    }

    fn reset(&mut self) {
        // Gain node has no state to reset
    }

    fn get_parameter(&self, id: ParameterId) -> Option<Sample> {
        if id == self.gain_param_id {
            Some(self.gain)
        } else {
            None
        }
    }

    fn set_parameter(&mut self, id: ParameterId, value: Sample) {
        if id == self.gain_param_id {
            self.gain = value.max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Buffer;

    fn make_context(size: usize) -> ProcessContext {
        ProcessContext::new(size, 44100, 120.0)
    }

    #[test]
    fn unity_gain_passes_through() {
        let mut node = GainNode::new();
        let input = Buffer::from_samples(vec![0.5, -0.5, 0.25]);
        let mut output = Buffer::allocate(3);
        
        let inputs = NodeInputs {
            audio: &[&input],
            midi: &[],
            controls: &[],
        };
        let mut output_ref: &mut Buffer = &mut output;
        let mut outputs = NodeOutputs {
            audio: &mut [&mut output_ref],
            midi: &mut vec![],
        };
        
        node.process(&make_context(3), inputs, outputs);
        
        assert_eq!(output.as_slice(), &[0.5, -0.5, 0.25]);
    }

    #[test]
    fn gain_scales_signal() {
        let mut node = GainNode::new().with_gain(0.5);
        let input = Buffer::from_samples(vec![1.0, -1.0]);
        let mut output = Buffer::allocate(2);
        
        let inputs = NodeInputs {
            audio: &[&input],
            midi: &[],
            controls: &[],
        };
        let mut output_ref: &mut Buffer = &mut output;
        let mut outputs = NodeOutputs {
            audio: &mut [&mut output_ref],
            midi: &mut vec![],
        };
        
        node.process(&make_context(2), inputs, outputs);
        
        // Output should be halved
        assert!((output[0] - 0.5).abs() < 1e-6);
        assert!((output[1] - -0.5).abs() < 1e-6);
    }

    #[test]
    fn minus_6db_halves_amplitude() {
        let node = GainNode::new().with_gain_db(-6.0);
        assert!((node.gain - 0.501).abs() < 0.01);
    }
}
