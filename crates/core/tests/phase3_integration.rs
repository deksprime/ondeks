//! Integration tests for Phase 3: Transport & Timing
//!
//! These tests verify that the transport system works correctly with metronome and tempo changes.

use ondeks_core::transport::*;
use ondeks_core::dsp::*;

#[test]
fn transport_with_metronome() {
    let mut transport = Transport::new(44100)
        .set_tempo(120.0)
        .play();
    
    let mut metronome = Metronome::new(MetronomeConfig::default(), 44100);
    let mut buffer = Buffer::allocate(512);
    
    // Process at beat 0
    metronome.process(&mut buffer, &transport, SampleTime(0));
    
    // Should produce click sound
    assert!(buffer.peak() > 0.0);
    
    // Advance transport
    transport = transport.advance(512);
    
    // Process again - should still be within the click duration
    buffer.silence();
    metronome.process(&mut buffer, &transport, SampleTime(512));
    
    // May or may not have sound depending on click duration
    // But shouldn't crash
}

#[test]
fn transport_seek_and_play() {
    let mut transport = Transport::new(44100)
        .set_tempo(120.0)
        .seek(Beats(4.0)) // Seek to bar 2
        .play();
    
    // Position should be at 4 beats
    assert!((transport.position_beats().0 - 4.0).abs() < 0.01);
    
    // Advance by 1 second (44100 samples)
    transport = transport.advance(44100);
    
    // Should be at approximately 6 beats (4 + 2 beats in 1 second at 120 BPM)
    assert!((transport.position_beats().0 - 6.0).abs() < 0.1);
}

#[test]
fn tempo_map_with_transport() {
    let mut tempo_map = TempoMap::constant(120.0);
    tempo_map.add_event(TempoEvent {
        position: Beats(4.0),
        tempo: 60.0,
        curve: TempoCurve::Instant,
    });
    
    // Verify tempo at different positions
    assert_eq!(tempo_map.tempo_at(Beats(0.0)), 120.0);
    assert_eq!(tempo_map.tempo_at(Beats(4.0)), 60.0);
    assert_eq!(tempo_map.tempo_at(Beats(8.0)), 60.0);
    
    // Verify beats to samples conversion
    let samples = tempo_map.beats_to_samples(Beats(8.0), 44100);
    // First 4 beats at 120 BPM = 2 seconds
    // Next 4 beats at 60 BPM = 4 seconds
    // Total = 6 seconds = 264600 samples
    assert_eq!(samples.0, 264600);
}

#[test]
fn transport_loop_region() {
    let mut transport = Transport::new(44100)
        .set_tempo(120.0)
        .set_loop(LoopRegion {
            start: Beats(0.0),
            end: Beats(4.0),
            enabled: true,
        })
        .play()
        .seek(Beats(3.9));
    
    // Advance past loop end
    transport = transport.advance(44100); // Way past loop end
    
    // Should wrap back
    assert!(transport.position_beats().0 < 4.0);
    assert!(transport.position_beats().0 >= 0.0);
}

#[test]
fn transport_state_transitions() {
    let mut transport = Transport::new(44100);
    
    // Start stopped
    assert_eq!(transport.state(), TransportState::Stopped);
    assert!(!transport.is_playing());
    
    // Play
    transport = transport.play();
    assert_eq!(transport.state(), TransportState::Playing);
    assert!(transport.is_playing());
    assert!(!transport.is_recording());
    
    // Record
    transport = transport.record();
    assert_eq!(transport.state(), TransportState::Recording);
    assert!(transport.is_playing());
    assert!(transport.is_recording());
    
    // Stop
    transport = transport.stop();
    assert_eq!(transport.state(), TransportState::Stopped);
    assert_eq!(transport.position().0, 0);
}

#[test]
fn transport_position_conversions() {
    let transport = Transport::new(44100)
        .set_tempo(120.0)
        .seek(Beats(5.5));
    
    // Verify all position representations are consistent
    let beats = transport.position_beats();
    let seconds = transport.position_seconds();
    let bbt = transport.position_bbt();
    
    // 5.5 beats at 120 BPM = 2.75 seconds
    assert!((seconds.0 - 2.75).abs() < 0.01);
    
    // Should be bar 2, beat 2 in 4/4
    assert_eq!(bbt.bar, 2);
    assert_eq!(bbt.beat, 2);
    
    // Round trip: beats -> samples -> beats
    let samples = transport.position();
    let roundtrip_beats = samples.to_beats(transport.tempo(), transport.sample_rate());
    assert!((beats.0 - roundtrip_beats.0).abs() < 0.01);
}
