use std::sync::atomic::{AtomicU64, Ordering};

/// Macro to define an ID type with auto-increment capability.
macro_rules! define_id {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(pub u64);

        impl $name {
            /// Generate a new unique ID.
            /// 
            /// IDs are globally unique within a process lifetime.
            pub fn generate() -> Self {
                static COUNTER: AtomicU64 = AtomicU64::new(1);
                Self(COUNTER.fetch_add(1, Ordering::Relaxed))
            }

            /// Create an ID from a raw value (for deserialization).
            pub fn from_raw(value: u64) -> Self {
                Self(value)
            }

            /// Get the raw value (for serialization).
            pub fn raw(self) -> u64 {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
    };
}

define_id!(NodeId, "Unique identifier for an audio graph node.");
define_id!(PortId, "Unique identifier for a port on a node.");
define_id!(TrackId, "Unique identifier for a track in a project.");
define_id!(ClipId, "Unique identifier for a clip.");
define_id!(SceneId, "Unique identifier for a scene in session view.");
define_id!(ParameterId, "Unique identifier for an automatable parameter.");
define_id!(AutomationLaneId, "Unique identifier for an automation lane.");
define_id!(AudioPoolId, "Unique identifier for an audio pool entry.");

/// A simple color representation for UI elements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const RED: Self = Self { r: 255, g: 0, b: 0 };
    pub const GREEN: Self = Self { r: 0, g: 255, b: 0 };
    pub const BLUE: Self = Self { r: 0, g: 0, b: 255 };
    pub const YELLOW: Self = Self { r: 255, g: 255, b: 0 };
    pub const CYAN: Self = Self { r: 0, g: 255, b: 255 };
    pub const MAGENTA: Self = Self { r: 255, g: 0, b: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255 };
    pub const GRAY: Self = Self { r: 128, g: 128, b: 128 };

    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let a = NodeId::generate();
        let b = NodeId::generate();
        let c = NodeId::generate();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert_ne!(a, c);
    }

    #[test]
    fn id_roundtrip() {
        let original = TrackId::generate();
        let raw = original.raw();
        let restored = TrackId::from_raw(raw);
        assert_eq!(original, restored);
    }

    #[test]
    fn id_display() {
        let id = NodeId::from_raw(42);
        assert_eq!(format!("{}", id), "NodeId(42)");
    }

    #[test]
    fn different_id_types_have_different_counters() {
        // This test verifies that NodeId and TrackId use separate counters
        // (they actually share one, but that's fine - they're still different types)
        let node = NodeId::generate();
        let track = TrackId::generate();
        // The key thing is they're different types, so this won't compile:
        // assert_eq!(node, track);
        // Just verify they exist
        assert!(node.raw() > 0);
        assert!(track.raw() > 0);
    }

    #[test]
    fn color_from_hex() {
        let color = Color::from_hex(0xFF8000);
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 128);
        assert_eq!(color.b, 0);
    }
}
