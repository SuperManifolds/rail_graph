use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::station::{StationNode, default_platforms};
use super::track::{Track, TrackSegment, TrackDirection};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailwayGraph {
    #[serde(with = "graph_serde")]
    pub graph: DiGraph<StationNode, TrackSegment>,
    pub station_name_to_index: HashMap<String, NodeIndex>,
    #[serde(default)]
    pub branch_angles: HashMap<(usize, usize), f64>,
}

impl RailwayGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            station_name_to_index: HashMap::new(),
            branch_angles: HashMap::new(),
        }
    }

    /// Add a station node if it doesn't exist, return its NodeIndex
    pub fn add_or_get_station(&mut self, name: String) -> NodeIndex {
        if let Some(&index) = self.station_name_to_index.get(&name) {
            index
        } else {
            let index = self.graph.add_node(StationNode {
                name: name.clone(),
                position: None,
                passing_loop: false,
                platforms: default_platforms(),
            });
            self.station_name_to_index.insert(name, index);
            index
        }
    }

    /// Update station position
    pub fn set_station_position(&mut self, index: NodeIndex, position: (f64, f64)) {
        if let Some(node) = self.graph.node_weight_mut(index) {
            node.position = Some(position);
        }
    }

    /// Get station position
    pub fn get_station_position(&self, index: NodeIndex) -> Option<(f64, f64)> {
        self.graph.node_weight(index).and_then(|node| node.position)
    }

    /// Add a track segment between two stations, returns the EdgeIndex
    pub fn add_track(&mut self, from: NodeIndex, to: NodeIndex, tracks: Vec<Track>) -> petgraph::graph::EdgeIndex {
        self.graph.add_edge(from, to, TrackSegment { tracks, distance: None })
    }

    /// Get track segment by edge index
    pub fn get_track(&self, edge_idx: petgraph::graph::EdgeIndex) -> Option<&TrackSegment> {
        self.graph.edge_weight(edge_idx)
    }

    /// Get endpoints of a track segment
    pub fn get_track_endpoints(&self, edge_idx: petgraph::graph::EdgeIndex) -> Option<(NodeIndex, NodeIndex)> {
        self.graph.edge_endpoints(edge_idx)
    }

    /// Get station name by NodeIndex
    pub fn get_station_name(&self, index: NodeIndex) -> Option<&str> {
        self.graph.node_weight(index).map(|node| node.name.as_str())
    }

    /// Get NodeIndex by station name
    pub fn get_station_index(&self, name: &str) -> Option<NodeIndex> {
        self.station_name_to_index.get(name).copied()
    }

    /// Get all edge indices connected to a station
    pub fn get_station_edges(&self, index: NodeIndex) -> Vec<usize> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

        self.graph.edges(index)
            .map(|e| e.id().index())
            .chain(self.graph.edges_directed(index, Direction::Incoming).map(|e| e.id().index()))
            .collect()
    }

    /// Find stations connected through a given station
    /// Returns a Vec of (station_before, station_after, tracks) tuples
    pub fn find_connections_through_station(&self, station_idx: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Vec<Track>)> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

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

    /// Delete a station and reconnect around it
    /// Returns (removed_edges, bypass_mapping) where bypass_mapping maps (old_edge1, old_edge2) -> new_edge
    pub fn delete_station(&mut self, index: NodeIndex) -> (Vec<usize>, std::collections::HashMap<(usize, usize), usize>) {
        // Find connections through this station to create bypass edges
        let connections = self.find_connections_through_station(index);

        // Create bypass edges and track the mapping
        let mut bypass_mapping = std::collections::HashMap::new();

        for (from_station, to_station, tracks) in connections {
            // Find the incoming and outgoing edges for this connection
            use petgraph::visit::EdgeRef;
            use petgraph::Direction;

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
        if let Some(station) = self.graph.node_weight(index) {
            self.station_name_to_index.remove(&station.name);
        }

        // Remove the station node (this also removes all connected edges)
        self.graph.remove_node(index);

        (removed_edges, bypass_mapping)
    }

    /// Get all stations in order by traversing the graph
    /// Performs a breadth-first traversal starting from the first station
    pub fn get_all_stations_ordered(&self) -> Vec<StationNode> {
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
            if seen.insert(node_idx) {
                if let Some(node) = self.graph.node_weight(node_idx) {
                    ordered.push(node.clone());
                }
            }
        }

        ordered
    }

    /// Toggle between single and double track for edges between two stations
    /// Returns a Vec of (edge_index, new_track_count) for all modified edges
    pub fn toggle_segment_double_track(&mut self, station1_name: &str, station2_name: &str) -> Vec<(usize, usize)> {
        let mut changed_edges = Vec::new();

        // Get node indices for both stations
        let Some(node1) = self.get_station_index(station1_name) else {
            return changed_edges;
        };
        let Some(node2) = self.get_station_index(station2_name) else {
            return changed_edges;
        };

        // Find and toggle edges in both directions
        for edge in self.graph.edge_indices() {
            let Some((from, to)) = self.graph.edge_endpoints(edge) else {
                continue;
            };
            if (from != node1 || to != node2) && (from != node2 || to != node1) {
                continue;
            }
            let Some(weight) = self.graph.edge_weight_mut(edge) else {
                continue;
            };
            // Toggle between single and double track
            let new_weight = if weight.tracks.len() == 1 {
                TrackSegment::new_double_track()
            } else {
                TrackSegment::new_single_track()
            };
            let new_track_count = new_weight.tracks.len();
            *weight = new_weight;
            changed_edges.push((edge.index(), new_track_count));
        }

        changed_edges
    }

    /// Extract ordered list of stations from a route based on direction
    /// Returns Vec of (station_name, NodeIndex) in the order they're visited
    pub fn get_stations_from_route(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)> {
        let mut stations = Vec::new();

        match direction {
            crate::models::RouteDirection::Forward => {
                // Forward: extract from -> to for each edge
                if let Some(segment) = route.first() {
                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                    if let Some((from, _)) = self.get_track_endpoints(edge_idx) {
                        if let Some(name) = self.get_station_name(from) {
                            stations.push((name.to_string(), from));
                        }
                    }
                }

                for segment in route {
                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                    if let Some((_, to)) = self.get_track_endpoints(edge_idx) {
                        if let Some(name) = self.get_station_name(to) {
                            stations.push((name.to_string(), to));
                        }
                    }
                }
            }
            crate::models::RouteDirection::Return => {
                // Return: extract to -> from for each edge (traveling backwards)
                if let Some(segment) = route.first() {
                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                    if let Some((_, to)) = self.get_track_endpoints(edge_idx) {
                        if let Some(name) = self.get_station_name(to) {
                            stations.push((name.to_string(), to));
                        }
                    }
                }

                for segment in route {
                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                    if let Some((from, _)) = self.get_track_endpoints(edge_idx) {
                        if let Some(name) = self.get_station_name(from) {
                            stations.push((name.to_string(), from));
                        }
                    }
                }
            }
        }

        stations
    }

    /// Get the first and last station indices for a route based on direction
    /// Returns (Option<first_station>, Option<last_station>)
    pub fn get_route_endpoints(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> (Option<NodeIndex>, Option<NodeIndex>) {
        match direction {
            crate::models::RouteDirection::Forward => {
                let first = route.first()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(from, _)| from)
                    });
                let last = route.last()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(_, to)| to)
                    });
                (first, last)
            }
            crate::models::RouteDirection::Return => {
                // Return route segments travel backwards on edges
                // First segment's 'to' is the starting station
                let first = route.first()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(_, to)| to)
                    });
                // Last segment's 'from' is the ending station
                let last = route.last()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(from, _)| from)
                    });
                (first, last)
            }
        }
    }

    /// Get available stations that can be added at the start of a route
    /// Returns station names that have edges connecting to the first station
    pub fn get_available_start_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String> {
        let (first_idx, _) = self.get_route_endpoints(route, direction);

        let Some(first_idx) = first_idx else {
            return Vec::new();
        };

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|station| {
                let station_idx = self.get_station_index(&station.name)?;
                // For forward: find edge from station_idx to first_idx
                // For return: find edge from first_idx to station_idx (traveling backwards)
                let has_edge = match direction {
                    crate::models::RouteDirection::Forward => self.graph.find_edge(station_idx, first_idx).is_some(),
                    crate::models::RouteDirection::Return => self.graph.find_edge(first_idx, station_idx).is_some(),
                };
                if has_edge {
                    Some(station.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get available stations that can be added at the end of a route
    /// Returns station names that have edges connecting from the last station
    pub fn get_available_end_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String> {
        let (_, last_idx) = self.get_route_endpoints(route, direction);

        let Some(last_idx) = last_idx else {
            return Vec::new();
        };

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|station| {
                let station_idx = self.get_station_index(&station.name)?;
                // For forward: find edge from last_idx to station_idx
                // For return: find edge from station_idx to last_idx (traveling backwards)
                let has_edge = match direction {
                    crate::models::RouteDirection::Forward => self.graph.find_edge(last_idx, station_idx).is_some(),
                    crate::models::RouteDirection::Return => self.graph.find_edge(station_idx, last_idx).is_some(),
                };
                if has_edge {
                    Some(station.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl Default for RailwayGraph {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization helpers
mod graph_serde {
    use super::{TrackSegment, StationNode};
    use petgraph::graph::DiGraph;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(graph: &DiGraph<StationNode, TrackSegment>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Petgraph's built-in serialization
        graph.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DiGraph<StationNode, TrackSegment>, D::Error>
    where
        D: Deserializer<'de>,
    {
        DiGraph::deserialize(deserializer)
    }
}
