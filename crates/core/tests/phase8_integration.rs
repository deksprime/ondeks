//! Integration tests for Phase 8: Mixer System
//!
//! These tests verify that the mixer system works correctly with channel strips,
//! sends, routing, and metering.

use ondeks_core::mixer::*;
use ondeks_core::TrackId;
use ondeks_core::dsp::StereoBuffer;

#[test]
fn mixer_channel_strip_processing() {
    let master_id = TrackId::generate();
    let mut mixer = Mixer::new(master_id, 512);
    let track_id = TrackId::generate();
    
    mixer.add_channel(track_id);
    
    // Set channel strip parameters
    let channel = mixer.channel_mut(track_id).unwrap();
    channel.volume_db = -6.0; // -6dB
    channel.pan = 0.0; // Center
    
    // Create input audio
    let mut track_outputs = std::collections::HashMap::new();
    let mut input = StereoBuffer::allocate(512);
    input.left_mut().fill(1.0);
    input.right_mut().fill(1.0);
    track_outputs.insert(track_id, input);
    
    // Process through mixer
    let output = mixer.process(&track_outputs);
    
    // Should have processed audio (with gain applied)
    assert!(output.left()[0] > 0.0);
    assert!(output.right()[0] > 0.0);
    // With -6dB gain on track channel, output should be reduced
    // (panning at center also affects gain, so we just verify it's less than input)
    assert!(output.left()[0] < 1.0);
}

#[test]
fn mixer_panning() {
    let master_id = TrackId::generate();
    let mut mixer = Mixer::new(master_id, 512);
    let track_id = TrackId::generate();
    
    mixer.add_channel(track_id);
    
    // Pan full left
    let channel = mixer.channel_mut(track_id).unwrap();
    channel.pan = -1.0;
    
    let mut track_outputs = std::collections::HashMap::new();
    let mut input = StereoBuffer::allocate(512);
    input.left_mut().fill(1.0);
    input.right_mut().fill(1.0);
    track_outputs.insert(track_id, input);
    
    let output = mixer.process(&track_outputs);
    
    // Right channel should be near zero
    assert!(output.right()[0].abs() < 0.1);
    // Left channel should have signal
    assert!(output.left()[0] > 0.0);
}

#[test]
fn mixer_mute() {
    let master_id = TrackId::generate();
    let mut mixer = Mixer::new(master_id, 512);
    let track_id = TrackId::generate();
    
    mixer.add_channel(track_id);
    
    // Mute the channel
    let channel = mixer.channel_mut(track_id).unwrap();
    channel.muted = true;
    
    let mut track_outputs = std::collections::HashMap::new();
    let mut input = StereoBuffer::allocate(512);
    input.left_mut().fill(1.0);
    input.right_mut().fill(1.0);
    track_outputs.insert(track_id, input);
    
    let output = mixer.process(&track_outputs);
    
    // Output should be silent (or very quiet from master)
    assert!(output.left()[0].abs() < 0.01);
    assert!(output.right()[0].abs() < 0.01);
}

#[test]
fn mixer_solo() {
    let master_id = TrackId::generate();
    let mut mixer = Mixer::new(master_id, 512);
    let track1_id = TrackId::generate();
    let track2_id = TrackId::generate();
    
    mixer.add_channel(track1_id);
    mixer.add_channel(track2_id);
    
    // Solo track1
    mixer.channel_mut(track1_id).unwrap().soloed = true;
    
    let mut track_outputs = std::collections::HashMap::new();
    
    let mut track1_input = StereoBuffer::allocate(512);
    track1_input.left_mut().fill(1.0);
    track_outputs.insert(track1_id, track1_input);
    
    let mut track2_input = StereoBuffer::allocate(512);
    track2_input.left_mut().fill(1.0);
    track_outputs.insert(track2_id, track2_input);
    
    let output = mixer.process(&track_outputs);
    
    // Only track1 should be audible
    // Track2 should be muted because solo is active
    assert!(output.left()[0] > 0.0);
}

#[test]
fn mixer_meters() {
    let master_id = TrackId::generate();
    let mut mixer = Mixer::new(master_id, 512);
    let track_id = TrackId::generate();
    
    mixer.add_channel(track_id);
    
    let mut track_outputs = std::collections::HashMap::new();
    let mut input = StereoBuffer::allocate(512);
    input.left_mut().fill(0.5);
    input.right_mut().fill(0.3);
    track_outputs.insert(track_id, input);
    
    mixer.process(&track_outputs);
    
    // Check meters
    let meters = mixer.get_meters();
    assert!(meters.contains_key(&track_id));
    
    let (peak_left, peak_right) = meters[&track_id];
    assert!(peak_left > 0.0);
    assert!(peak_right > 0.0);
}

#[test]
fn send_matrix_routing() {
    let mut matrix = SendMatrix::new();
    let source1 = TrackId::generate();
    let source2 = TrackId::generate();
    let return_track = TrackId::generate();
    
    // Add sends from both sources to return track
    matrix.add_send(Send::new(source1, return_track));
    matrix.add_send(Send::new(source2, return_track));
    
    // Verify sends
    let sends_to_return: Vec<_> = matrix.sends_to(return_track).collect();
    assert_eq!(sends_to_return.len(), 2);
    
    let sends_from_source1: Vec<_> = matrix.sends_from(source1).collect();
    assert_eq!(sends_from_source1.len(), 1);
}

#[test]
fn send_matrix_remove_track() {
    let mut matrix = SendMatrix::new();
    let source1 = TrackId::generate();
    let source2 = TrackId::generate();
    let return_track = TrackId::generate();
    
    matrix.add_send(Send::new(source1, return_track));
    matrix.add_send(Send::new(source2, return_track));
    matrix.add_send(Send::new(source1, return_track));
    
    assert_eq!(matrix.all().len(), 3);
    
    // Remove all sends from source1
    matrix.remove_sends_from(source1);
    
    assert_eq!(matrix.all().len(), 1);
    assert_eq!(matrix.all()[0].source, source2);
}

#[test]
fn channel_strip_effective_gain() {
    let track_id = TrackId::generate();
    let mut strip = ChannelStrip::new(track_id);
    
    // Default: unity gain
    assert!((strip.effective_gain(false) - 1.0).abs() < 1e-6);
    
    // Muted: no gain
    strip.muted = true;
    assert_eq!(strip.effective_gain(false), 0.0);
    
    // Reset mute, set solo
    strip.muted = false;
    strip.soloed = true;
    
    // Solo active and this is soloed: should have gain
    assert!((strip.effective_gain(true) - 1.0).abs() < 1e-6);
    
    // Solo active but this isn't soloed: no gain
    strip.soloed = false;
    assert_eq!(strip.effective_gain(true), 0.0);
}

#[test]
fn mixer_multiple_channels() {
    let master_id = TrackId::generate();
    let mut mixer = Mixer::new(master_id, 512);
    
    let track1_id = TrackId::generate();
    let track2_id = TrackId::generate();
    let track3_id = TrackId::generate();
    
    mixer.add_channel(track1_id);
    mixer.add_channel(track2_id);
    mixer.add_channel(track3_id);
    
    // Set different volumes
    mixer.channel_mut(track1_id).unwrap().volume_db = 0.0;
    mixer.channel_mut(track2_id).unwrap().volume_db = -6.0;
    mixer.channel_mut(track3_id).unwrap().volume_db = -12.0;
    
    let mut track_outputs = std::collections::HashMap::new();
    
    let mut input1 = StereoBuffer::allocate(512);
    input1.left_mut().fill(1.0);
    track_outputs.insert(track1_id, input1);
    
    let mut input2 = StereoBuffer::allocate(512);
    input2.left_mut().fill(1.0);
    track_outputs.insert(track2_id, input2);
    
    let mut input3 = StereoBuffer::allocate(512);
    input3.left_mut().fill(1.0);
    track_outputs.insert(track3_id, input3);
    
    let output = mixer.process(&track_outputs);
    
    // All channels should be mixed together
    assert!(output.left()[0] > 0.0);
    // The sum should be less than 3.0 due to different gains
    assert!(output.left()[0] < 3.0);
}
