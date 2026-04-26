use crate::dsp::{Oscillator, Waveform};
use crate::ids::NodeId;
use crate::graph::{AudioNode, ProcessContext, NodeInputs, NodeOutputs};
use crate::graph::port::{InputPort, OutputPort};

/// A graph node wrapping a DSP oscillator. Gated by transport playing state —
/// silent when transport is stopped, produces a waveform when playing.
///
/// Single mono output port. Connect to both left and right inputs of an
/// [`OutputNode`](crate::graph::nodes::OutputNode) for stereo playback.
pub struct OscillatorNode {
    id: NodeId,
    oscillator: Oscillator,
    outputs: Vec<OutputPort>,
    amplitude: f32,
}

impl OscillatorNode {
    /// Create a new oscillator node with the given sample rate.
    /// Default frequency is 440 Hz, sine wave, amplitude 0.2 (safe listening level).
    pub fn new(sample_rate: u32) -> Self {
        Self {
            id: NodeId::generate(),
            oscillator: Oscillator::new(sample_rate),
            outputs: vec![OutputPort::audio("out")],
            amplitude: 0.2,
        }
    }

    /// Set the oscillator frequency in Hz.
    pub fn with_frequency(mut self, hz: f32) -> Self {
        self.oscillator.set_frequency(hz);
        self
    }

    /// Set the waveform.
    pub fn with_waveform(mut self, waveform: Waveform) -> Self {
        self.oscillator.set_waveform(waveform);
        self
    }

    /// Set the amplitude (linear, 0.0-1.0 typical).
    pub fn with_amplitude(mut self, amplitude: f32) -> Self {
        self.amplitude = amplitude;
        self
    }

    /// Get this node's unique identifier.
    pub fn id(&self) -> NodeId {
        self.id
    }
}

impl AudioNode for OscillatorNode {
    fn name(&self) -> &str {
        "Oscillator"
    }

    fn inputs(&self) -> &[InputPort] {
        &[]
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, context: &ProcessContext, _inputs: NodeInputs, mut outputs: NodeOutputs) {
        let Some(output) = outputs.audio_mut(0) else { return };

        if !context.is_playing {
            output.silence();
            return;
        }

        let amplitude = self.amplitude;
        for sample in output.as_mut_slice() {
            *sample = self.oscillator.tick() * amplitude;
        }
    }

    fn reset(&mut self) {
        self.oscillator.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Buffer;

    fn ctx(frames: usize, playing: bool) -> ProcessContext {
        let mut c = ProcessContext::new(frames, 44100, 120.0);
        c.is_playing = playing;
        c
    }

    #[test]
    fn silent_when_not_playing() {
        let mut node = OscillatorNode::new(44100);
        let mut output = Buffer::allocate(64);

        let inputs = NodeInputs { audio: &[], midi: &[], controls: &[] };
        let output_ref: &mut Buffer = &mut output;
        let outputs = NodeOutputs { audio: &mut [output_ref], midi: &mut vec![] };

        node.process(&ctx(64, false), inputs, outputs);

        assert!(output.as_slice().iter().all(|&s| s == 0.0));
    }

    #[test]
    fn produces_audio_when_playing() {
        let mut node = OscillatorNode::new(44100).with_frequency(440.0);
        let mut output = Buffer::allocate(512);

        let inputs = NodeInputs { audio: &[], midi: &[], controls: &[] };
        let output_ref: &mut Buffer = &mut output;
        let outputs = NodeOutputs { audio: &mut [output_ref], midi: &mut vec![] };

        node.process(&ctx(512, true), inputs, outputs);

        assert!(output.peak() > 0.01, "expected non-zero output, got peak={}", output.peak());
    }
}
