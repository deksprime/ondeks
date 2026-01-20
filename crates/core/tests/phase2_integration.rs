//! Integration tests for Phase 2: Audio Graph System
//!
//! These tests verify that the audio graph system works correctly.

use ondeks_core::graph::*;
use ondeks_core::graph::nodes::*;

#[test]
fn complete_signal_chain() {
    let mut graph = AudioGraph::new();
    
    // Create nodes
    let gain = GainNode::new().with_gain_db(-6.0);
    let gain_id = gain.id();
    let _gain_in = gain.inputs()[0].id;
    let gain_out = gain.outputs()[0].id;
    graph.add_node_with_id(gain_id, Box::new(gain));
    
    // Connect gain to output
    let output_in = graph.get_node(graph.output()).unwrap().inputs()[0].id;
    graph.connect(
        PortAddress::new(gain_id, gain_out),
        PortAddress::new(graph.output(), output_in),
    ).unwrap();
    
    // Verify processing order
    let output_id = graph.output();
    let order = graph.processing_order().unwrap();
    assert!(order.contains(&gain_id));
    assert!(order.contains(&output_id));
    
    // Gain should come before output
    let gain_pos = order.iter().position(|&id| id == gain_id).unwrap();
    let out_pos = order.iter().position(|&id| id == output_id).unwrap();
    assert!(gain_pos < out_pos);
}
