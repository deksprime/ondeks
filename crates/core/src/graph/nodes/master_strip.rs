//! Master output strip: stereo gain, no pan.
//!
//! Sits between all per-track channel strips and the `OutputNode`. Two mono
//! inputs (L, R) → two mono outputs (L, R). Master fader is its smoothed
//! gain. Multi-connection summation happens automatically in `GraphProcessor`
//! when more than one strip connects to the same input port.

use crate::dsp::{db_to_linear, Sample};
use crate::graph::port::{InputPort, OutputPort};
use crate::graph::{AudioNode, NodeInputs, NodeOutputs, ProcessContext};
use crate::ids::NodeId;

const SMOOTHING_TIME_S: f32 = 0.05;

fn smoothing_coeff(sample_rate: u32, time_s: f32) -> f32 {
    let samples = (sample_rate as f32) * time_s;
    if samples <= 1.0 {
        1.0
    } else {
        1.0 - (-1.0_f32 / samples).exp()
    }
}

/// The master output strip: applies a smoothed master gain to a stereo bus.
pub struct MasterStripNode {
    id: NodeId,
    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,

    target_gain: Sample,
    current_gain: Sample,

    smoothing_coeff: f32,
    sample_rate: u32,
}

impl MasterStripNode {
    /// Create a unity-gain master strip with the given id.
    pub fn with_id(id: NodeId, sample_rate: u32) -> Self {
        Self {
            id,
            inputs: vec![InputPort::audio("left"), InputPort::audio("right")],
            outputs: vec![OutputPort::audio("left"), OutputPort::audio("right")],
            target_gain: 1.0,
            current_gain: 1.0,
            smoothing_coeff: smoothing_coeff(sample_rate, SMOOTHING_TIME_S),
            sample_rate,
        }
    }

    /// Node id.
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Set the target master volume in dB.
    pub fn set_volume_db(&mut self, db: f32) {
        self.target_gain = db_to_linear(db);
    }
}

impl AudioNode for MasterStripNode {
    fn name(&self) -> &str {
        "Master Strip"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, context: &ProcessContext, inputs: NodeInputs, outputs: NodeOutputs) {
        if context.sample_rate != self.sample_rate {
            self.sample_rate = context.sample_rate;
            self.smoothing_coeff = smoothing_coeff(self.sample_rate, SMOOTHING_TIME_S);
        }

        let coeff = self.smoothing_coeff;
        let frames = context.buffer_size;

        let input_l = inputs.audio(0);
        let input_r = inputs.audio(1);

        let (left, right) = match &mut *outputs.audio {
            [l, r, ..] => (&mut **l, &mut **r),
            _ => return,
        };

        let frames = frames.min(left.len()).min(right.len());
        let l_slice = left.as_mut_slice();
        let r_slice = right.as_mut_slice();

        for i in 0..frames {
            self.current_gain += coeff * (self.target_gain - self.current_gain);
            let in_l = input_l.map(|b| b.as_slice()[i]).unwrap_or(0.0);
            let in_r = input_r.map(|b| b.as_slice()[i]).unwrap_or(0.0);
            l_slice[i] = in_l * self.current_gain;
            r_slice[i] = in_r * self.current_gain;
        }
    }

    fn reset(&mut self) {
        self.current_gain = self.target_gain;
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Buffer;

    fn ctx(frames: usize) -> ProcessContext {
        ProcessContext::new(frames, 44100, 120.0)
    }

    fn run(node: &mut MasterStripNode, l_value: f32, r_value: f32, frames: usize) -> (Buffer, Buffer) {
        let in_l = Buffer::from_samples(vec![l_value; frames]);
        let in_r = Buffer::from_samples(vec![r_value; frames]);
        let mut out_l = Buffer::allocate(frames);
        let mut out_r = Buffer::allocate(frames);

        let inputs = NodeInputs { audio: &[&in_l, &in_r], midi: &[], controls: &[] };
        let l_ref: &mut Buffer = &mut out_l;
        let r_ref: &mut Buffer = &mut out_r;
        let outputs = NodeOutputs {
            audio: &mut [l_ref, r_ref],
            midi: &mut vec![],
        };

        node.process(&ctx(frames), inputs, outputs);

        (out_l, out_r)
    }

    #[test]
    fn unity_gain_passes_through() {
        let mut node = MasterStripNode::with_id(NodeId::generate(), 44100);
        node.reset();
        let (l, r) = run(&mut node, 0.5, -0.5, 64);
        for i in 0..64 {
            assert!((l[i] - 0.5).abs() < 1e-3);
            assert!((r[i] - -0.5).abs() < 1e-3);
        }
    }

    #[test]
    fn volume_change_smooths() {
        let mut node = MasterStripNode::with_id(NodeId::generate(), 44100);
        node.reset();
        node.set_volume_db(-60.0);
        let (l, _) = run(&mut node, 1.0, 1.0, 64);
        assert!(l[0] > 0.1, "first sample shouldn't already be at the new target");
        // ~5 time constants (50 ms × 5 ≈ 11 000 samples) for ~99% decay; check
        // the *tail* of the long render so the early ramp doesn't dominate.
        let (later, _) = run(&mut node, 1.0, 1.0, 16_384);
        let tail = &later.as_slice()[16_384 - 256..];
        let tail_peak = tail.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
        assert!(tail_peak < 0.05, "tail peak after settle = {}", tail_peak);
    }
}
