//! Built-in audio processing nodes.

mod channel_strip;
mod gain;
mod master_strip;
mod mixer;
mod oscillator;
mod output;
mod synth;

pub use channel_strip::ChannelStripNode;
pub use gain::GainNode;
pub use master_strip::MasterStripNode;
pub use mixer::MixerNode;
pub use oscillator::OscillatorNode;
pub use output::OutputNode;
pub use synth::SynthNode;
