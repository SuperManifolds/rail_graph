use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationNode {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSegment {
    pub double_tracked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailwayGraph {
    #[serde(with = "graph_serde")]
    pub graph: DiGraph<StationNode, TrackSegment>,
    pub station_name_to_index: HashMap<String, NodeIndex>,
}

impl RailwayGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            station_name_to_index: HashMap::new(),
        }
    }

    /// Add a station node if it doesn't exist, return its NodeIndex
    pub fn add_or_get_station(&mut self, name: String) -> NodeIndex {
        if let Some(&index) = self.station_name_to_index.get(&name) {
            index
        } else {
            let index = self.graph.add_node(StationNode { name: name.clone() });
            self.station_name_to_index.insert(name, index);
            index
        }
    }

    /// Add a track segment between two stations, returns the EdgeIndex
    pub fn add_track(&mut self, from: NodeIndex, to: NodeIndex, double_tracked: bool) -> petgraph::graph::EdgeIndex {
        self.graph.add_edge(from, to, TrackSegment { double_tracked })
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

    /// Get all stations in order by traversing the graph
    /// Performs a breadth-first traversal starting from the first station
    pub fn get_all_stations_ordered(&self) -> Vec<StationNode> {
        if self.graph.node_count() == 0 {
            return Vec::new();
        }

        let mut ordered = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Start from the first node in the graph
        let start_node = self.graph.node_indices().next().unwrap();

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
