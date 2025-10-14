use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use super::RailwayGraph;
use crate::models::station::{StationNode, default_platforms};
use crate::models::node::Node;

/// Extension trait for station-related operations on `RailwayGraph`
pub trait Stations {
    /// Add a station node if it doesn't exist, return its `NodeIndex`
    fn add_or_get_station(&mut self, name: String) -> NodeIndex;

    /// Update station position
    fn set_station_position(&mut self, index: NodeIndex, position: (f64, f64));

    /// Get station position
    fn get_station_position(&self, index: NodeIndex) -> Option<(f64, f64)>;

    /// Get station name by `NodeIndex`
    fn get_station_name(&self, index: NodeIndex) -> Option<&str>;

    /// Get `NodeIndex` by station name
    fn get_station_index(&self, name: &str) -> Option<NodeIndex>;

    /// Get all edge indices connected to a station
    fn get_station_edges(&self, index: NodeIndex) -> Vec<usize>;

    /// Find stations connected through a given station
    /// Returns a Vec of (`station_before`, `station_after`, tracks) tuples
    fn find_connections_through_station(&self, station_idx: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Vec<crate::models::track::Track>)>;

    /// Delete a station and reconnect around it
    /// Returns (`removed_edges`, `bypass_mapping`) where `bypass_mapping` maps (`old_edge1`, `old_edge2`) -> `new_edge`
    fn delete_station(&mut self, index: NodeIndex) -> (Vec<usize>, std::collections::HashMap<(usize, usize), usize>);

    /// Get all stations in order by traversing the graph (deprecated, use get_all_nodes_ordered)
    /// Performs a breadth-first traversal starting from the first station
    /// Returns Vec<(`NodeIndex`, `StationNode`)>
    fn get_all_stations_ordered(&self) -> Vec<(NodeIndex, StationNode)>;

    /// Get all nodes (stations and junctions) in order by traversing the graph
    /// Performs a breadth-first traversal starting from the first node
    /// Returns Vec<(`NodeIndex`, `Node`)>
    fn get_all_nodes_ordered(&self) -> Vec<(NodeIndex, Node)>;

    /// Get all station names in order
    fn get_all_station_names(&self) -> Vec<String>;
}

impl Stations for RailwayGraph {
    fn add_or_get_station(&mut self, name: String) -> NodeIndex {
        if let Some(&index) = self.station_name_to_index.get(&name) {
            index
        } else {
            let index = self.graph.add_node(Node::Station(StationNode {
                name: name.clone(),
                position: None,
                passing_loop: false,
                platforms: default_platforms(),
            }));
            self.station_name_to_index.insert(name, index);
            index
        }
    }

    fn set_station_position(&mut self, index: NodeIndex, position: (f64, f64)) {
        if let Some(node) = self.graph.node_weight_mut(index) {
            node.set_position(Some(position));
        }
    }

    fn get_station_position(&self, index: NodeIndex) -> Option<(f64, f64)> {
        self.graph.node_weight(index).and_then(Node::position)
    }

    fn get_station_name(&self, index: NodeIndex) -> Option<&str> {
        self.graph.node_weight(index).and_then(|node| {
            node.as_station().map(|s| s.name.as_str())
        })
    }

    fn get_station_index(&self, name: &str) -> Option<NodeIndex> {
        self.station_name_to_index.get(name).copied()
    }

    fn get_station_edges(&self, index: NodeIndex) -> Vec<usize> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

        self.graph.edges(index)
            .map(|e| e.id().index())
            .chain(self.graph.edges_directed(index, Direction::Incoming).map(|e| e.id().index()))
            .collect()
    }

    fn find_connections_through_station(&self, station_idx: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Vec<crate::models::track::Track>)> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;
        use crate::models::track::{Track, TrackDirection};

        let mut connections = Vec::new();

        // Get incoming edges (edges pointing to this station)
        let incoming: Vec<_> = self.graph.edges_directed(station_idx, Direction::Incoming)
            .map(|e| (e.source(), &e.weight().tracks))
            .collect();

        // Get outgoing edges (edges from this station)
        let outgoing: Vec<_> = self.graph.edges(station_idx)
            .map(|e| (e.target(), &e.weight().tracks))
            .collect();

        // Create connections from each incoming station to each outgoing station
        for (from_station, from_tracks) in &incoming {
            for (to_station, to_tracks) in &outgoing {
                // Use the max number of tracks from either segment
                let track_count = from_tracks.len().max(to_tracks.len());
                let tracks = (0..track_count).map(|_| Track { direction: TrackDirection::Bidirectional }).collect();
                connections.push((*from_station, *to_station, tracks));
            }
        }

        connections
    }

    fn delete_station(&mut self, index: NodeIndex) -> (Vec<usize>, std::collections::HashMap<(usize, usize), usize>) {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;
        use super::tracks::Tracks;

        // Find connections through this station to create bypass edges
        let connections = self.find_connections_through_station(index);

        // Create bypass edges and track the mapping
        let mut bypass_mapping = std::collections::HashMap::new();

        for (from_station, to_station, tracks) in connections {
            // Find the incoming and outgoing edges for this connection
            let incoming_edge = self.graph.edges_directed(index, Direction::Incoming)
                .find(|e| e.source() == from_station)
                .map(|e| e.id().index());

            let outgoing_edge = self.graph.edges(index)
                .find(|e| e.target() == to_station)
                .map(|e| e.id().index());

            if let (Some(edge1), Some(edge2)) = (incoming_edge, outgoing_edge) {
                let new_edge = self.add_track(from_station, to_station, tracks);
                bypass_mapping.insert((edge1, edge2), new_edge.index());
            }
        }

        // Get edges that will be removed
        let removed_edges = self.get_station_edges(index);

        // Remove station from name mapping
        if let Some(node) = self.graph.node_weight(index) {
            if let Some(station) = node.as_station() {
                self.station_name_to_index.remove(&station.name);
            }
        }

        // Remove the station node (this also removes all connected edges)
        self.graph.remove_node(index);

        (removed_edges, bypass_mapping)
    }

    fn get_all_stations_ordered(&self) -> Vec<(NodeIndex, StationNode)> {
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
                if let Some(station) = node.as_station() {
                    ordered.push((node_idx, station.clone()));
                }
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
            let Some(node) = self.graph.node_weight(node_idx) else { continue };
            let Some(station) = node.as_station() else { continue };
            ordered.push((node_idx, station.clone()));
        }

        ordered
    }

    fn get_all_nodes_ordered(&self) -> Vec<(NodeIndex, Node)> {
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
                ordered.push((node_idx, node.clone()));
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
            let Some(node) = self.graph.node_weight(node_idx) else { continue };
            ordered.push((node_idx, node.clone()));
        }

        ordered
    }

    fn get_all_station_names(&self) -> Vec<String> {
        self.get_all_stations_ordered()
            .into_iter()
            .map(|(_, s)| s.name)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RailwayGraph;
    use crate::models::track::{Track, TrackDirection};
    use super::super::tracks::Tracks;

    #[test]
    fn test_add_station() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());

        assert_eq!(graph.graph.node_count(), 2);
        assert_ne!(idx1, idx2);
        assert_eq!(graph.get_station_name(idx1), Some("Station A"));
        assert_eq!(graph.get_station_name(idx2), Some("Station B"));
    }

    #[test]
    fn test_add_or_get_station_returns_existing() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station A".to_string());

        assert_eq!(idx1, idx2);
        assert_eq!(graph.graph.node_count(), 1);
    }

    #[test]
    fn test_station_position() {
        let mut graph = RailwayGraph::new();
        let idx = graph.add_or_get_station("Station A".to_string());

        assert_eq!(graph.get_station_position(idx), None);

        graph.set_station_position(idx, (100.0, 200.0));
        assert_eq!(graph.get_station_position(idx), Some((100.0, 200.0)));
    }

    #[test]
    fn test_get_station_index() {
        let mut graph = RailwayGraph::new();
        let idx = graph.add_or_get_station("Station A".to_string());

        assert_eq!(graph.get_station_index("Station A"), Some(idx));
        assert_eq!(graph.get_station_index("Nonexistent"), None);
    }

    #[test]
    fn test_get_all_stations_ordered() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        // Create a linear graph A -> B -> C
        graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let stations = graph.get_all_stations_ordered();
        assert_eq!(stations.len(), 3);
        assert_eq!(stations[0].1.name, "Station A");
        assert_eq!(stations[1].1.name, "Station B");
        assert_eq!(stations[2].1.name, "Station C");
    }

    #[test]
    fn test_get_all_stations_ordered_empty_graph() {
        let graph = RailwayGraph::new();
        let stations = graph.get_all_stations_ordered();
        assert_eq!(stations.len(), 0);
    }

    #[test]
    fn test_delete_station_creates_bypass() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        // Create A -> B -> C
        graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        assert_eq!(graph.graph.node_count(), 3);
        assert_eq!(graph.graph.edge_count(), 2);

        // Delete B, should create A -> C bypass
        let (removed_edges, bypass_mapping) = graph.delete_station(idx2);

        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(removed_edges.len(), 2);
        assert_eq!(bypass_mapping.len(), 1);

        // Should have one edge connecting A -> C
        assert_eq!(graph.graph.edge_count(), 1);
        assert_eq!(graph.get_station_index("Station B"), None);
    }

    #[test]
    fn test_get_station_edges() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let edges = graph.get_station_edges(idx2);
        assert_eq!(edges.len(), 2); // Station B has 2 connected edges

        let edges = graph.get_station_edges(idx1);
        assert_eq!(edges.len(), 1); // Station A has 1 connected edge
    }

    #[test]
    fn test_find_connections_through_station() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let connections = graph.find_connections_through_station(idx2);
        assert_eq!(connections.len(), 1);
        assert_eq!(connections[0].0, idx1); // from A
        assert_eq!(connections[0].1, idx3); // to C
    }
}
