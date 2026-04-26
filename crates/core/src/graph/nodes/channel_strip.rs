//! Per-track channel strip: gain, pan, mute, solo.
//!
//! Mono input → stereo output. The strip applies (in order) mute/solo
//! gating, then a smoothed gain, then constant-power panning. Smoothing is a
//! simple one-pole low-pass against per-block target values — long enough
//! (~50 ms) that drag-driven changes don't click, short enough that the UI
//! still feels responsive.
//!
//! Solo is split between two flags: `soloed` is the user's intent on this
//! strip, and `silenced_by_other_solo` is set externally by the engine
//! whenever any *other* strip is soloed. The engine recomputes the latter on
//! every solo change.

use std::f32::consts::PI;

use crate::dsp::{db_to_linear, Sample};
use crate::graph::port::{InputPort, OutputPort};
use crate::graph::{AudioNode, NodeInputs, NodeOutputs, ProcessContext};
use crate::ids::{NodeId, TrackId};

/// Time constant for parameter smoothing in seconds.
const SMOOTHING_TIME_S: f32 = 0.05;

/// One-pole LPF coefficient for the given time constant and sample rate.
fn smoothing_coeff(sample_rate: u32, time_s: f32) -> f32 {
    // Standard one-pole: y[n] = a * x[n] + (1 - a) * y[n-1], with a chosen
    // so that step response reaches ~63% in time_s.
    let samples = (sample_rate as f32) * time_s;
    if samples <= 1.0 {
        1.0
    } else {
        1.0 - (-1.0_f32 / samples).exp()
    }
}

/// Per-track channel strip node: 1 mono input → 2 mono outputs (L, R).
pub struct ChannelStripNode {
    id: NodeId,
    track_id: TrackId,

    inputs: Vec<InputPort>,
    outputs: Vec<OutputPort>,

    // Targets: set by Command::SetTrack*. Strip values smooth toward these.
    target_gain: Sample,
    target_pan: Sample,

    // Smoothed values updated each block.
    current_gain: Sample,
    current_pan: Sample,

    // Mute state (instant — no smoothing). Click avoidance for mute is a
    // polish slice; for now the smoothed gain handles most of it.
    muted: bool,
    /// User's solo intent on this track.
    soloed: bool,
    /// True when *some other* strip is soloed, so this strip should be
    /// silent. Set by the engine on every solo change.
    silenced_by_other_solo: bool,

    /// One-pole smoothing coefficient, recomputed if sample rate changes.
    smoothing_coeff: f32,
    sample_rate: u32,
}

impl ChannelStripNode {
    /// Create a strip with the given node id at unity gain, center pan.
    pub fn with_id(id: NodeId, track_id: TrackId, sample_rate: u32) -> Self {
        Self {
            id,
            track_id,
            inputs: vec![InputPort::audio("in")],
            outputs: vec![OutputPort::audio("left"), OutputPort::audio("right")],
            target_gain: 1.0,
            target_pan: 0.0,
            current_gain: 1.0,
            current_pan: 0.0,
            muted: false,
            soloed: false,
            silenced_by_other_solo: false,
            smoothing_coeff: smoothing_coeff(sample_rate, SMOOTHING_TIME_S),
            sample_rate,
        }
    }

    /// Node id.
    pub fn id(&self) -> NodeId {
        self.id
    }

    /// Owning track id.
    pub fn track_id(&self) -> TrackId {
        self.track_id
    }

    /// Set the target volume in dB. Smoothing brings `current_gain` to it
    /// over ~50 ms.
    pub fn set_volume_db(&mut self, db: f32) {
        self.target_gain = db_to_linear(db);
    }

    /// Set the target pan (-1.0 = full left, 1.0 = full right).
    pub fn set_pan(&mut self, pan: f32) {
        self.target_pan = pan.clamp(-1.0, 1.0);
    }

    /// Set the mute flag (instant).
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
    }

    /// Set the user's solo intent.
    pub fn set_soloed(&mut self, soloed: bool) {
        self.soloed = soloed;
    }

    /// Whether the user has soloed this strip.
    pub fn is_soloed(&self) -> bool {
        self.soloed
    }

    /// Set whether some other strip is soloed (silences this one).
    pub fn set_silenced_by_other_solo(&mut self, silenced: bool) {
        self.silenced_by_other_solo = silenced;
    }
}

impl AudioNode for ChannelStripNode {
    fn name(&self) -> &str {
        "Channel Strip"
    }

    fn inputs(&self) -> &[InputPort] {
        &self.inputs
    }

    fn outputs(&self) -> &[OutputPort] {
        &self.outputs
    }

    fn process(&mut self, context: &ProcessContext, inputs: NodeInputs, outputs: NodeOutputs) {
        // Recompute smoothing coefficient if sample rate changed (rare).
        if context.sample_rate != self.sample_rate {
            self.sample_rate = context.sample_rate;
            self.smoothing_coeff = smoothing_coeff(self.sample_rate, SMOOTHING_TIME_S);
        }

        // Mute / solo gating: fold into the smoothed gain target so transitions
        // ramp instead of clicking. We fold the gate as a multiplicative 0/1
        // on the *target* — the smoothed `current_gain` follows it.
        let gate = if self.muted || self.silenced_by_other_solo {
            0.0
        } else {
            1.0
        };
        let effective_target = self.target_gain * gate;
        let coeff = self.smoothing_coeff;
        let frames = context.buffer_size;
        let input = inputs.audio(0);

        // Destructure the mutable output slice into two independent
        // mutable buffer references.
        let (left, right) = match &mut *outputs.audio {
            [l, r, ..] => (&mut **l, &mut **r),
            _ => return,
        };

        let frames = frames.min(left.len()).min(right.len());
        let l_slice = left.as_mut_slice();
        let r_slice = right.as_mut_slice();

        for i in 0..frames {
            self.current_gain += coeff * (effective_target - self.current_gain);
            self.current_pan += coeff * (self.target_pan - self.current_pan);

            // Constant-power pan: pan ∈ [-1, 1] → angle ∈ [0, π/2].
            let angle = (self.current_pan + 1.0) * PI / 4.0;
            let l_gain = self.current_gain * angle.cos();
            let r_gain = self.current_gain * angle.sin();

            let sample = input.map(|b| b.as_slice()[i]).unwrap_or(0.0);
            l_slice[i] = sample * l_gain;
            r_slice[i] = sample * r_gain;
        }
    }

    fn reset(&mut self) {
        self.current_gain = self.target_gain;
        self.current_pan = self.target_pan;
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::Buffer;

    fn ctx(frames: usize, sr: u32) -> ProcessContext {
        ProcessContext::new(frames, sr, 120.0)
    }

    fn run(node: &mut ChannelStripNode, input_value: f32, frames: usize) -> (Buffer, Buffer) {
        let input = Buffer::from_samples(vec![input_value; frames]);
        let mut left = Buffer::allocate(frames);
        let mut right = Buffer::allocate(frames);

        let inputs = NodeInputs { audio: &[&input], midi: &[], controls: &[] };
        let l_ref: &mut Buffer = &mut left;
        let r_ref: &mut Buffer = &mut right;
        let outputs = NodeOutputs {
            audio: &mut [l_ref, r_ref],
            midi: &mut vec![],
        };

        node.process(&ctx(frames, 44100), inputs, outputs);

        (left, right)
    }

    fn make() -> ChannelStripNode {
        ChannelStripNode::with_id(NodeId::generate(), TrackId::generate(), 44100)
    }

    #[test]
    fn unity_gain_center_pan_passes_through_with_constant_power() {
        let mut node = make();
        node.reset();
        let (left, right) = run(&mut node, 1.0, 64);
        // Constant power center pan = sin(π/4) = cos(π/4) ≈ 0.707
        let sqrt_half = (0.5_f32).sqrt();
        for i in 0..64 {
            assert!(
                (left[i] - sqrt_half).abs() < 1e-3,
                "L[{i}]={} expected ~{}",
                left[i], sqrt_half
            );
            assert!(
                (right[i] - sqrt_half).abs() < 1e-3,
                "R[{i}]={} expected ~{}",
                right[i], sqrt_half
            );
        }
    }

    #[test]
    fn full_left_pan_silences_right() {
        let mut node = make();
        node.set_pan(-1.0);
        node.reset();
        let (_left, right) = run(&mut node, 1.0, 256);
        assert!(right.peak() < 0.05, "right peak {} should be near zero", right.peak());
    }

    /// Long enough for the 50 ms smoothing to reach ~99% steady state at
    /// 44.1 kHz: ~5 × τ ≈ 11 000 samples. Use 16 384 to leave headroom.
    const SETTLE_FRAMES: usize = 16_384;

    #[test]
    fn mute_smooths_to_silence() {
        let mut node = make();
        node.reset();
        node.set_muted(true);
        let (left, right) = run(&mut node, 1.0, SETTLE_FRAMES);
        let tail = SETTLE_FRAMES - 256;
        assert!(left.as_slice()[tail..].iter().all(|&s| s.abs() < 1e-2));
        assert!(right.as_slice()[tail..].iter().all(|&s| s.abs() < 1e-2));
    }

    #[test]
    fn silenced_by_other_solo_silences_output() {
        let mut node = make();
        node.reset();
        node.set_silenced_by_other_solo(true);
        let (left, right) = run(&mut node, 1.0, SETTLE_FRAMES);
        let tail = SETTLE_FRAMES - 256;
        assert!(left.as_slice()[tail..].iter().all(|&s| s.abs() < 1e-2));
        assert!(right.as_slice()[tail..].iter().all(|&s| s.abs() < 1e-2));
    }

    #[test]
    fn volume_change_smooths_does_not_click() {
        // Drop volume from 0 dB → -60 dB. The first sample should NOT already
        // be at the new target (that's a click); after the smoothing time
        // constant settles, the *tail* of the buffer should be near silent.
        let mut node = make();
        node.reset();
        node.set_volume_db(-60.0);
        let (left, _right) = run(&mut node, 1.0, 64);
        assert!(left[0] > 0.1, "first sample should still be high (smoothing)");
        let (later_left, _) = run(&mut node, 1.0, SETTLE_FRAMES);
        let tail_start = SETTLE_FRAMES - 256;
        let tail_peak = later_left.as_slice()[tail_start..]
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max);
        assert!(
            tail_peak < 0.05,
            "after {} samples, tail peak should be near silent, got {}",
            SETTLE_FRAMES,
            tail_peak,
        );
    }
}
