use crate::ids::AutomationLaneId;
use crate::transport::Beats;
use super::types::{AutomationPoint, AutomationTarget, CurveType};

/// An automation lane with points.
#[derive(Debug, Clone)]
pub struct AutomationLane {
    /// Unique identifier for this automation lane.
    pub id: AutomationLaneId,
    /// Target parameter that this lane automates.
    pub target: AutomationTarget,
    /// Automation points (sorted by position).
    points: Vec<AutomationPoint>,
    /// Whether this lane is enabled.
    pub enabled: bool,
    /// Default value when no points exist or lane is disabled.
    pub default_value: f32,
}

impl AutomationLane {
    /// Create a new automation lane.
    pub fn new(target: AutomationTarget, default_value: f32) -> Self {
        Self {
            id: AutomationLaneId::generate(),
            target,
            points: Vec::new(),
            enabled: true,
            default_value,
        }
    }

    /// Add a point (maintains sorted order).
    pub fn add_point(&mut self, point: AutomationPoint) {
        let pos = self.points
            .iter()
            .position(|p| p.position.0 > point.position.0)
            .unwrap_or(self.points.len());
        self.points.insert(pos, point);
    }

    /// Remove point at position.
    pub fn remove_point_at(&mut self, position: Beats, tolerance: f64) {
        self.points.retain(|p| (p.position.0 - position.0).abs() > tolerance);
    }

    /// Get value at a specific position.
    pub fn value_at(&self, position: Beats) -> f32 {
        if self.points.is_empty() || !self.enabled {
            return self.default_value;
        }

        // Find surrounding points
        let mut prev: Option<&AutomationPoint> = None;
        let mut next: Option<&AutomationPoint> = None;

        for point in &self.points {
            if point.position.0 <= position.0 {
                prev = Some(point);
            } else {
                next = Some(point);
                break;
            }
        }

        match (prev, next) {
            (Some(p), Some(n)) => {
                // Interpolate
                let t = (position.0 - p.position.0) / (n.position.0 - p.position.0);
                self.interpolate(p.value, n.value, t as f32, p.curve)
            }
            (Some(p), None) => p.value,
            (None, Some(n)) => n.value,
            (None, None) => self.default_value,
        }
    }

    /// Generate values for a range of samples.
    pub fn values_for_range(
        &self,
        start: Beats,
        end: Beats,
        num_samples: usize,
    ) -> Vec<f32> {
        if num_samples == 0 {
            return Vec::new();
        }
        if num_samples == 1 {
            return vec![self.value_at(start)];
        }
        let step = (end.0 - start.0) / (num_samples - 1) as f64;
        (0..num_samples)
            .map(|i| {
                let pos = Beats(start.0 + i as f64 * step);
                self.value_at(pos)
            })
            .collect()
    }

    fn interpolate(&self, a: f32, b: f32, t: f32, curve: CurveType) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match curve {
            CurveType::Hold => a,
            CurveType::Linear => a + (b - a) * t,
            CurveType::SCurve => {
                let t = t * t * (3.0 - 2.0 * t); // Smoothstep
                a + (b - a) * t
            }
            CurveType::Logarithmic => {
                let t = t.sqrt();
                a + (b - a) * t
            }
            CurveType::Exponential => {
                let t = t * t;
                a + (b - a) * t
            }
        }
    }

    /// Get all points.
    pub fn points(&self) -> &[AutomationPoint] {
        &self.points
    }

    /// Clear all points.
    pub fn clear(&mut self) {
        self.points.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::TrackId;

    fn make_lane() -> AutomationLane {
        AutomationLane::new(
            AutomationTarget::TrackVolume(TrackId::generate()),
            0.0,
        )
    }

    #[test]
    fn empty_lane_returns_default() {
        let lane = make_lane();
        assert_eq!(lane.value_at(Beats(5.0)), 0.0);
    }

    #[test]
    fn single_point_holds_value() {
        let mut lane = make_lane();
        lane.add_point(AutomationPoint::new(Beats(2.0), 0.8));

        assert_eq!(lane.value_at(Beats(0.0)), 0.8);
        assert_eq!(lane.value_at(Beats(2.0)), 0.8);
        assert_eq!(lane.value_at(Beats(10.0)), 0.8);
    }

    #[test]
    fn linear_interpolation() {
        let mut lane = make_lane();
        lane.add_point(AutomationPoint::new(Beats(0.0), 0.0));
        lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));

        assert!((lane.value_at(Beats(2.0)) - 0.5).abs() < 0.01);
        assert!((lane.value_at(Beats(1.0)) - 0.25).abs() < 0.01);
    }

    #[test]
    fn hold_curve() {
        let mut lane = make_lane();
        lane.add_point(AutomationPoint::new(Beats(0.0), 0.0).with_curve(CurveType::Hold));
        lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));

        assert_eq!(lane.value_at(Beats(2.0)), 0.0); // Holds at 0
        assert_eq!(lane.value_at(Beats(3.9)), 0.0);
        assert_eq!(lane.value_at(Beats(4.0)), 1.0);
    }

    #[test]
    fn values_for_range() {
        let mut lane = make_lane();
        lane.add_point(AutomationPoint::new(Beats(0.0), 0.0));
        lane.add_point(AutomationPoint::new(Beats(4.0), 1.0));

        let values = lane.values_for_range(Beats(0.0), Beats(4.0), 5);
        assert_eq!(values.len(), 5);
        assert!((values[0] - 0.0).abs() < 0.01);
        assert!((values[2] - 0.5).abs() < 0.01);
    }
}
