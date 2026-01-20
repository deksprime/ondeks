use crate::ids::PortId;

/// The type of data a port carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    /// Audio signal (buffer of samples).
    Audio,
    /// MIDI events.
    Midi,
    /// Control value (single f32, for automation).
    Control,
}

/// Describes an input port on a node.
#[derive(Debug, Clone)]
pub struct InputPort {
    pub id: PortId,
    pub name: String,
    pub port_type: PortType,
}

/// Describes an output port on a node.
#[derive(Debug, Clone)]
pub struct OutputPort {
    pub id: PortId,
    pub name: String,
    pub port_type: PortType,
}

impl InputPort {
    /// Create an audio input port.
    pub fn audio(name: impl Into<String>) -> Self {
        Self {
            id: PortId::generate(),
            name: name.into(),
            port_type: PortType::Audio,
        }
    }

    /// Create a MIDI input port.
    pub fn midi(name: impl Into<String>) -> Self {
        Self {
            id: PortId::generate(),
            name: name.into(),
            port_type: PortType::Midi,
        }
    }

    /// Create a control input port.
    pub fn control(name: impl Into<String>) -> Self {
        Self {
            id: PortId::generate(),
            name: name.into(),
            port_type: PortType::Control,
        }
    }
}

impl OutputPort {
    /// Create an audio output port.
    pub fn audio(name: impl Into<String>) -> Self {
        Self {
            id: PortId::generate(),
            name: name.into(),
            port_type: PortType::Audio,
        }
    }

    /// Create a MIDI output port.
    pub fn midi(name: impl Into<String>) -> Self {
        Self {
            id: PortId::generate(),
            name: name.into(),
            port_type: PortType::Midi,
        }
    }

    /// Create a control output port.
    pub fn control(name: impl Into<String>) -> Self {
        Self {
            id: PortId::generate(),
            name: name.into(),
            port_type: PortType::Control,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_types_are_distinct() {
        assert_ne!(PortType::Audio, PortType::Midi);
        assert_ne!(PortType::Midi, PortType::Control);
        assert_ne!(PortType::Audio, PortType::Control);
    }

    #[test]
    fn input_port_creation() {
        let audio = InputPort::audio("main");
        assert_eq!(audio.port_type, PortType::Audio);
        assert_eq!(audio.name, "main");

        let midi = InputPort::midi("midi_in");
        assert_eq!(midi.port_type, PortType::Midi);
    }

    #[test]
    fn output_port_creation() {
        let audio = OutputPort::audio("out");
        assert_eq!(audio.port_type, PortType::Audio);

        let control = OutputPort::control("envelope");
        assert_eq!(control.port_type, PortType::Control);
    }

    #[test]
    fn ports_have_unique_ids() {
        let a = InputPort::audio("a");
        let b = InputPort::audio("b");
        assert_ne!(a.id, b.id);
    }
}
