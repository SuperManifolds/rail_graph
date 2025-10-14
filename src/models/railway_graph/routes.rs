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
    /// Returns station names that have edges connecting to the first station
    fn get_available_start_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String>;

    /// Get available stations that can be added at the end of a route
    /// Returns station names that have edges connecting from the last station
    fn get_available_end_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String>;

    /// Find a path between two nodes, potentially going through junctions
    /// Returns a list of edge indices that form the path, or None if no path exists
    fn find_path_between_nodes(
        &self,
        from: NodeIndex,
        to: NodeIndex,
    ) -> Option<Vec<EdgeIndex>>;
}

impl Routes for RailwayGraph {
    fn get_stations_from_route(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)> {
        use super::tracks::Tracks;
        use std::collections::HashSet;

        let mut stations = Vec::new();
        let mut seen = HashSet::new();

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
                        self.add_station_if_not_seen(travel_from, &mut stations, &mut seen);
                    }

                    // Add the destination node
                    self.add_station_if_not_seen(travel_to, &mut stations, &mut seen);
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
                        self.add_station_if_not_seen(travel_from, &mut stations, &mut seen);
                    }

                    // Add the destination node
                    self.add_station_if_not_seen(travel_to, &mut stations, &mut seen);
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
    ) -> Vec<String> {
        use super::stations::Stations;
        use std::collections::HashSet;

        let (first_idx, _) = self.get_route_endpoints(route, direction);

        let Some(first_idx) = first_idx else {
            return Vec::new();
        };

        // Get all nodes already in the route
        let nodes_in_route: HashSet<NodeIndex> = self.get_stations_from_route(route, direction)
            .iter()
            .map(|(_, idx)| *idx)
            .collect();

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|(node_idx, station)| {
                // Skip if already in route
                if nodes_in_route.contains(node_idx) {
                    return None;
                }

                // For forward: find path from node_idx to first_idx
                // For return: find path from first_idx to node_idx (traveling backwards)
                let has_path = match direction {
                    crate::models::RouteDirection::Forward => self.find_path_between_nodes(*node_idx, first_idx).is_some(),
                    crate::models::RouteDirection::Return => self.find_path_between_nodes(first_idx, *node_idx).is_some(),
                };
                if has_path && *node_idx != first_idx {
                    Some(station.name.clone())
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
    ) -> Vec<String> {
        use super::stations::Stations;
        use std::collections::HashSet;

        let (_, last_idx) = self.get_route_endpoints(route, direction);

        let Some(last_idx) = last_idx else {
            return Vec::new();
        };

        // Get all nodes already in the route
        let nodes_in_route: HashSet<NodeIndex> = self.get_stations_from_route(route, direction)
            .iter()
            .map(|(_, idx)| *idx)
            .collect();

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|(node_idx, station)| {
                // Skip if already in route
                if nodes_in_route.contains(node_idx) {
                    return None;
                }

                // For forward: find path from last_idx to node_idx
                // For return: find path from node_idx to last_idx (traveling backwards)
                let has_path = match direction {
                    crate::models::RouteDirection::Forward => self.find_path_between_nodes(last_idx, *node_idx).is_some(),
                    crate::models::RouteDirection::Return => self.find_path_between_nodes(*node_idx, last_idx).is_some(),
                };
                if has_path && *node_idx != last_idx {
                    Some(station.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn find_path_between_nodes(
        &self,
        from: NodeIndex,
        to: NodeIndex,
    ) -> Option<Vec<EdgeIndex>> {
        use std::collections::{VecDeque, HashMap};
        use petgraph::visit::EdgeRef;
        use crate::models::track::TrackDirection;

        // BFS to find shortest path, respecting track directions
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();

        queue.push_back(from);
        visited.insert(from, None);

        while let Some(current) = queue.pop_front() {
            if current == to {
                // Reconstruct path
                let mut path = Vec::new();
                let mut node = to;

                while let Some(Some((prev_node, edge))) = visited.get(&node) {
                    path.push(*edge);
                    node = *prev_node;
                }

                path.reverse();
                return Some(path);
            }

            // Explore all edges connected to current node (both incoming and outgoing)
            // Check each edge's TrackDirection to see if we can use it

            // Check outgoing edges (current -> neighbor)
            for edge in self.graph.edges(current) {
                let neighbor = edge.target();
                let track_segment = edge.weight();

                // Can always use Forward or Bidirectional edges in their natural direction
                let can_use = track_segment.tracks.iter().any(|t|
                    matches!(t.direction, TrackDirection::Forward | TrackDirection::Bidirectional)
                );

                if can_use && matches!(visited.entry(neighbor), std::collections::hash_map::Entry::Vacant(_)) {
                    visited.insert(neighbor, Some((current, edge.id())));
                    queue.push_back(neighbor);
                }
            }

            // Check incoming edges (neighbor -> current, but we want to go current -> neighbor)
            for edge in self.graph.edges_directed(current, petgraph::Direction::Incoming) {
                let neighbor = edge.source();
                let track_segment = edge.weight();

                // Can use Backward or Bidirectional edges in reverse direction
                let can_use = track_segment.tracks.iter().any(|t|
                    matches!(t.direction, TrackDirection::Backward | TrackDirection::Bidirectional)
                );

                if can_use && matches!(visited.entry(neighbor), std::collections::hash_map::Entry::Vacant(_)) {
                    visited.insert(neighbor, Some((current, edge.id())));
                    queue.push_back(neighbor);
                }
            }
        }

        None
    }
}

impl RailwayGraph {
    /// Add a node to the stations list if not already seen
    fn add_station_if_not_seen(
        &self,
        node_idx: NodeIndex,
        stations: &mut Vec<(String, NodeIndex)>,
        seen: &mut std::collections::HashSet<NodeIndex>,
    ) {
        if seen.contains(&node_idx) {
            return;
        }

        let Some(name) = self.get_node_name(node_idx) else {
            return;
        };

        stations.push((name, node_idx));
        seen.insert(node_idx);
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
    fn get_node_name(&self, node_idx: NodeIndex) -> Option<String> {
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
            duration: Duration::minutes(5),
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
        assert_eq!(available.len(), 1);
        assert!(available.contains(&"Station A".to_string()));
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
        assert_eq!(available.len(), 1);
        assert!(available.contains(&"Station C".to_string()));
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
                    duration: Duration::minutes(5),
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
}
