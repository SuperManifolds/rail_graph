use super::RailwayGraph;
use crate::models::node::Node;
use petgraph::visit::EdgeRef;

/// Extension trait for node-related operations on `RailwayGraph`
pub trait Nodes {
    /// Get all nodes (stations and junctions) in order by traversing the graph
    /// Performs a breadth-first traversal starting from the first node
    fn get_all_nodes_ordered(&self) -> Vec<Node>;
}

impl Nodes for RailwayGraph {
    fn get_all_nodes_ordered(&self) -> Vec<Node> {
        if self.graph.node_count() == 0 {
            return Vec::new();
        }

        let mut ordered = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let Some(start_node) = self.graph.node_indices().next() else {
            return Vec::new();
        };

        // BFS traversal
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start_node);
        seen.insert(start_node);

        while let Some(node_idx) = queue.pop_front() {
            if let Some(node) = self.graph.node_weight(node_idx) {
                ordered.push(node.clone());
            }

            // Add neighbors
            for edge in self.graph.edges(node_idx) {
                let target = edge.target();
                if seen.insert(target) {
                    queue.push_back(target);
                }
            }
        }

        // Add any remaining disconnected nodes
        for node_idx in self.graph.node_indices() {
            if !seen.insert(node_idx) {
                continue;
            }
            if let Some(node) = self.graph.node_weight(node_idx) {
                ordered.push(node.clone());
            }
        }

        ordered
    }
}
