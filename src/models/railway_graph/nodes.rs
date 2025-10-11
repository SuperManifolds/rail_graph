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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Stations, Junctions, Tracks, Junction, Track, TrackDirection};

    #[test]
    fn test_get_all_nodes_ordered_empty_graph() {
        let graph = RailwayGraph::new();
        let nodes = graph.get_all_nodes_ordered();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_get_all_nodes_ordered_single_node() {
        let mut graph = RailwayGraph::new();
        graph.add_or_get_station("Station A".to_string());

        let nodes = graph.get_all_nodes_ordered();
        assert_eq!(nodes.len(), 1);

        match &nodes[0] {
            Node::Station(station) => {
                assert_eq!(station.name, "Station A");
            }
            Node::Junction(_) => panic!("Expected station node"),
        }
    }

    #[test]
    fn test_get_all_nodes_ordered_connected_graph() {
        let mut graph = RailwayGraph::new();

        // Create A -> B -> C
        let a = graph.add_or_get_station("A".to_string());
        let b = graph.add_or_get_station("B".to_string());
        let c = graph.add_or_get_station("C".to_string());

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        graph.add_track(a, b, tracks.clone());
        graph.add_track(b, c, tracks);

        let nodes = graph.get_all_nodes_ordered();
        assert_eq!(nodes.len(), 3);

        // BFS traversal should start from first node and visit connected nodes
        // All nodes should be present
        let names: Vec<String> = nodes.iter().filter_map(|n| {
            match n {
                Node::Station(station) => Some(station.name.clone()),
                Node::Junction(_) => None,
            }
        }).collect();

        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"B".to_string()));
        assert!(names.contains(&"C".to_string()));
    }

    #[test]
    fn test_get_all_nodes_ordered_disconnected_components() {
        let mut graph = RailwayGraph::new();

        // Create two disconnected components: station1 -> station2 and station3 -> station4
        let station1 = graph.add_or_get_station("A".to_string());
        let station2 = graph.add_or_get_station("B".to_string());
        let station3 = graph.add_or_get_station("C".to_string());
        let station4 = graph.add_or_get_station("D".to_string());

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        graph.add_track(station1, station2, tracks.clone());
        graph.add_track(station3, station4, tracks);

        let nodes = graph.get_all_nodes_ordered();
        assert_eq!(nodes.len(), 4);

        // All nodes should be present even though they're disconnected
        let names: Vec<String> = nodes.iter().filter_map(|n| {
            match n {
                Node::Station(station) => Some(station.name.clone()),
                Node::Junction(_) => None,
            }
        }).collect();

        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"B".to_string()));
        assert!(names.contains(&"C".to_string()));
        assert!(names.contains(&"D".to_string()));
    }

    #[test]
    fn test_get_all_nodes_ordered_mixed_stations_and_junctions() {
        let mut graph = RailwayGraph::new();

        // Create A -> Junction -> B
        let a = graph.add_or_get_station("A".to_string());
        let j = graph.add_junction(Junction {
            name: Some("Junction".to_string()),
            position: None,
            routing_rules: vec![],
        });
        let b = graph.add_or_get_station("B".to_string());

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        graph.add_track(a, j, tracks.clone());
        graph.add_track(j, b, tracks);

        let nodes = graph.get_all_nodes_ordered();
        assert_eq!(nodes.len(), 3);

        // Should have 2 stations and 1 junction
        let station_count = nodes.iter().filter(|n| matches!(n, Node::Station(_))).count();
        let junction_count = nodes.iter().filter(|n| matches!(n, Node::Junction(_))).count();

        assert_eq!(station_count, 2);
        assert_eq!(junction_count, 1);
    }
}
