use super::{Sample, Oscillator, Waveform, AdsrEnvelope, EnvelopeStage, SvFilter, FilterType};
use crate::midi::{MidiEvent, Note, Velocity};

const MAX_VOICES: usize = 8;

/// A single synth voice.
#[derive(Debug, Clone)]
struct Voice {
    oscillator: Oscillator,
    filter: SvFilter,
    amp_envelope: AdsrEnvelope,
    filter_envelope: AdsrEnvelope,
    note: Option<Note>,
    velocity: f32,
}

impl Voice {
    fn new(sample_rate: u32) -> Self {
        Self {
            oscillator: Oscillator::new(sample_rate),
            filter: SvFilter::new(sample_rate),
            amp_envelope: AdsrEnvelope::new(sample_rate),
            filter_envelope: AdsrEnvelope::new(sample_rate),
            note: None,
            velocity: 0.0,
        }
    }

    fn is_active(&self) -> bool {
        self.amp_envelope.is_active()
    }
}

/// A simple polyphonic synthesizer.
#[derive(Debug)]
pub struct SimpleSynth {
    voices: Vec<Voice>,
    waveform: Waveform,
    filter_cutoff: f32,
    filter_resonance: f32,
    filter_env_amount: f32,
    sample_rate: u32,
}

impl SimpleSynth {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            voices: (0..MAX_VOICES).map(|_| Voice::new(sample_rate)).collect(),
            waveform: Waveform::Saw,
            filter_cutoff: 2000.0,
            filter_resonance: 0.3,
            filter_env_amount: 2000.0,
            sample_rate,
        }
    }

    pub fn set_waveform(&mut self, waveform: Waveform) {
        self.waveform = waveform;
        for voice in &mut self.voices {
            voice.oscillator.set_waveform(waveform);
        }
    }

    pub fn set_filter_cutoff(&mut self, freq: f32) {
        self.filter_cutoff = freq;
    }

    pub fn set_filter_resonance(&mut self, res: f32) {
        self.filter_resonance = res;
    }

    /// Handle a MIDI event.
    pub fn handle_midi(&mut self, event: &MidiEvent) {
        match event {
            MidiEvent::NoteOn { note, velocity, .. } => {
                self.note_on(*note, *velocity);
            }
            MidiEvent::NoteOff { note, .. } => {
                self.note_off(*note);
            }
            _ => {}
        }
    }

    fn note_on(&mut self, note: Note, velocity: Velocity) {
        // Find a free voice or steal the oldest
        let voice_idx = self.voices.iter()
            .position(|v| !v.is_active())
            .unwrap_or(0);
        let voice = &mut self.voices[voice_idx];

        voice.note = Some(note);
        voice.velocity = velocity.normalized();
        voice.oscillator.set_frequency(note.frequency());
        voice.oscillator.set_waveform(self.waveform);
        voice.amp_envelope.trigger();
        voice.filter_envelope.trigger();
    }

    fn note_off(&mut self, note: Note) {
        for voice in &mut self.voices {
            if voice.note == Some(note) {
                voice.amp_envelope.release();
                voice.filter_envelope.release();
            }
        }
    }

    /// Generate audio output.
    pub fn process(&mut self, output: &mut [Sample]) {
        output.fill(0.0);

        for voice in &mut self.voices {
            if !voice.is_active() {
                continue;
            }

            for sample in output.iter_mut() {
                // Generate oscillator
                let osc = voice.oscillator.tick();

                // Apply filter with envelope
                let filter_env = voice.filter_envelope.tick();
                let cutoff = self.filter_cutoff + self.filter_env_amount * filter_env;
                voice.filter.set_cutoff(cutoff);
                voice.filter.set_resonance(self.filter_resonance);
                let filtered = voice.filter.tick(osc);

                // Apply amplitude envelope
                let amp = voice.amp_envelope.tick() * voice.velocity;
                *sample += filtered * amp;
            }
        }
    }

    /// Reset all voices.
    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.amp_envelope.reset();
            voice.filter_envelope.reset();
            voice.oscillator.reset();
            voice.filter.reset();
            voice.note = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::Channel;

    #[test]
    fn synth_creation() {
        let synth = SimpleSynth::new(44100);
        assert_eq!(synth.voices.len(), MAX_VOICES);
    }

    #[test]
    fn synth_note_on() {
        let mut synth = SimpleSynth::new(44100);
        let note = Note::new(60).unwrap();
        let velocity = Velocity::new(100).unwrap();
        let channel = Channel::new(0).unwrap();
        
        let event = MidiEvent::NoteOn { channel, note, velocity };
        synth.handle_midi(&event);
        
        // At least one voice should be active
        assert!(synth.voices.iter().any(|v| v.is_active()));
    }

    #[test]
    fn synth_note_off() {
        let mut synth = SimpleSynth::new(44100);
        let note = Note::new(60).unwrap();
        let velocity = Velocity::new(100).unwrap();
        let channel = Channel::new(0).unwrap();
        
        let note_on = MidiEvent::NoteOn { channel, note, velocity };
        synth.handle_midi(&note_on);
        
        let note_off = MidiEvent::NoteOff { channel, note, velocity: Velocity::OFF };
        synth.handle_midi(&note_off);
        
        // Voice should be releasing
        assert!(synth.voices.iter().any(|v| v.amp_envelope.stage() == EnvelopeStage::Release));
    }

    #[test]
    fn synth_process_generates_audio() {
        let mut synth = SimpleSynth::new(44100);
        let note = Note::new(60).unwrap();
        let velocity = Velocity::new(100).unwrap();
        let channel = Channel::new(0).unwrap();
        
        let event = MidiEvent::NoteOn { channel, note, velocity };
        synth.handle_midi(&event);
        
        let mut output = vec![0.0; 64];
        synth.process(&mut output);
        
        // Should generate some audio
        assert!(output.iter().any(|&s| s.abs() > 0.0));
    }

    #[test]
    fn synth_reset() {
        let mut synth = SimpleSynth::new(44100);
        let note = Note::new(60).unwrap();
        let velocity = Velocity::new(100).unwrap();
        let channel = Channel::new(0).unwrap();
        
        let event = MidiEvent::NoteOn { channel, note, velocity };
        synth.handle_midi(&event);
        synth.reset();
        
        // All voices should be inactive
        assert!(synth.voices.iter().all(|v| !v.is_active()));
    }
}
