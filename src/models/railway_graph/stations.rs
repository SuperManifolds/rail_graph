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
    /// Returns a Vec of (`station_before`, `station_after`, tracks, `combined_distance`) tuples
    /// Only returns a bypass for stations with exactly 2 connections
    fn find_connections_through_station(&self, station_idx: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Vec<crate::models::track::Track>, Option<f64>)>;

    /// Delete a station and reconnect around it
    /// Returns (`removed_edges`, `bypass_mapping`) where `bypass_mapping` maps (`old_edge1`, `old_edge2`) -> `new_edge`
    fn delete_station(&mut self, index: NodeIndex) -> (Vec<usize>, std::collections::HashMap<(usize, usize), usize>);

    /// Get all stations in order by traversing the graph (deprecated, use `get_all_nodes_ordered`)
    /// Performs a breadth-first traversal starting from the first station
    /// Returns Vec<(`NodeIndex`, `StationNode`)>
    fn get_all_stations_ordered(&self) -> Vec<(NodeIndex, StationNode)>;

    /// Get all nodes (stations and junctions) in order by traversing the graph
    /// Performs a breadth-first traversal starting from the first node
    /// Returns Vec<(`NodeIndex`, `Node`)>
    fn get_all_nodes_ordered(&self) -> Vec<(NodeIndex, Node)>;

    /// Get all station names in order
    fn get_all_station_names(&self) -> Vec<String>;

    /// Find adjacent non-passing-loop stations for a passing loop
    /// Returns (`previous_station`, `next_station`) or None if not found
    fn find_adjacent_stations_for_passing_loop(&self, passing_loop_idx: NodeIndex) -> Option<(NodeIndex, NodeIndex)>;

    /// Calculate interpolated position for a passing loop
    /// Returns midpoint between adjacent non-passing-loop stations
    fn calculate_passing_loop_position(&self, passing_loop_idx: NodeIndex) -> Option<(f64, f64)>;
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
                label_position: None,
                external_id: None,
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
        let node = self.graph.node_weight(index)?;

        // Check if this is a passing loop
        if let Some(station) = node.as_station() {
            if station.passing_loop {
                // Calculate interpolated position for passing loop
                return self.calculate_passing_loop_position(index);
            }
        }

        // Regular station or junction - return stored position
        node.position()
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

    fn find_connections_through_station(&self, station_idx: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Vec<crate::models::track::Track>, Option<f64>)> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

        let mut connections = Vec::new();

        // Get all edges connected to this station
        let incoming: Vec<_> = self.graph.edges_directed(station_idx, Direction::Incoming)
            .map(|e| (e.source(), e.weight().tracks.clone(), e.weight().distance))
            .collect();

        let outgoing: Vec<_> = self.graph.edges(station_idx)
            .map(|e| (e.target(), e.weight().tracks.clone(), e.weight().distance))
            .collect();

        let total_connections = incoming.len() + outgoing.len();

        // Only create bypass for exactly 2 connections (1 incoming + 1 outgoing)
        if total_connections == 2 && incoming.len() == 1 && outgoing.len() == 1 {
            let (from_station, from_tracks, from_distance) = &incoming[0];
            let (to_station, to_tracks, to_distance) = &outgoing[0];

            // Choose track configuration with more tracks
            let tracks = if from_tracks.len() >= to_tracks.len() {
                from_tracks.clone()
            } else {
                to_tracks.clone()
            };

            // Combine distances if both are present
            let combined_distance = match (from_distance, to_distance) {
                (Some(d1), Some(d2)) => Some(d1 + d2),
                (Some(d), None) | (None, Some(d)) => Some(*d),
                (None, None) => None,
            };

            connections.push((*from_station, *to_station, tracks, combined_distance));
        }
        // For stations with more or fewer than 2 connections, don't create any bypass

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

        for (from_station, to_station, tracks, combined_distance) in connections {
            // Find the incoming and outgoing edges for this connection
            let incoming_edge = self.graph.edges_directed(index, Direction::Incoming)
                .find(|e| e.source() == from_station)
                .map(|e| e.id().index());

            let outgoing_edge = self.graph.edges(index)
                .find(|e| e.target() == to_station)
                .map(|e| e.id().index());

            let Some(edge1) = incoming_edge else { continue };
            let Some(edge2) = outgoing_edge else { continue };

            let new_edge = self.add_track(from_station, to_station, tracks);

            // Set the combined distance on the bypass edge
            if let Some(distance) = combined_distance {
                if let Some(edge_weight) = self.graph.edge_weight_mut(new_edge) {
                    edge_weight.distance = Some(distance);
                }
            }

            bypass_mapping.insert((edge1, edge2), new_edge.index());
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

    fn find_adjacent_stations_for_passing_loop(&self, passing_loop_idx: NodeIndex) -> Option<(NodeIndex, NodeIndex)> {
        use petgraph::Direction;
        use std::collections::HashSet;

        // Helper to traverse in one direction until finding a non-passing-loop station
        fn find_adjacent_in_direction(
            graph: &RailwayGraph,
            start_idx: NodeIndex,
            direction: Direction,
            visited: &mut HashSet<NodeIndex>,
        ) -> Option<NodeIndex> {
            let mut current = start_idx;

            loop {
                visited.insert(current);

                // Get neighbors in the specified direction
                let neighbors: Vec<NodeIndex> = match direction {
                    Direction::Outgoing => graph.graph.edges(current).map(|e| e.target()).collect(),
                    Direction::Incoming => graph.graph.edges_directed(current, Direction::Incoming).map(|e| e.source()).collect(),
                };

                // Find the first unvisited neighbor
                let next = neighbors.into_iter().find(|&n| !visited.contains(&n))?;

                // Check if this neighbor is a non-passing-loop station
                let node = graph.graph.node_weight(next)?;

                // Check if it's a station
                let Some(station) = node.as_station() else {
                    // It's a junction - can't use it for interpolation
                    return None;
                };

                // Check if it's not a passing loop
                if !station.passing_loop {
                    return Some(next);
                }

                current = next;
            }
        }

        let mut visited = HashSet::new();

        // Find previous non-passing-loop station (incoming direction)
        let prev = find_adjacent_in_direction(self, passing_loop_idx, Direction::Incoming, &mut visited);

        // Reset visited for forward search
        visited.clear();

        // Find next non-passing-loop station (outgoing direction)
        let next = find_adjacent_in_direction(self, passing_loop_idx, Direction::Outgoing, &mut visited);

        // Return both if found
        match (prev, next) {
            (Some(p), Some(n)) => Some((p, n)),
            _ => None,
        }
    }

    fn calculate_passing_loop_position(&self, passing_loop_idx: NodeIndex) -> Option<(f64, f64)> {
        use petgraph::Direction;

        // Find adjacent non-passing-loop stations
        let (prev_idx, next_idx) = self.find_adjacent_stations_for_passing_loop(passing_loop_idx)?;

        // Get their positions (using stored positions to avoid infinite recursion)
        let prev_pos = self.graph.node_weight(prev_idx).and_then(Node::position)?;
        let next_pos = self.graph.node_weight(next_idx).and_then(Node::position)?;

        // Count how many passing loops are in sequence between prev and next
        // and determine this passing loop's index in that sequence
        let mut passing_loops_in_sequence = Vec::new();
        let mut current = prev_idx;
        let mut found_self = false;

        // Traverse from prev to next, collecting all passing loops
        loop {
            // Get next node in the direction of next_idx
            let neighbors: Vec<NodeIndex> = self.graph.edges(current)
                .map(|e| e.target())
                .chain(self.graph.edges_directed(current, Direction::Incoming).map(|e| e.source()))
                .collect();

            // Find the neighbor that's on the path to next_idx
            let next_node = neighbors.into_iter().find(|&n| {
                n != current && (n == next_idx || {
                    // Check if this node is between current and next_idx
                    let is_passing = self.graph.node_weight(n)
                        .and_then(|node| node.as_station())
                        .is_some_and(|s| s.passing_loop);
                    is_passing && !passing_loops_in_sequence.contains(&n)
                })
            })?;

            if next_node == next_idx {
                break;
            }

            // Check if it's a passing loop
            let Some(node) = self.graph.node_weight(next_node) else {
                current = next_node;
                continue;
            };

            let Some(station) = node.as_station() else {
                current = next_node;
                continue;
            };

            if station.passing_loop {
                passing_loops_in_sequence.push(next_node);
                if next_node == passing_loop_idx {
                    found_self = true;
                }
            }

            current = next_node;
        }

        if !found_self {
            // Fallback: just use midpoint if we couldn't determine position in sequence
            return Some((
                (prev_pos.0 + next_pos.0) / 2.0,
                (prev_pos.1 + next_pos.1) / 2.0,
            ));
        }

        // Find this passing loop's index in the sequence
        let position_index = passing_loops_in_sequence.iter().position(|&idx| idx == passing_loop_idx)?;
        let total_count = passing_loops_in_sequence.len();

        // Distribute evenly: position at (index + 1) / (total_count + 1)
        #[allow(clippy::cast_precision_loss)]
        let fraction = (position_index + 1) as f64 / (total_count + 1) as f64;

        Some((
            prev_pos.0 + (next_pos.0 - prev_pos.0) * fraction,
            prev_pos.1 + (next_pos.1 - prev_pos.1) * fraction,
        ))
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
