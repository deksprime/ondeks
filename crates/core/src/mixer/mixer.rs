use std::collections::HashMap;
use crate::ids::TrackId;
use crate::dsp::StereoBuffer;
use super::channel_strip::ChannelStrip;
use super::send::SendMatrix;

/// The main mixer.
#[derive(Debug)]
pub struct Mixer {
    channels: HashMap<TrackId, ChannelStrip>,
    sends: SendMatrix,
    master_id: TrackId,
    buffer_size: usize,
}

impl Mixer {
    pub fn new(master_id: TrackId, buffer_size: usize) -> Self {
        let mut channels = HashMap::new();
        channels.insert(master_id, ChannelStrip::new(master_id));

        Self {
            channels,
            sends: SendMatrix::new(),
            master_id,
            buffer_size,
        }
    }

    /// Add a channel for a track.
    pub fn add_channel(&mut self, track_id: TrackId) {
        self.channels.insert(track_id, ChannelStrip::new(track_id));
    }

    /// Remove a channel.
    pub fn remove_channel(&mut self, track_id: TrackId) {
        self.channels.remove(&track_id);
        self.sends.remove_sends_from(track_id);
    }

    /// Get a channel.
    pub fn channel(&self, track_id: TrackId) -> Option<&ChannelStrip> {
        self.channels.get(&track_id)
    }

    /// Get a mutable channel.
    pub fn channel_mut(&mut self, track_id: TrackId) -> Option<&mut ChannelStrip> {
        self.channels.get_mut(&track_id)
    }

    /// Check if any channel is soloed.
    pub fn is_solo_active(&self) -> bool {
        self.channels.values().any(|c| c.soloed)
    }

    /// Get the send matrix.
    pub fn sends(&self) -> &SendMatrix {
        &self.sends
    }

    /// Get mutable send matrix.
    pub fn sends_mut(&mut self) -> &mut SendMatrix {
        &mut self.sends
    }

    /// Process all channels and return the master output.
    pub fn process(
        &mut self,
        track_outputs: &HashMap<TrackId, StereoBuffer>,
    ) -> StereoBuffer {
        let solo_active = self.is_solo_active();
        let mut master_input = StereoBuffer::allocate(self.buffer_size);

        // Process each track through its channel strip
        for (track_id, input) in track_outputs {
            if let Some(channel) = self.channels.get_mut(track_id) {
                let mut processed = StereoBuffer::allocate(self.buffer_size);
                channel.process(input, &mut processed, solo_active);
                master_input.add_from(&processed);
            }
        }

        // Process through master channel
        let mut output = StereoBuffer::allocate(self.buffer_size);
        if let Some(master) = self.channels.get_mut(&self.master_id) {
            master.process(&master_input, &mut output, false);
        }

        output
    }

    /// Get all meter readings.
    pub fn get_meters(&self) -> HashMap<TrackId, (f32, f32)> {
        self.channels
            .iter()
            .map(|(&id, ch)| (id, (ch.meters().peak_left, ch.meters().peak_right)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_mixer_has_master() {
        let master_id = TrackId::generate();
        let mixer = Mixer::new(master_id, 512);
        
        assert!(mixer.channel(master_id).is_some());
        assert_eq!(mixer.channel(master_id).unwrap().track_id, master_id);
    }

    #[test]
    fn add_and_remove_channel() {
        let master_id = TrackId::generate();
        let mut mixer = Mixer::new(master_id, 512);
        let track_id = TrackId::generate();
        
        mixer.add_channel(track_id);
        assert!(mixer.channel(track_id).is_some());
        
        mixer.remove_channel(track_id);
        assert!(mixer.channel(track_id).is_none());
    }

    #[test]
    fn is_solo_active() {
        let master_id = TrackId::generate();
        let mut mixer = Mixer::new(master_id, 512);
        let track_id = TrackId::generate();
        
        mixer.add_channel(track_id);
        assert!(!mixer.is_solo_active());
        
        mixer.channel_mut(track_id).unwrap().soloed = true;
        assert!(mixer.is_solo_active());
    }

    #[test]
    fn process_mixes_channels() {
        let master_id = TrackId::generate();
        let mut mixer = Mixer::new(master_id, 4);
        let track_id = TrackId::generate();
        
        mixer.add_channel(track_id);
        
        let mut track_outputs = HashMap::new();
        let mut track_buf = StereoBuffer::allocate(4);
        track_buf.left_mut().fill(0.5);
        track_buf.right_mut().fill(0.5);
        track_outputs.insert(track_id, track_buf);
        
        let output = mixer.process(&track_outputs);
        
        // Should have processed audio
        assert!(output.left()[0] > 0.0);
        assert!(output.right()[0] > 0.0);
    }

    #[test]
    fn process_respects_solo() {
        let master_id = TrackId::generate();
        let mut mixer = Mixer::new(master_id, 4);
        let track1_id = TrackId::generate();
        let track2_id = TrackId::generate();
        
        mixer.add_channel(track1_id);
        mixer.add_channel(track2_id);
        mixer.channel_mut(track1_id).unwrap().soloed = true;
        
        let mut track_outputs = HashMap::new();
        
        let mut track1_buf = StereoBuffer::allocate(4);
        track1_buf.left_mut().fill(1.0);
        track_outputs.insert(track1_id, track1_buf);
        
        let mut track2_buf = StereoBuffer::allocate(4);
        track2_buf.left_mut().fill(1.0);
        track_outputs.insert(track2_id, track2_buf);
        
        let output = mixer.process(&track_outputs);
        
        // Only track1 should be audible (soloed)
        // The output should contain track1's signal
        assert!(output.left()[0] > 0.0);
    }

    #[test]
    fn get_meters() {
        let master_id = TrackId::generate();
        let mut mixer = Mixer::new(master_id, 4);
        let track_id = TrackId::generate();
        
        mixer.add_channel(track_id);
        
        let mut track_outputs = HashMap::new();
        let mut track_buf = StereoBuffer::allocate(4);
        track_buf.left_mut().fill(0.5);
        track_buf.right_mut().fill(0.3);
        track_outputs.insert(track_id, track_buf);
        
        mixer.process(&track_outputs);
        
        let meters = mixer.get_meters();
        assert!(meters.contains_key(&track_id));
        assert!(meters.contains_key(&master_id));
    }
}
