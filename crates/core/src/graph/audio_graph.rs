use std::collections::HashMap;
use crate::ids::NodeId;
use crate::error::GraphError;
use super::node::AudioNode;
use super::connection::{Connection, PortAddress};
use super::nodes::OutputNode;
use super::topology::topological_sort;

/// The audio processing graph.
pub struct AudioGraph {
    nodes: HashMap<NodeId, Box<dyn AudioNode>>,
    connections: Vec<Connection>,
    output_node: NodeId,
    processing_order: Vec<NodeId>,
    is_dirty: bool,
}

impl AudioGraph {
    /// Create a new empty graph with a stereo output.
    pub fn new() -> Self {
        let output = OutputNode::stereo();
        let output_id = output.id();
        
        let mut nodes: HashMap<NodeId, Box<dyn AudioNode>> = HashMap::new();
        nodes.insert(output_id, Box::new(output));
        
        Self {
            nodes,
            connections: Vec::new(),
            output_node: output_id,
            processing_order: vec![output_id],
            is_dirty: false,
        }
    }

    /// Add a node to the graph, returning its ID.
    pub fn add_node<N: AudioNode + 'static>(&mut self, node: N) -> NodeId {
        let id = NodeId::generate();
        self.nodes.insert(id, Box::new(node));
        self.is_dirty = true;
        id
    }

    /// Add a node that already has an ID.
    pub fn add_node_with_id(&mut self, id: NodeId, node: Box<dyn AudioNode>) {
        self.nodes.insert(id, node);
        self.is_dirty = true;
    }

    /// Remove a node and all its connections.
    pub fn remove_node(&mut self, id: NodeId) -> Result<(), GraphError> {
        if id == self.output_node {
            return Err(GraphError::CannotRemoveOutput);
        }
        
        if self.nodes.remove(&id).is_none() {
            return Err(GraphError::NodeNotFound(id));
        }
        
        // Remove all connections involving this node
        self.connections.retain(|c| {
            c.source.node != id && c.target.node != id
        });
        
        self.is_dirty = true;
        Ok(())
    }

    /// Get a reference to a node.
    pub fn get_node(&self, id: NodeId) -> Option<&dyn AudioNode> {
        self.nodes.get(&id).map(|n| n.as_ref())
    }

    /// Get a mutable reference to a node.
    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut dyn AudioNode> {
        // SAFETY: The Box<dyn AudioNode> is 'static, but we're returning a reference
        // with the lifetime of self. This is safe because the Box is owned by self.
        self.nodes.get_mut(&id).map(|n| n.as_mut() as &mut dyn AudioNode)
    }

    /// Connect two ports.
    pub fn connect(&mut self, source: PortAddress, target: PortAddress) -> Result<(), GraphError> {
        // Validate source node exists
        let source_node = self.nodes.get(&source.node)
            .ok_or(GraphError::NodeNotFound(source.node))?;
        
        // Validate source port exists
        if !source_node.outputs().iter().any(|p| p.id == source.port) {
            return Err(GraphError::PortNotFound { 
                node: source.node, 
                port: source.port 
            });
        }
        
        // Validate target node exists
        let target_node = self.nodes.get(&target.node)
            .ok_or(GraphError::NodeNotFound(target.node))?;
        
        // Validate target port exists
        if !target_node.inputs().iter().any(|p| p.id == target.port) {
            return Err(GraphError::PortNotFound { 
                node: target.node, 
                port: target.port 
            });
        }
        
        let connection = Connection::new(source, target);
        
        // Check for cycles
        let mut test_connections = self.connections.clone();
        test_connections.push(connection.clone());
        if topological_sort(&self.nodes, &test_connections, self.output_node).is_err() {
            return Err(GraphError::CycleDetected);
        }
        
        self.connections.push(connection);
        self.is_dirty = true;
        Ok(())
    }

    /// Disconnect two ports.
    pub fn disconnect(&mut self, source: PortAddress, target: PortAddress) -> Result<(), GraphError> {
        let connection = Connection::new(source, target);
        
        if let Some(pos) = self.connections.iter().position(|c| *c == connection) {
            self.connections.remove(pos);
            self.is_dirty = true;
            Ok(())
        } else {
            Err(GraphError::ConnectionNotFound)
        }
    }

    /// Get all connections from a node's outputs.
    pub fn connections_from(&self, node: NodeId) -> impl Iterator<Item = &Connection> {
        self.connections.iter().filter(move |c| c.source.node == node)
    }

    /// Get all connections to a node's inputs.
    pub fn connections_to(&self, node: NodeId) -> impl Iterator<Item = &Connection> {
        self.connections.iter().filter(move |c| c.target.node == node)
    }

    /// Get the output node ID.
    pub fn output(&self) -> NodeId {
        self.output_node
    }

    /// Get nodes in processing order (topologically sorted).
    /// Recalculates if the graph has changed.
    pub fn processing_order(&mut self) -> Result<&[NodeId], GraphError> {
        if self.is_dirty {
            self.processing_order = topological_sort(
                &self.nodes, 
                &self.connections, 
                self.output_node
            )?;
            self.is_dirty = false;
        }
        Ok(&self.processing_order)
    }

    /// Get all node IDs.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes.keys().copied()
    }

    /// Get all connections.
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }
}

impl Default for AudioGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::PortId;
    use crate::graph::nodes::GainNode;

    #[test]
    fn new_graph_has_output_node() {
        let graph = AudioGraph::new();
        assert!(graph.get_node(graph.output()).is_some());
    }

    #[test]
    fn add_and_retrieve_node() {
        let mut graph = AudioGraph::new();
        let gain = GainNode::new();
        let gain_id = gain.id();
        graph.add_node_with_id(gain_id, Box::new(gain));
        
        assert!(graph.get_node(gain_id).is_some());
        assert_eq!(graph.get_node(gain_id).unwrap().name(), "Gain");
    }

    #[test]
    fn remove_node_removes_connections() {
        let mut graph = AudioGraph::new();
        let gain = GainNode::new();
        let gain_id = gain.id();
        let gain_out = gain.outputs()[0].id;
        graph.add_node_with_id(gain_id, Box::new(gain));
        
        let output = graph.get_node(graph.output()).unwrap();
        let output_in = output.inputs()[0].id;
        
        graph.connect(
            PortAddress::new(gain_id, gain_out),
            PortAddress::new(graph.output(), output_in),
        ).unwrap();
        
        assert_eq!(graph.connections_to(graph.output()).count(), 1);
        
        graph.remove_node(gain_id).unwrap();
        
        assert!(graph.get_node(gain_id).is_none());
        assert_eq!(graph.connections_to(graph.output()).count(), 0);
    }

    #[test]
    fn cannot_remove_output_node() {
        let mut graph = AudioGraph::new();
        let result = graph.remove_node(graph.output());
        assert!(matches!(result, Err(GraphError::CannotRemoveOutput)));
    }

    #[test]
    fn connect_validates_nodes_exist() {
        let mut graph = AudioGraph::new();
        let fake_node = NodeId::generate();
        let fake_port = PortId::generate();
        
        let result = graph.connect(
            PortAddress::new(fake_node, fake_port),
            PortAddress::new(graph.output(), fake_port),
        );
        
        assert!(matches!(result, Err(GraphError::NodeNotFound(_))));
    }
}
