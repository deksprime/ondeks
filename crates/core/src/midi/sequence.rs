use super::event::{MidiEvent, TimestampedEvent};
use crate::transport::Beats;

/// A sequence of MIDI events.
#[derive(Debug, Clone, Default)]
pub struct MidiSequence {
    events: Vec<TimestampedEvent>,
    length: Beats,
}

impl MidiSequence {
    /// Create a new MIDI sequence with default length of 4 beats.
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            length: Beats(4.0),
        }
    }

    /// Create with a specific length.
    pub fn with_length(length: Beats) -> Self {
        Self {
            events: Vec::new(),
            length,
        }
    }

    /// Set the sequence length.
    pub fn set_length(&mut self, length: Beats) {
        self.length = length;
    }

    /// Get the sequence length.
    pub fn length(&self) -> Beats {
        self.length
    }

    /// Add an event (maintains sorted order).
    pub fn add_event(&mut self, event: TimestampedEvent) {
        let pos = self.events
            .iter()
            .position(|e| e.time.0 > event.time.0)
            .unwrap_or(self.events.len());
        self.events.insert(pos, event);
    }

    /// Remove events at a specific time.
    pub fn remove_events_at(&mut self, time: Beats, tolerance: f64) {
        self.events.retain(|e| (e.time.0 - time.0).abs() > tolerance);
    }

    /// Get events in a time range.
    pub fn events_in_range(&self, start: Beats, end: Beats) -> impl Iterator<Item = &TimestampedEvent> {
        self.events.iter().filter(move |e| {
            e.time.0 >= start.0 && e.time.0 < end.0
        })
    }

    /// Get all events.
    pub fn events(&self) -> &[TimestampedEvent] {
        &self.events
    }

    /// Quantize all events to a grid.
    pub fn quantize(&mut self, grid: Beats) {
        for event in &mut self.events {
            let beats = event.time.0;
            let quantized = (beats / grid.0).round() * grid.0;
            event.time = Beats(quantized);
        }
    }

    /// Transpose all note events.
    pub fn transpose(&mut self, semitones: i8) {
        use super::types::Note;
        for event in &mut self.events {
            match &mut event.event {
                MidiEvent::NoteOn { note, .. } | MidiEvent::NoteOff { note, .. } => {
                    let new_note = (note.raw() as i16 + semitones as i16).clamp(0, 127) as u8;
                    *note = Note::new(new_note).unwrap();
                }
                _ => {}
            }
        }
    }

    /// Shift all events by a time offset.
    pub fn shift(&mut self, offset: Beats) {
        for event in &mut self.events {
            event.time = Beats((event.time.0 + offset.0).max(0.0));
        }
    }

    /// Clear all events.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if the sequence has no events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::types::*;

    fn make_note_on(time: f64, note: u8) -> TimestampedEvent {
        TimestampedEvent::new(
            Beats(time),
            MidiEvent::note_on(
                Channel::new(0).unwrap(),
                Note::new(note).unwrap(),
                Velocity::new(100).unwrap(),
            ),
        )
    }

    #[test]
    fn add_events_maintains_order() {
        let mut seq = MidiSequence::new();
        seq.add_event(make_note_on(2.0, 60));
        seq.add_event(make_note_on(1.0, 62));
        seq.add_event(make_note_on(3.0, 64));

        let times: Vec<f64> = seq.events().iter().map(|e| e.time.0).collect();
        assert_eq!(times, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn events_in_range() {
        let mut seq = MidiSequence::new();
        seq.add_event(make_note_on(1.0, 60));
        seq.add_event(make_note_on(2.0, 62));
        seq.add_event(make_note_on(3.0, 64));
        seq.add_event(make_note_on(4.0, 65));

        let in_range: Vec<f64> = seq
            .events_in_range(Beats(1.5), Beats(3.5))
            .map(|e| e.time.0)
            .collect();
        assert_eq!(in_range, vec![2.0, 3.0]);
    }

    #[test]
    fn quantize_to_grid() {
        let mut seq = MidiSequence::new();
        seq.add_event(make_note_on(0.9, 60));
        seq.add_event(make_note_on(1.1, 62));
        seq.add_event(make_note_on(2.4, 64));

        seq.quantize(Beats(1.0));

        let times: Vec<f64> = seq.events().iter().map(|e| e.time.0).collect();
        assert_eq!(times, vec![1.0, 1.0, 2.0]);
    }

    #[test]
    fn transpose_notes() {
        let mut seq = MidiSequence::new();
        seq.add_event(make_note_on(1.0, 60)); // C4

        seq.transpose(12); // Up one octave

        if let MidiEvent::NoteOn { note, .. } = &seq.events()[0].event {
            assert_eq!(note.raw(), 72); // C5
        }
    }
}
