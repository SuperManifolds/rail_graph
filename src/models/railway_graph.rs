use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Platform {
    pub name: String,
}

fn default_platforms() -> Vec<Platform> {
    vec![
        Platform { name: "1".to_string() },
        Platform { name: "2".to_string() },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationNode {
    pub name: String,
    #[serde(default)]
    pub position: Option<(f64, f64)>,
    #[serde(default)]
    pub passing_loop: bool,
    #[serde(default = "default_platforms")]
    pub platforms: Vec<Platform>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum TrackDirection {
    Bidirectional,
    Forward,    // From source to target only
    Backward,   // From target to source only
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub direction: TrackDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSegment {
    pub tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
}

impl TrackSegment {
    pub fn new_single_track() -> Self {
        Self {
            tracks: vec![Track { direction: TrackDirection::Bidirectional }],
            distance: None,
        }
    }

    pub fn new_double_track() -> Self {
        Self {
            tracks: vec![
                Track { direction: TrackDirection::Forward },
                Track { direction: TrackDirection::Backward },
            ],
            distance: None,
        }
    }
}

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
