/// A musical time signature (e.g., 4/4, 3/4, 6/8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSignature {
    /// Beats per bar (numerator).
    pub numerator: u8,
    /// Note value that gets one beat (denominator).
    /// Common values: 4 (quarter), 8 (eighth).
    pub denominator: u8,
}

impl TimeSignature {
    /// Common time (4/4)
    pub const FOUR_FOUR: Self = Self { numerator: 4, denominator: 4 };
    /// Waltz time (3/4)
    pub const THREE_FOUR: Self = Self { numerator: 3, denominator: 4 };
    /// Compound duple (6/8)
    pub const SIX_EIGHT: Self = Self { numerator: 6, denominator: 8 };

    /// Create a new time signature.
    /// 
    /// Returns None if:
    /// - numerator is 0
    /// - denominator is 0
    /// - denominator is not a power of 2
    pub fn new(numerator: u8, denominator: u8) -> Option<Self> {
        if numerator == 0 || denominator == 0 {
            return None;
        }
        // Check if denominator is power of 2
        if !denominator.is_power_of_two() {
            return None;
        }
        Some(Self { numerator, denominator })
    }

    /// Number of beats per bar (in the time signature's beat unit).
    pub fn beats_per_bar(&self) -> f64 {
        self.numerator as f64
    }

    /// Quarter notes per bar.
    /// 
    /// - 4/4 → 4 quarter notes
    /// - 3/4 → 3 quarter notes
    /// - 6/8 → 3 quarter notes (6 eighths = 3 quarters)
    pub fn quarter_notes_per_bar(&self) -> f64 {
        self.numerator as f64 * (4.0 / self.denominator as f64)
    }

    /// Duration of one bar in beats (quarter notes).
    pub fn bar_duration(&self) -> super::time::Beats {
        super::time::Beats(self.quarter_notes_per_bar())
    }
}

impl Default for TimeSignature {
    fn default() -> Self {
        Self::FOUR_FOUR
    }
}

impl std::fmt::Display for TimeSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.numerator, self.denominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_time_signatures() {
        assert!(TimeSignature::new(4, 4).is_some());
        assert!(TimeSignature::new(3, 4).is_some());
        assert!(TimeSignature::new(6, 8).is_some());
        assert!(TimeSignature::new(7, 8).is_some());
        assert!(TimeSignature::new(2, 2).is_some());
        assert!(TimeSignature::new(12, 8).is_some());
    }

    #[test]
    fn invalid_denominators() {
        assert!(TimeSignature::new(4, 3).is_none()); // Not power of 2
        assert!(TimeSignature::new(4, 5).is_none());
        assert!(TimeSignature::new(4, 6).is_none());
        assert!(TimeSignature::new(4, 0).is_none());
    }

    #[test]
    fn invalid_numerator() {
        assert!(TimeSignature::new(0, 4).is_none());
    }

    #[test]
    fn quarter_notes_per_bar() {
        assert_eq!(TimeSignature::FOUR_FOUR.quarter_notes_per_bar(), 4.0);
        assert_eq!(TimeSignature::THREE_FOUR.quarter_notes_per_bar(), 3.0);
        assert_eq!(TimeSignature::SIX_EIGHT.quarter_notes_per_bar(), 3.0);
        
        let ts_2_2 = TimeSignature::new(2, 2).unwrap();
        assert_eq!(ts_2_2.quarter_notes_per_bar(), 4.0); // 2 half notes = 4 quarters
    }

    #[test]
    fn beats_per_bar() {
        assert_eq!(TimeSignature::FOUR_FOUR.beats_per_bar(), 4.0);
        assert_eq!(TimeSignature::SIX_EIGHT.beats_per_bar(), 6.0);
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", TimeSignature::FOUR_FOUR), "4/4");
        assert_eq!(format!("{}", TimeSignature::SIX_EIGHT), "6/8");
    }

    #[test]
    fn default_is_4_4() {
        assert_eq!(TimeSignature::default(), TimeSignature::FOUR_FOUR);
    }
}
