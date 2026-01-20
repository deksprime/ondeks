//! Integration tests for Phase 1: Core Foundation
//!
//! These tests verify that the fundamental types work together correctly.

use ondeks_core::dsp::*;
use ondeks_core::transport::*;

#[test]
fn buffer_processing_chain() {
    let mut buf = Buffer::from_samples(vec![0.5; 512]);
    buf.apply_gain(db_to_linear(-6.0));
    // -6dB ≈ 0.5, so 0.5 * 0.5 ≈ 0.25
    assert!(buf.peak() < 0.3);
    assert!(buf.peak() > 0.2);
}

#[test]
fn time_conversion_chain() {
    let sample_rate = 44100;
    let tempo = 120.0;
    
    // Start at bar 2, beat 1
    let bbt = BarBeatTick { bar: 2, beat: 1, tick: 0 };
    let beats = Beats::from_bar_beat_tick(bbt, 4, 4);
    let samples = SampleTime::from_beats(beats, tempo, sample_rate);
    let seconds = samples.to_seconds(sample_rate);
    
    // 4 quarter notes at 120 BPM = 2 seconds
    assert!((seconds.0 - 2.0).abs() < 0.001);
}

#[test]
fn stereo_processing_chain() {
    let mut stereo = StereoBuffer::allocate(256);
    stereo.left_mut().fill(1.0);
    stereo.right_mut().fill(1.0);
    
    stereo.apply_gain(0.5);
    stereo.apply_pan(-0.5); // Pan left
    
    // Left should be louder than right
    assert!(stereo.left().peak() > stereo.right().peak());
}
