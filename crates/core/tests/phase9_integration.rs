//! Integration tests for Phase 9: Automation System
//!
//! These tests verify that the automation system works correctly with
//! automation lanes, points, interpolation, and curve types.

use ondeks_core::automation::*;
use ondeks_core::TrackId;
use ondeks_core::transport::Beats;

#[test]
fn automation_lane_creation() {
    let track_id = TrackId::generate();
    let lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.5,
    );
    
    assert_eq!(lane.default_value, 0.5);
    assert!(lane.enabled);
    assert_eq!(lane.points().len(), 0);
}

#[test]
fn automation_lane_add_points() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackPan(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    lane.add_point(AutomationPoint::new(Beats(2.0), 0.5));
    
    // Points should be sorted by position
    let points = lane.points();
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].position.0, 0.0);
    assert_eq!(points[1].position.0, 2.0);
    assert_eq!(points[2].position.0, 4.0);
}

#[test]
fn automation_lane_value_at_empty() {
    let track_id = TrackId::generate();
    let lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.75,
    );
    
    // Empty lane should return default value
    assert_eq!(lane.value_at(Beats(0.0)), 0.75);
    assert_eq!(lane.value_at(Beats(10.0)), 0.75);
}

#[test]
fn automation_lane_value_at_disabled() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.5,
    );
    
    lane.add_point(AutomationPoint::new(Beats(2.0), 1.0));
    lane.enabled = false;
    
    // Disabled lane should return default value
    assert_eq!(lane.value_at(Beats(2.0)), 0.5);
}

#[test]
fn automation_lane_linear_interpolation() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    // Test linear interpolation
    let value_at_2 = lane.value_at(Beats(2.0));
    assert!((value_at_2 - 0.5).abs() < 0.01, "Expected 0.5, got {}", value_at_2);
    
    let value_at_1 = lane.value_at(Beats(1.0));
    assert!((value_at_1 - 0.25).abs() < 0.01, "Expected 0.25, got {}", value_at_1);
    
    let value_at_3 = lane.value_at(Beats(3.0));
    assert!((value_at_3 - 0.75).abs() < 0.01, "Expected 0.75, got {}", value_at_3);
}

#[test]
fn automation_lane_hold_curve() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0).with_curve(CurveType::Hold));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    // Hold curve should maintain value until next point
    assert_eq!(lane.value_at(Beats(0.0)), 0.0);
    assert_eq!(lane.value_at(Beats(1.0)), 0.0);
    assert_eq!(lane.value_at(Beats(2.0)), 0.0);
    assert_eq!(lane.value_at(Beats(3.9)), 0.0);
    assert_eq!(lane.value_at(Beats(4.0)), 1.0);
    assert_eq!(lane.value_at(Beats(5.0)), 1.0);
}

#[test]
fn automation_lane_scurve_interpolation() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackPan(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0).with_curve(CurveType::SCurve));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    // S-curve should be smooth (smoothstep function)
    let value_at_2 = lane.value_at(Beats(2.0));
    // S-curve at t=0.5 should be 0.5 (smoothstep(0.5) = 0.5)
    assert!((value_at_2 - 0.5).abs() < 0.01);
    
    // Values should be different from linear at intermediate points
    let value_at_1 = lane.value_at(Beats(1.0));
    let linear_value = 0.25;
    // S-curve should be smoother, so at t=0.25 it should be different from linear
    assert_ne!((value_at_1 - linear_value).abs(), 0.0);
}

#[test]
fn automation_lane_logarithmic_interpolation() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0).with_curve(CurveType::Logarithmic));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    // Logarithmic: fast start, slow end
    let value_at_1 = lane.value_at(Beats(1.0));
    let value_at_2 = lane.value_at(Beats(2.0));
    
    // At t=0.25, sqrt(0.25) = 0.5, so value should be 0.5
    assert!((value_at_1 - 0.5).abs() < 0.01);
    // At t=0.5, sqrt(0.5) ≈ 0.707, so value should be ~0.707
    assert!((value_at_2 - 0.707).abs() < 0.02);
}

#[test]
fn automation_lane_exponential_interpolation() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0).with_curve(CurveType::Exponential));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    // Exponential: slow start, fast end
    let value_at_1 = lane.value_at(Beats(1.0));
    let value_at_2 = lane.value_at(Beats(2.0));
    
    // At t=0.25, 0.25^2 = 0.0625, so value should be ~0.0625
    assert!((value_at_1 - 0.0625).abs() < 0.01);
    // At t=0.5, 0.5^2 = 0.25, so value should be 0.25
    assert!((value_at_2 - 0.25).abs() < 0.01);
}

#[test]
fn automation_lane_values_for_range() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    let values = lane.values_for_range(Beats(0.0), Beats(4.0), 5);
    
    assert_eq!(values.len(), 5);
    assert!((values[0] - 0.0).abs() < 0.01);
    assert!((values[2] - 0.5).abs() < 0.01);
    assert!((values[4] - 1.0).abs() < 0.01);
}

#[test]
fn automation_lane_remove_point() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0));
    lane.add_point(AutomationPoint::new(Beats(2.0), 0.5));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    assert_eq!(lane.points().len(), 3);
    
    // Remove point at position 2.0 with tolerance 0.1
    lane.remove_point_at(Beats(2.0), 0.1);
    
    assert_eq!(lane.points().len(), 2);
    assert_eq!(lane.points()[0].position.0, 0.0);
    assert_eq!(lane.points()[1].position.0, 4.0);
}

#[test]
fn automation_lane_clear() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.0));
    lane.add_point(AutomationPoint::new(Beats(2.0), 0.5));
    lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));
    
    assert_eq!(lane.points().len(), 3);
    
    lane.clear();
    
    assert_eq!(lane.points().len(), 0);
    assert_eq!(lane.value_at(Beats(2.0)), 0.0); // Should return default
}

#[test]
fn automation_target_track_volume() {
    let track_id = TrackId::generate();
    let target = AutomationTarget::TrackVolume(track_id);
    
    match target {
        AutomationTarget::TrackVolume(id) => assert_eq!(id, track_id),
        _ => panic!("Expected TrackVolume"),
    }
}

#[test]
fn automation_target_send_level() {
    let source_id = TrackId::generate();
    let dest_id = TrackId::generate();
    let target = AutomationTarget::SendLevel {
        source: source_id,
        destination: dest_id,
    };
    
    match target {
        AutomationTarget::SendLevel { source, destination } => {
            assert_eq!(source, source_id);
            assert_eq!(destination, dest_id);
        }
        _ => panic!("Expected SendLevel"),
    }
}

#[test]
fn automation_point_with_curve() {
    let point = AutomationPoint::new(Beats(2.0), 0.5)
        .with_curve(CurveType::SCurve);
    
    assert_eq!(point.position.0, 2.0);
    assert_eq!(point.value, 0.5);
    assert_eq!(point.curve, CurveType::SCurve);
}

#[test]
fn automation_lane_before_first_point() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(5.0), 1.0));
    
    // Before first point should return first point's value
    assert_eq!(lane.value_at(Beats(0.0)), 1.0);
    assert_eq!(lane.value_at(Beats(3.0)), 1.0);
}

#[test]
fn automation_lane_after_last_point() {
    let track_id = TrackId::generate();
    let mut lane = AutomationLane::new(
        AutomationTarget::TrackVolume(track_id),
        0.0,
    );
    
    lane.add_point(AutomationPoint::new(Beats(0.0), 0.5));
    
    // After last point should return last point's value
    assert_eq!(lane.value_at(Beats(5.0)), 0.5);
    assert_eq!(lane.value_at(Beats(10.0)), 0.5);
}
