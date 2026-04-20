use crate::dsp::{Buffer, Sample};
use crate::midi::MidiEvent;
use crate::transport::SampleTime;
use crate::ids::ParameterId;
use super::port::{InputPort, OutputPort};

/// Context provided to nodes during processing.
#[derive(Debug, Clone)]
pub struct ProcessContext {
    /// Number of samples to process in this cycle.
    pub buffer_size: usize,
    /// Current sample rate.
    pub sample_rate: u32,
    /// Current tempo in BPM.
    pub tempo: f64,
    /// Current playback position in samples.
    pub position: SampleTime,
    /// Whether transport is playing.
    pub is_playing: bool,
}

impl ProcessContext {
    /// Create a new process context.
    pub fn new(buffer_size: usize, sample_rate: u32, tempo: f64) -> Self {
        Self {
            buffer_size,
            sample_rate,
            tempo,
            position: SampleTime(0),
            is_playing: false,
        }
    }
}

/// Input data for a node's process call.
pub struct NodeInputs<'a> {
    /// Audio inputs (index corresponds to input port index).
    pub audio: &'a [&'a Buffer],
    /// MIDI events for this cycle.
    pub midi: &'a [MidiEvent],
    /// Control inputs (parameter ID -> value).
    pub controls: &'a [(ParameterId, Sample)],
}

impl<'a> NodeInputs<'a> {
    /// Get audio input at index, or None if out of bounds.
    pub fn audio(&self, index: usize) -> Option<&'a Buffer> {
        self.audio.get(index).copied()
    }

    /// Get control value for a parameter.
    pub fn control(&self, id: ParameterId) -> Option<Sample> {
        self.controls.iter()
            .find(|(pid, _)| *pid == id)
            .map(|(_, v)| *v)
    }
}

/// Output data from a node's process call.
pub struct NodeOutputs<'a> {
    /// Audio outputs (index corresponds to output port index).
    pub audio: &'a mut [&'a mut Buffer],
    /// MIDI events to emit.
    pub midi: &'a mut Vec<MidiEvent>,
}

impl<'a> NodeOutputs<'a> {
    /// Get mutable audio output at index, or None if out of bounds.
    pub fn audio_mut(&mut self, index: usize) -> Option<&mut Buffer> {
        self.audio.get_mut(index).map(|b| &mut **b)
    }
}

/// A processing node in the audio graph.
///
/// # Contract
/// Implementations must:
/// - Write to ALL output buffers every process call (even if just silence)
/// - Not allocate memory during `process()`
/// - Not block or perform IO during `process()`
/// - Not panic during `process()`
pub trait AudioNode: Send {
    /// Get the node's display name.
    fn name(&self) -> &str;

    /// Get the node's input port descriptors.
    fn inputs(&self) -> &[InputPort];

    /// Get the node's output port descriptors.
    fn outputs(&self) -> &[OutputPort];

    /// Process audio for one buffer cycle.
    fn process(&mut self, context: &ProcessContext, inputs: NodeInputs, outputs: NodeOutputs);

    /// Reset the node's internal state (called on transport stop/seek).
    fn reset(&mut self);

    /// Get the current value of a parameter.
    fn get_parameter(&self, _id: ParameterId) -> Option<Sample> {
        None
    }

    /// Set the value of a parameter.
    fn set_parameter(&mut self, _id: ParameterId, _value: Sample) {
        // Default: no parameters
    }

    /// List all parameters this node exposes.
    fn parameters(&self) -> &[ParameterDescriptor] {
        &[]
    }

    /// Deliver a MIDI event to this node's side-channel inbox.
    ///
    /// Called by the engine (on the audio thread) when a `Command::SendMidi`
    /// targets this node. Default no-op. MIDI-aware nodes (`SynthNode`, future
    /// sampler/drum-rack nodes) override this to queue the event for the next
    /// `process()` call, applying at the given sample offset within the block.
    ///
    /// # RT-safety contract
    /// Implementations must not allocate, lock, or block. Enqueueing into a
    /// pre-sized buffer and silently dropping on overflow is the expected shape.
    fn handle_midi(&mut self, _event: &MidiEvent, _sample_offset: u32) {}
}

/// Describes a parameter exposed by a node.
#[derive(Debug, Clone)]
pub struct ParameterDescriptor {
    /// Unique identifier for this parameter.
    pub id: ParameterId,
    /// Human-readable name for this parameter.
    pub name: String,
    /// Minimum allowed value.
    pub min: Sample,
    /// Maximum allowed value.
    pub max: Sample,
    /// Default value when the parameter is reset.
    pub default: Sample,
    /// Unit of measurement for this parameter.
    pub unit: ParameterUnit,
}

/// The unit/display format for a parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterUnit {
    /// Raw 0.0 to 1.0
    Linear,
    /// Decibels (typically -inf to +12)
    Decibels,
    /// Frequency in Hz
    Hertz,
    /// Time in milliseconds
    Milliseconds,
    /// Percentage 0-100
    Percent,
    /// Pitch in semitones
    Semitones,
}
