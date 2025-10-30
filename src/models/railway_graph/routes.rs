use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use super::RailwayGraph;

/// Extension trait for route-related operations on `RailwayGraph`
pub trait Routes {
    /// Extract ordered list of stations from a route based on direction
    /// Returns Vec of (`station_name`, `NodeIndex`) in the order they're visited
    fn get_stations_from_route(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)>;

    /// Get the first and last station indices for a route based on direction
    /// Returns (Option<`first_station`>, Option<`last_station`>)
    fn get_route_endpoints(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> (Option<NodeIndex>, Option<NodeIndex>);

    /// Get available stations that can be added at the start of a route
    /// Returns (`station_name`, `NodeIndex`) pairs for stations that have edges connecting to the first station
    fn get_available_start_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)>;

    /// Get available stations that can be added at the end of a route
    /// Returns (`station_name`, `NodeIndex`) pairs for stations that have edges connecting from the last station
    fn get_available_end_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)>;

    /// Find a path between two nodes, potentially going through junctions
    /// Returns a list of edge indices that form the path, or None if no path exists
    fn find_path_between_nodes(
        &self,
        from: NodeIndex,
        to: NodeIndex,
    ) -> Option<Vec<EdgeIndex>>;

    /// Find a path through multiple waypoints
    /// Returns a list of edge indices that form the complete path, or None if any segment has no path
    fn find_multi_point_path(
        &self,
        waypoints: &[NodeIndex],
    ) -> Option<Vec<EdgeIndex>>;
}

impl Routes for RailwayGraph {
    #[allow(clippy::excessive_nesting)]
    fn get_stations_from_route(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)> {
        use super::tracks::Tracks;

        let mut stations = Vec::new();

        match direction {
            crate::models::RouteDirection::Forward => {
                let mut current_node: Option<NodeIndex> = None;

                for (idx, segment) in route.iter().enumerate() {
                    let edge_idx = EdgeIndex::new(segment.edge_index);
                    let Some((edge_from, edge_to)) = self.get_track_endpoints(edge_idx) else {
                        continue;
                    };

                    // Determine which direction we're traveling on this edge
                    let (travel_from, travel_to) = self.determine_travel_direction(
                        edge_from,
                        edge_to,
                        current_node,
                        route.get(idx + 1),
                    );

                    // Skip if discontinuous
                    if current_node.is_some_and(|prev| prev != travel_from) {
                        continue;
                    }

                    // Add the origin node if this is the first segment
                    if idx == 0 {
                        if let Some(node) = self.graph.node_weight(travel_from) {
                            stations.push((node.display_name().to_string(), travel_from));
                        }
                    }

                    // Add the destination node (allow duplicates)
                    if let Some(node) = self.graph.node_weight(travel_to) {
                        stations.push((node.display_name().to_string(), travel_to));
                    }
                    current_node = Some(travel_to);
                }
            }
            crate::models::RouteDirection::Return => {
                let mut current_node: Option<NodeIndex> = None;

                for (idx, segment) in route.iter().enumerate() {
                    let edge_idx = EdgeIndex::new(segment.edge_index);
                    let Some((edge_from, edge_to)) = self.get_track_endpoints(edge_idx) else {
                        continue;
                    };

                    // Determine which direction we're traveling on this edge
                    let (travel_from, travel_to) = self.determine_travel_direction(
                        edge_from,
                        edge_to,
                        current_node,
                        route.get(idx + 1),
                    );

                    // Skip if discontinuous
                    if current_node.is_some_and(|prev| prev != travel_from) {
                        continue;
                    }

                    // Add the origin node if this is the first segment
                    if idx == 0 {
                        if let Some(node) = self.graph.node_weight(travel_from) {
                            stations.push((node.display_name().to_string(), travel_from));
                        }
                    }

                    // Add the destination node (allow duplicates)
                    if let Some(node) = self.graph.node_weight(travel_to) {
                        stations.push((node.display_name().to_string(), travel_to));
                    }
                    current_node = Some(travel_to);
                }
            }
        }

        stations
    }

    fn get_route_endpoints(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> (Option<NodeIndex>, Option<NodeIndex>) {
        // Use get_stations_from_route to get all stations, then return first and last
        let stations = self.get_stations_from_route(route, direction);
        let first = stations.first().map(|(_, idx)| *idx);
        let last = stations.last().map(|(_, idx)| *idx);
        (first, last)
    }

    fn get_available_start_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)> {
        use super::stations::Stations;

        let (first_idx, _) = self.get_route_endpoints(route, direction);

        let Some(first_idx) = first_idx else {
            return Vec::new();
        };

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|(node_idx, station)| {
                // For forward: find path from node_idx to first_idx
                // For return: find path from first_idx to node_idx (traveling backwards)
                let has_path = match direction {
                    crate::models::RouteDirection::Forward => self.find_path_between_nodes(*node_idx, first_idx).is_some(),
                    crate::models::RouteDirection::Return => self.find_path_between_nodes(first_idx, *node_idx).is_some(),
                };
                if has_path {
                    Some((station.name.clone(), *node_idx))
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_available_end_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)> {
        use super::stations::Stations;

        let (_, last_idx) = self.get_route_endpoints(route, direction);

        let Some(last_idx) = last_idx else {
            return Vec::new();
        };

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|(node_idx, station)| {
                // For forward: find path from last_idx to node_idx
                // For return: find path from node_idx to last_idx (traveling backwards)
                let has_path = match direction {
                    crate::models::RouteDirection::Forward => self.find_path_between_nodes(last_idx, *node_idx).is_some(),
                    crate::models::RouteDirection::Return => self.find_path_between_nodes(*node_idx, last_idx).is_some(),
                };
                if has_path {
                    Some((station.name.clone(), *node_idx))
                } else {
                    None
                }
            })
            .collect()
    }

    #[allow(clippy::excessive_nesting)]
    fn find_path_between_nodes(
        &self,
        from: NodeIndex,
        to: NodeIndex,
    ) -> Option<Vec<EdgeIndex>> {
        use std::collections::{VecDeque, HashMap, HashSet};
        use petgraph::visit::EdgeRef;
        use crate::models::track::TrackDirection;

        // State = (current node, edge used to arrive at that node)
        // This allows visiting the same junction multiple times via different incoming edges
        type State = (NodeIndex, Option<EdgeIndex>);

        // Don't consider a path from a node to itself
        if from == to {
            return None;
        }

        // BFS to find shortest path, respecting track directions and junction routing rules
        let mut queue = VecDeque::new();
        let mut visited = HashSet::new();
        let mut came_from: HashMap<State, (State, EdgeIndex)> = HashMap::new();

        let start_state = (from, None);
        queue.push_back(start_state);
        visited.insert(start_state);

        while let Some((current, incoming_edge)) = queue.pop_front() {
            // Explore all edges connected to current node (both incoming and outgoing)
            // Check each edge's TrackDirection to see if we can use it

            // Check outgoing edges (current -> neighbor)
            for edge in self.graph.edges(current) {
                let neighbor = edge.target();
                let track_segment = edge.weight();

                // Can always use Forward or Bidirectional edges in their natural direction
                let can_use_track = track_segment.tracks.iter().any(|t|
                    matches!(t.direction, TrackDirection::Forward | TrackDirection::Bidirectional)
                );

                let can_use_junction = self.is_junction_routing_allowed(current, incoming_edge, edge.id());

                // Check if we can use this track and junction routing allows it
                if !can_use_track || !can_use_junction {
                    continue;
                }

                // Check if we've reached destination
                if neighbor == to {
                    // Found the destination! Reconstruct path
                    let mut path = Vec::new();
                    let mut state = (current, incoming_edge);
                    while let Some((prev_state, prev_edge)) = came_from.get(&state) {
                        path.push(*prev_edge);
                        state = *prev_state;
                    }
                    path.reverse();
                    path.push(edge.id());
                    return Some(path);
                }

                // Create new state for neighbor
                let neighbor_state = (neighbor, Some(edge.id()));

                // For non-destination nodes, check if state already visited
                if !visited.contains(&neighbor_state) {
                    visited.insert(neighbor_state);
                    came_from.insert(neighbor_state, ((current, incoming_edge), edge.id()));
                    queue.push_back(neighbor_state);
                }
            }

            // Check incoming edges (neighbor -> current, but we want to go current -> neighbor)
            for edge in self.graph.edges_directed(current, petgraph::Direction::Incoming) {
                let neighbor = edge.source();
                let track_segment = edge.weight();

                // Can use Backward or Bidirectional edges in reverse direction
                let can_use_track = track_segment.tracks.iter().any(|t|
                    matches!(t.direction, TrackDirection::Backward | TrackDirection::Bidirectional)
                );

                let can_use_junction = self.is_junction_routing_allowed(current, incoming_edge, edge.id());

                // Check if we can use this track and junction routing allows it
                if !can_use_track || !can_use_junction {
                    continue;
                }

                // Check if we've reached destination
                if neighbor == to {
                    // Found the destination! Reconstruct path
                    let mut path = Vec::new();
                    let mut state = (current, incoming_edge);
                    while let Some((prev_state, prev_edge)) = came_from.get(&state) {
                        path.push(*prev_edge);
                        state = *prev_state;
                    }
                    path.reverse();
                    path.push(edge.id());
                    return Some(path);
                }

                // Create new state for neighbor
                let neighbor_state = (neighbor, Some(edge.id()));

                // For non-destination nodes, check if state already visited
                if !visited.contains(&neighbor_state) {
                    visited.insert(neighbor_state);
                    came_from.insert(neighbor_state, ((current, incoming_edge), edge.id()));
                    queue.push_back(neighbor_state);
                }
            }
        }

        None
    }

    fn find_multi_point_path(
        &self,
        waypoints: &[NodeIndex],
    ) -> Option<Vec<EdgeIndex>> {
        // Need at least 2 waypoints to form a path
        if waypoints.len() < 2 {
            return None;
        }

        let mut complete_path = Vec::new();

        // For each consecutive pair of waypoints, find the path between them
        for window in waypoints.windows(2) {
            let from = window[0];
            let to = window[1];

            // Find path for this segment
            let segment_path = self.find_path_between_nodes(from, to)?;

            // Add this segment's edges to the complete path
            complete_path.extend(segment_path);
        }

        Some(complete_path)
    }
}

impl RailwayGraph {
    /// Check if routing through a junction is allowed
    /// Returns true if node is not a junction, or if routing is allowed
    fn is_junction_routing_allowed(
        &self,
        node_idx: NodeIndex,
        incoming_edge: Option<EdgeIndex>,
        outgoing_edge: EdgeIndex,
    ) -> bool {
        let Some(node) = self.graph.node_weight(node_idx) else {
            return false;
        };

        let Some(junction) = node.as_junction() else {
            // Not a junction, routing is always allowed
            return true;
        };

        let Some(inc_edge) = incoming_edge else {
            // No incoming edge (starting node), allow all routes
            return true;
        };

        junction.is_routing_allowed(inc_edge, outgoing_edge)
    }

    /// Determine travel direction for an edge based on previous node and next segment
    /// Returns (`from_node`, `to_node`) for the direction of travel
    fn determine_travel_direction(
        &self,
        edge_from: NodeIndex,
        edge_to: NodeIndex,
        prev_node: Option<NodeIndex>,
        next_segment: Option<&crate::models::RouteSegment>,
    ) -> (NodeIndex, NodeIndex) {
        use super::tracks::Tracks;

        if let Some(prev) = prev_node {
            // We know where we're coming from
            if prev == edge_from {
                (edge_from, edge_to)
            } else if prev == edge_to {
                (edge_to, edge_from)
            } else {
                // Shouldn't happen, default to forward
                (edge_from, edge_to)
            }
        } else if let Some(next_seg) = next_segment {
            // First segment - check next segment to determine direction
            let next_edge_idx = EdgeIndex::new(next_seg.edge_index);
            if let Some((next_from, next_to)) = self.get_track_endpoints(next_edge_idx) {
                if edge_to == next_from || edge_to == next_to {
                    (edge_from, edge_to)
                } else if edge_from == next_from || edge_from == next_to {
                    (edge_to, edge_from)
                } else {
                    (edge_from, edge_to)
                }
            } else {
                (edge_from, edge_to)
            }
        } else {
            // Single segment, default to forward
            (edge_from, edge_to)
        }
    }

    /// Get the name of a node (station or junction)
    #[must_use]
    pub fn get_node_name(&self, node_idx: NodeIndex) -> Option<String> {
        use super::stations::Stations;
        use crate::models::Node;

        if let Some(station_name) = self.get_station_name(node_idx) {
            Some(station_name.to_string())
        } else if let Some(node) = self.graph.node_weight(node_idx) {
            match node {
                Node::Junction(junction) => Some(junction.name.clone().unwrap_or_else(|| "Junction".to_string())),
                Node::Station(_) => None, // Already handled above
            }
        } else {
            None
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RailwayGraph, Stations, Tracks, RouteDirection, RouteSegment};
    use crate::models::track::{Track, TrackDirection};
    use chrono::Duration;

    fn create_test_route_segment(edge_index: usize) -> RouteSegment {
        RouteSegment {
            edge_index,
            track_index: 0,
            origin_platform: 0,
            destination_platform: 0,
            duration: Some(Duration::minutes(5)),
            wait_time: Duration::seconds(30),
        }
    }

    #[test]
    fn test_get_route_endpoints_forward() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            create_test_route_segment(edge1.index()),
            create_test_route_segment(edge2.index()),
        ];

        let (first, last) = graph.get_route_endpoints(&route, RouteDirection::Forward);
        assert_eq!(first, Some(idx1));
        assert_eq!(last, Some(idx3));
    }

    #[test]
    fn test_get_route_endpoints_return() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            create_test_route_segment(edge2.index()),
            create_test_route_segment(edge1.index()),
        ];

        let (first, last) = graph.get_route_endpoints(&route, RouteDirection::Return);
        assert_eq!(first, Some(idx3));
        assert_eq!(last, Some(idx1));
    }

    #[test]
    fn test_get_route_endpoints_empty() {
        let graph = RailwayGraph::new();
        let route = vec![];

        let (first, last) = graph.get_route_endpoints(&route, RouteDirection::Forward);
        assert_eq!(first, None);
        assert_eq!(last, None);
    }

    #[test]
    fn test_get_stations_from_route_forward() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            create_test_route_segment(edge1.index()),
            create_test_route_segment(edge2.index()),
        ];

        let stations = graph.get_stations_from_route(&route, RouteDirection::Forward);
        assert_eq!(stations.len(), 3);
        assert_eq!(stations[0].0, "Station A");
        assert_eq!(stations[1].0, "Station B");
        assert_eq!(stations[2].0, "Station C");
    }

    #[test]
    fn test_get_stations_from_route_return() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Return route: C -> B -> A
        let route = vec![
            create_test_route_segment(edge2.index()),
            create_test_route_segment(edge1.index()),
        ];

        let stations = graph.get_stations_from_route(&route, RouteDirection::Return);
        assert_eq!(stations.len(), 3);
        assert_eq!(stations[0].0, "Station C");
        assert_eq!(stations[1].0, "Station B");
        assert_eq!(stations[2].0, "Station A");
    }

    #[test]
    fn test_get_available_start_stations() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        // Create: A -> B -> C
        graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Route currently starts at B
        let route = vec![create_test_route_segment(edge2.index())];

        let available = graph.get_available_start_stations(&route, RouteDirection::Forward);
        assert_eq!(available.len(), 2);
        assert!(available.iter().any(|(name, _)| name == "Station A"));
        assert!(available.iter().any(|(name, _)| name == "Station C"));
    }

    #[test]
    fn test_get_available_end_stations() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        // Create: A -> B -> C
        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Route currently ends at B
        let route = vec![create_test_route_segment(edge1.index())];

        let available = graph.get_available_end_stations(&route, RouteDirection::Forward);
        assert_eq!(available.len(), 2);
        assert!(available.iter().any(|(name, _)| name == "Station A"));
        assert!(available.iter().any(|(name, _)| name == "Station C"));
    }

    #[test]
    fn test_get_available_stations_empty_route() {
        let mut graph = RailwayGraph::new();
        graph.add_or_get_station("Station A".to_string());

        let route = vec![];

        let start_stations = graph.get_available_start_stations(&route, RouteDirection::Forward);
        assert_eq!(start_stations.len(), 0);

        let end_stations = graph.get_available_end_stations(&route, RouteDirection::Forward);
        assert_eq!(end_stations.len(), 0);
    }

    #[test]
    fn test_find_path_between_nodes_direct() {
        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("A".to_string());
        let b = graph.add_or_get_station("B".to_string());

        let e1 = graph.add_track(a, b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Direct path exists
        let path = graph.find_path_between_nodes(a, b);
        assert!(path.is_some());
        if let Some(path) = path {
            assert_eq!(path.len(), 1);
            assert_eq!(path[0].index(), e1.index());
        }
    }

    #[test]
    fn test_find_path_between_nodes_indirect() {
        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("A".to_string());
        let b = graph.add_or_get_station("B".to_string());
        let c = graph.add_or_get_station("C".to_string());

        // Create A -> B -> C
        let e1 = graph.add_track(a, b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(b, c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Find path from A to C (should go through B)
        let path = graph.find_path_between_nodes(a, c);
        assert!(path.is_some());
        if let Some(path) = path {
            assert_eq!(path.len(), 2);
            assert_eq!(path[0].index(), e1.index());
            assert_eq!(path[1].index(), e2.index());
        }
    }

    #[test]
    fn test_find_path_between_nodes_no_path() {
        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("A".to_string());
        let b = graph.add_or_get_station("B".to_string());
        let c = graph.add_or_get_station("C".to_string());

        // Create A -> B, but C is disconnected
        graph.add_track(a, b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // No path from A to C
        let path = graph.find_path_between_nodes(a, c);
        assert!(path.is_none());
    }

    #[test]
    fn test_find_path_between_nodes_through_junction() {
        use crate::models::{Junctions, Junction};

        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("A".to_string());
        let b = graph.add_or_get_station("B".to_string());
        let j = graph.add_junction(Junction {
            name: Some("J".to_string()),
            position: None,
            routing_rules: vec![],
        });

        // Create A -> J -> B
        let e1 = graph.add_track(a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Find path from A to B through junction
        let path = graph.find_path_between_nodes(a, b);
        assert!(path.is_some());
        if let Some(path) = path {
            assert_eq!(path.len(), 2);
            assert_eq!(path[0].index(), e1.index());
            assert_eq!(path[1].index(), e2.index());
        }
    }

    #[test]
    fn test_get_stations_from_route_with_junction() {
        use crate::models::{Junctions, Junction};

        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("Station A".to_string());
        let b = graph.add_or_get_station("Station B".to_string());
        let j = graph.add_junction(Junction {
            name: Some("Junction J".to_string()),
            position: None,
            routing_rules: vec![],
        });

        // Create A -> J -> B
        let e1 = graph.add_track(a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, b, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            create_test_route_segment(e1.index()),
            create_test_route_segment(e2.index()),
        ];

        let nodes = graph.get_stations_from_route(&route, RouteDirection::Forward);

        // Should have all 3 nodes: A, J, and B
        assert_eq!(nodes.len(), 3, "Expected 3 nodes (A, J, B), got {}", nodes.len());
        assert_eq!(nodes[0].0, "Station A");
        assert_eq!(nodes[1].0, "Junction J");
        assert_eq!(nodes[2].0, "Station B");
    }

    #[test]
    fn test_pathfinding_creates_complete_route() {
        use crate::models::{Junctions, Junction, RouteSegment};

        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("Station A".to_string());
        let b = graph.add_or_get_station("Station B".to_string());
        let j = graph.add_junction(Junction {
            name: Some("Junction J".to_string()),
            position: None,
            routing_rules: vec![],
        });

        // Create A -> J -> B
        graph.add_track(a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(j, b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Use pathfinding like the UI does
        let path = graph.find_path_between_nodes(a, b);
        assert!(path.is_some());

        // Create route segments from path
        let mut route: Vec<RouteSegment> = Vec::new();
        if let Some(path) = path {
            for edge in &path {
                route.push(RouteSegment {
                    edge_index: edge.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(5)),
                    wait_time: Duration::seconds(30),
                });
            }
        }

        // Verify route has 2 segments (A->J, J->B)
        assert_eq!(route.len(), 2, "Route should have 2 segments");

        // Verify get_stations_from_route returns all 3 nodes
        let nodes = graph.get_stations_from_route(&route, RouteDirection::Forward);
        assert_eq!(nodes.len(), 3, "Expected 3 nodes (A, J, B), got {}", nodes.len());
        assert_eq!(nodes[0].0, "Station A");
        assert_eq!(nodes[1].0, "Junction J");
        assert_eq!(nodes[2].0, "Station B");
    }

    #[test]
    fn test_find_path_respects_disabled_junction_routing() {
        use crate::models::{Junctions, Junction, RoutingRule};

        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("Station A".to_string());
        let b = graph.add_or_get_station("Station B".to_string());
        let c = graph.add_or_get_station("Station C".to_string());

        let j = graph.add_junction(Junction {
            name: Some("Junction J".to_string()),
            position: None,
            routing_rules: vec![],
        });

        // Create A -> J -> B and A -> J -> C
        let e1 = graph.add_track(a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e3 = graph.add_track(j, c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // First verify path from A to B exists with default routing (all allowed)
        let path = graph.find_path_between_nodes(a, b);
        assert!(path.is_some(), "Path from A to B should exist initially");

        // Now disable routing from e1 to e2 and e3 to e2 at junction J
        if let Some(junction) = graph.get_junction_mut(j) {
            junction.routing_rules.push(RoutingRule {
                from_edge: e1,
                to_edge: e2,
                allowed: false,
            });
            junction.routing_rules.push(RoutingRule {
                from_edge: e3,
                to_edge: e2,
                allowed: false,
            });
        }

        // Now path from A to B should not exist (blocked at junction)
        // Even with state-based tracking allowing A->J->C->J, the J->B route is blocked from e3 too
        let path = graph.find_path_between_nodes(a, b);
        assert!(path.is_none(), "Path from A to B should be blocked by junction routing rule");

        // But path from A to C should still exist
        let path = graph.find_path_between_nodes(a, c);
        assert!(path.is_some(), "Path from A to C should still exist");
        if let Some(path) = path {
            assert_eq!(path.len(), 2);
            assert_eq!(path[0], e1);
            assert_eq!(path[1], e3);
        }
    }

    #[test]
    fn test_find_path_respects_junction_routing_with_alternate_route() {
        use crate::models::{Junctions, Junction, RoutingRule};

        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("Station A".to_string());
        let b = graph.add_or_get_station("Station B".to_string());
        let c = graph.add_or_get_station("Station C".to_string());

        let j = graph.add_junction(Junction {
            name: Some("Junction J".to_string()),
            position: None,
            routing_rules: vec![],
        });

        // Create network: A -> J -> B and also A -> C -> B (alternate route)
        let e1 = graph.add_track(a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e3 = graph.add_track(a, c, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e4 = graph.add_track(c, b, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Disable routing from e1 to e2 at junction J
        if let Some(junction) = graph.get_junction_mut(j) {
            junction.routing_rules.push(RoutingRule {
                from_edge: e1,
                to_edge: e2,
                allowed: false,
            });
        }

        // Path from A to B should use alternate route through C
        let path = graph.find_path_between_nodes(a, b);
        assert!(path.is_some(), "Alternate path from A to B should exist");
        if let Some(path) = path {
            // Should use A -> C -> B route
            assert_eq!(path.len(), 2);
            assert_eq!(path[0], e3);
            assert_eq!(path[1], e4);
        }
    }

    #[test]
    fn test_find_path_through_junction_twice() {
        use crate::models::{Junctions, Junction, RoutingRule};

        let mut graph = RailwayGraph::new();
        let a = graph.add_or_get_station("Station A".to_string());
        let b = graph.add_or_get_station("Station B".to_string());
        let c = graph.add_or_get_station("Station C".to_string());

        let j = graph.add_junction(Junction {
            name: Some("Junction J".to_string()),
            position: None,
            routing_rules: vec![],
        });

        // Create network: A -> J -> B and J -> C
        //     A
        //     |
        //     J (junction)
        //    / \
        //   B   C
        let e1 = graph.add_track(a, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(j, b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e3 = graph.add_track(j, c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Block direct route from A to C (e1 -> e3)
        if let Some(junction) = graph.get_junction_mut(j) {
            junction.routing_rules.push(RoutingRule {
                from_edge: e1,
                to_edge: e3,
                allowed: false,
            });
        }

        // Path from A to C should go: A -> J -> B -> J -> C
        // This requires visiting junction J twice via different incoming edges
        let path = graph.find_path_between_nodes(a, c);
        assert!(path.is_some(), "Path from A to C should exist by visiting J twice");
        if let Some(path) = path {
            assert_eq!(path.len(), 4, "Path should have 4 edges: A->J, J->B, B->J, J->C");
            assert_eq!(path[0], e1, "First edge should be A->J");
            assert_eq!(path[1], e2, "Second edge should be J->B");
            assert_eq!(path[2], e2, "Third edge should be B->J (same as J->B, bidirectional)");
            assert_eq!(path[3], e3, "Fourth edge should be J->C");
        }
    }
}
