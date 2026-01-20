use crate::ids::NodeId;
use crate::graph::{AudioNode, ProcessContext, NodeInputs, NodeOutputs};
use crate::graph::port::{InputPort, OutputPort};

/// The final destination node that represents audio hardware output.
/// Every graph has exactly one output node.
pub struct OutputNode {
    id: NodeId,
    inputs: Vec<InputPort>,
}

impl OutputNode {
    /// Create a stereo output node.
    pub fn stereo() -> Self {
        Self {
            id: NodeId::generate(),
            inputs: vec![
                InputPort::audio("left"),
                InputPort::audio("right"),
            ],
        }
    }

    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl AudioNode for OutputNode {
    fn name(&self) -> &str {
        "Output"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &[] // Output node has no outputs
    }

    fn process(&mut self, _context: &ProcessContext, _inputs: NodeInputs, _outputs: NodeOutputs) {
        // Output node doesn't process - the graph processor reads its inputs directly
    }

    fn reset(&mut self) {}
}
