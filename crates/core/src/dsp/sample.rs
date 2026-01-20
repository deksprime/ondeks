/// A single audio sample, ranging from -1.0 to 1.0.
///
/// Values outside this range are technically valid but will clip
/// when sent to audio hardware.
pub type Sample = f32;

/// The "zero" sample (silence)
pub const SILENCE: Sample = 0.0;

/// Convert decibels to linear amplitude.
/// 
/// # Examples
/// - 0 dB → 1.0
/// - -6 dB → ~0.5
/// - -inf dB → 0.0
pub fn db_to_linear(db: f32) -> Sample {
    if db <= -100.0 {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

/// Convert linear amplitude to decibels.
///
/// # Examples
/// - 1.0 → 0 dB
/// - 0.5 → ~-6 dB
/// - 0.0 → -inf dB
pub fn linear_to_db(linear: Sample) -> f32 {
    if linear <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * linear.log10()
    }
}

/// Clamp a sample to the valid range [-1.0, 1.0]
pub fn clamp(sample: Sample) -> Sample {
    sample.clamp(-1.0, 1.0)
}

/// Linear interpolation between two samples.
/// t=0 returns a, t=1 returns b.
pub fn lerp(a: Sample, b: Sample, t: f32) -> Sample {
    a + (b - a) * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_to_linear_unity_gain() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn db_to_linear_minus_6db() {
        // -6dB ≈ 0.501 amplitude
        assert!((db_to_linear(-6.0) - 0.501).abs() < 0.01);
    }

    #[test]
    fn db_to_linear_silence() {
        assert_eq!(db_to_linear(-100.0), 0.0);
        assert_eq!(db_to_linear(f32::NEG_INFINITY), 0.0);
    }

    #[test]
    fn linear_to_db_unity() {
        assert!((linear_to_db(1.0) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn linear_to_db_zero() {
        assert_eq!(linear_to_db(0.0), f32::NEG_INFINITY);
    }

    #[test]
    fn linear_to_db_roundtrip() {
        let original = 0.7;
        let roundtrip = db_to_linear(linear_to_db(original));
        assert!((original - roundtrip).abs() < 1e-6);
    }

    #[test]
    fn clamp_within_range() {
        assert_eq!(clamp(0.5), 0.5);
        assert_eq!(clamp(-0.5), -0.5);
    }

    #[test]
    fn clamp_exceeds_positive() {
        assert_eq!(clamp(1.5), 1.0);
        assert_eq!(clamp(100.0), 1.0);
    }

    #[test]
    fn clamp_exceeds_negative() {
        assert_eq!(clamp(-2.0), -1.0);
        assert_eq!(clamp(-100.0), -1.0);
    }

    #[test]
    fn lerp_endpoints() {
        assert_eq!(lerp(0.0, 1.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 1.0, 1.0), 1.0);
    }

    #[test]
    fn lerp_midpoint() {
        assert!((lerp(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        assert!((lerp(-1.0, 1.0, 0.5) - 0.0).abs() < 1e-6);
    }
}
