use crate::ids::{NodeId, PortId};

/// Identifies a specific port on a specific node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortAddress {
    pub node: NodeId,
    pub port: PortId,
}

impl PortAddress {
    pub fn new(node: NodeId, port: PortId) -> Self {
        Self { node, port }
    }
}

/// A connection between two ports.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Connection {
    pub source: PortAddress,
    pub target: PortAddress,
}

impl Connection {
    pub fn new(source: PortAddress, target: PortAddress) -> Self {
        Self { source, target }
    }

    /// Create a connection between two nodes using port indices.
    pub fn between(
        source_node: NodeId,
        source_port: PortId,
        target_node: NodeId,
        target_port: PortId,
    ) -> Self {
        Self {
            source: PortAddress::new(source_node, source_port),
            target: PortAddress::new(target_node, target_port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_equality() {
        let a = Connection::new(
            PortAddress::new(NodeId::from_raw(1), PortId::from_raw(0)),
            PortAddress::new(NodeId::from_raw(2), PortId::from_raw(0)),
        );
        let b = Connection::new(
            PortAddress::new(NodeId::from_raw(1), PortId::from_raw(0)),
            PortAddress::new(NodeId::from_raw(2), PortId::from_raw(0)),
        );
        assert_eq!(a, b);
    }

    #[test]
    fn connection_inequality() {
        let a = Connection::new(
            PortAddress::new(NodeId::from_raw(1), PortId::from_raw(0)),
            PortAddress::new(NodeId::from_raw(2), PortId::from_raw(0)),
        );
        let b = Connection::new(
            PortAddress::new(NodeId::from_raw(1), PortId::from_raw(0)),
            PortAddress::new(NodeId::from_raw(3), PortId::from_raw(0)),
        );
        assert_ne!(a, b);
    }
}
