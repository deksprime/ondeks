use std::collections::{HashMap, HashSet, VecDeque};
use crate::ids::NodeId;
use crate::error::GraphError;
use super::node::AudioNode;
use super::connection::Connection;

/// Compute the processing order for a graph using Kahn's algorithm.
/// Returns nodes in order such that dependencies come before dependents.
pub fn topological_sort(
    nodes: &HashMap<NodeId, Box<dyn AudioNode>>,
    connections: &[Connection],
    output: NodeId,
) -> Result<Vec<NodeId>, GraphError> {
    // Build adjacency list (reverse: target -> sources)
    let mut dependencies: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
    let mut dependents: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();
    
    for node_id in nodes.keys() {
        dependencies.insert(*node_id, HashSet::new());
        dependents.insert(*node_id, HashSet::new());
    }
    
    for conn in connections {
        // source -> target means target depends on source
        dependencies.get_mut(&conn.target.node)
            .map(|deps| deps.insert(conn.source.node));
        dependents.get_mut(&conn.source.node)
            .map(|deps| deps.insert(conn.target.node));
    }
    
    // Find all nodes reachable from output (walking backwards)
    let reachable = find_reachable_nodes(output, &dependencies);
    
    // Kahn's algorithm on reachable nodes only
    let mut in_degree: HashMap<NodeId, usize> = HashMap::new();
    for &node_id in &reachable {
        let deps = dependencies.get(&node_id).unwrap();
        let relevant_deps: usize = deps.iter()
            .filter(|d| reachable.contains(d))
            .count();
        in_degree.insert(node_id, relevant_deps);
    }
    
    // Start with nodes that have no dependencies
    let mut queue: VecDeque<NodeId> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();
    
    let mut result = Vec::new();
    
    while let Some(node_id) = queue.pop_front() {
        result.push(node_id);
        
        if let Some(deps) = dependents.get(&node_id) {
            for &dependent in deps {
                if !reachable.contains(&dependent) {
                    continue;
                }
                if let Some(deg) = in_degree.get_mut(&dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dependent);
                    }
                }
            }
        }
    }
    
    // Check for cycles (if not all reachable nodes are in result)
    if result.len() != reachable.len() {
        return Err(GraphError::CycleDetected);
    }
    
    Ok(result)
}

/// Find all nodes reachable from the given node by walking dependencies.
fn find_reachable_nodes(
    start: NodeId,
    dependencies: &HashMap<NodeId, HashSet<NodeId>>,
) -> HashSet<NodeId> {
    let mut reachable = HashSet::new();
    let mut stack = vec![start];
    
    while let Some(node) = stack.pop() {
        if reachable.insert(node) {
            if let Some(deps) = dependencies.get(&node) {
                for &dep in deps {
                    stack.push(dep);
                }
            }
        }
    }
    
    reachable
}

/// Check if adding a connection would create a cycle.
pub fn would_create_cycle(
    nodes: &HashMap<NodeId, Box<dyn AudioNode>>,
    connections: &[Connection],
    new_connection: &Connection,
    output: NodeId,
) -> bool {
    let mut test_connections = connections.to_vec();
    test_connections.push(new_connection.clone());
    topological_sort(nodes, &test_connections, output).is_err()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::nodes::{GainNode, OutputNode};

    fn make_test_graph() -> (HashMap<NodeId, Box<dyn AudioNode>>, NodeId) {
        let mut nodes: HashMap<NodeId, Box<dyn AudioNode>> = HashMap::new();
        let output = OutputNode::stereo();
        let output_id = output.id();
        nodes.insert(output_id, Box::new(output));
        (nodes, output_id)
    }

    #[test]
    fn simple_chain_sorts_correctly() {
        let (mut nodes, output_id) = make_test_graph();
        
        let a = GainNode::new();
        let a_id = a.id();
        let a_out = a.outputs()[0].id;
        nodes.insert(a_id, Box::new(a));
        
        let b = GainNode::new();
        let b_id = b.id();
        let b_out = b.outputs()[0].id;
        let b_in = b.inputs()[0].id;
        nodes.insert(b_id, Box::new(b));
        
        let output_in = nodes.get(&output_id).unwrap().inputs()[0].id;
        
        // A -> B -> Output
        let connections = vec![
            Connection::between(a_id, a_out, b_id, b_in),
            Connection::between(b_id, b_out, output_id, output_in),
        ];
        
        let order = topological_sort(&nodes, &connections, output_id).unwrap();
        
        // A must come before B, B must come before Output
        let a_pos = order.iter().position(|&id| id == a_id).unwrap();
        let b_pos = order.iter().position(|&id| id == b_id).unwrap();
        let out_pos = order.iter().position(|&id| id == output_id).unwrap();
        
        assert!(a_pos < b_pos);
        assert!(b_pos < out_pos);
    }

    #[test]
    fn disconnected_nodes_not_in_order() {
        let (mut nodes, output_id) = make_test_graph();
        
        let disconnected = GainNode::new();
        let disconnected_id = disconnected.id();
        nodes.insert(disconnected_id, Box::new(disconnected));
        
        let order = topological_sort(&nodes, &[], output_id).unwrap();
        
        // Disconnected node should not be in processing order
        assert!(!order.contains(&disconnected_id));
        assert!(order.contains(&output_id));
    }

    #[test]
    fn cycle_detected() {
        let (mut nodes, output_id) = make_test_graph();
        
        let a = GainNode::new();
        let a_id = a.id();
        let a_in = a.inputs()[0].id;
        let a_out = a.outputs()[0].id;
        nodes.insert(a_id, Box::new(a));
        
        let b = GainNode::new();
        let b_id = b.id();
        let b_in = b.inputs()[0].id;
        let b_out = b.outputs()[0].id;
        nodes.insert(b_id, Box::new(b));
        
        let output_in = nodes.get(&output_id).unwrap().inputs()[0].id;
        
        // A -> B -> A (cycle!), and A -> Output (so cycle is reachable)
        let connections = vec![
            Connection::between(a_id, a_out, b_id, b_in),
            Connection::between(b_id, b_out, a_id, a_in),
            Connection::between(a_id, a_out, output_id, output_in),
        ];
        
        let result = topological_sort(&nodes, &connections, output_id);
        assert!(matches!(result, Err(GraphError::CycleDetected)));
    }
}
