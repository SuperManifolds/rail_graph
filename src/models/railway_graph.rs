use chrono::Duration;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StationNode {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineSegment {
    pub line_id: String,
    #[serde(with = "duration_serde")]
    pub travel_time: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailwayGraph {
    #[serde(with = "graph_serde")]
    pub graph: DiGraph<StationNode, LineSegment>,
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

    /// Add an edge representing a line segment between two stations
    pub fn add_segment(&mut self, from: NodeIndex, to: NodeIndex, line_id: String, travel_time: Duration) {
        self.graph.add_edge(from, to, LineSegment { line_id, travel_time });
    }

    /// Get station name by NodeIndex
    pub fn get_station_name(&self, index: NodeIndex) -> Option<&str> {
        self.graph.node_weight(index).map(|node| node.name.as_str())
    }

    /// Get NodeIndex by station name
    pub fn get_station_index(&self, name: &str) -> Option<NodeIndex> {
        self.station_name_to_index.get(name).copied()
    }

    /// Get all edges for a specific line, in order
    pub fn get_line_path(&self, line_id: &str) -> Vec<(NodeIndex, NodeIndex, Duration)> {
        let mut edges: Vec<_> = self.graph
            .edge_references()
            .filter(|e| e.weight().line_id == line_id)
            .map(|e| (e.source(), e.target(), e.weight().travel_time))
            .collect();

        // Sort edges to form a connected path
        // Start with an edge and build the path
        if edges.is_empty() {
            return Vec::new();
        }

        let mut path = vec![edges.remove(0)];

        // Keep adding edges that connect to the end of the current path
        while !edges.is_empty() {
            let last_target = path.last().unwrap().1;

            if let Some(pos) = edges.iter().position(|(src, _, _)| *src == last_target) {
                path.push(edges.remove(pos));
            } else {
                // No more connected edges, path is complete
                break;
            }
        }

        path
    }

    /// Get ordered list of stations for a line
    pub fn get_line_stations(&self, line_id: &str) -> Vec<(NodeIndex, String)> {
        let path = self.get_line_path(line_id);
        if path.is_empty() {
            return Vec::new();
        }

        let mut stations = Vec::new();

        // Add first station
        if let Some(name) = self.get_station_name(path[0].0) {
            stations.push((path[0].0, name.to_string()));
        }

        // Add all subsequent stations
        for (_, to, _) in &path {
            if let Some(name) = self.get_station_name(*to) {
                stations.push((*to, name.to_string()));
            }
        }

        stations
    }

    /// Get all unique line IDs in the graph (in order they appear in edges)
    pub fn get_line_ids(&self) -> Vec<String> {
        let mut line_ids = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for edge in self.graph.edge_references() {
            let line_id = edge.weight().line_id.clone();
            if seen.insert(line_id.clone()) {
                line_ids.push(line_id);
            }
        }

        line_ids
    }

    /// Get all stations in order by traversing the graph
    pub fn get_all_stations_ordered(&self) -> Vec<StationNode> {
        let mut ordered = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Start with the first line and follow its path
        if let Some(first_line) = self.get_line_ids().first() {
            let line_stations = self.get_line_stations(first_line);

            for (idx, _name) in &line_stations {
                if seen.insert(*idx) {
                    if let Some(node) = self.graph.node_weight(*idx) {
                        ordered.push(node.clone());
                    }
                }
            }

            // For remaining unseen stations, try to place them by finding where they connect
            // to already-ordered stations
            let mut added_new = true;
            while added_new {
                added_new = false;

                // Check all edges to find stations that connect to what we've already ordered
                for edge in self.graph.edge_references() {
                    let src = edge.source();
                    let tgt = edge.target();

                    // If source is in ordered but target isn't, add target after source
                    if seen.contains(&src) && !seen.contains(&tgt) {
                        if let Some(src_pos) = ordered.iter().position(|n| {
                            self.station_name_to_index.get(&n.name) == Some(&src)
                        }) {
                            seen.insert(tgt);
                            if let Some(node) = self.graph.node_weight(tgt) {
                                // Insert after the source station
                                ordered.insert(src_pos + 1, node.clone());
                                added_new = true;
                            }
                        }
                    }
                }
            }

            return ordered;
        }

        // Fallback: return all stations in arbitrary order
        self.station_name_to_index
            .values()
            .filter_map(|&idx| self.graph.node_weight(idx).cloned())
            .collect()
    }
}

impl Default for RailwayGraph {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization helpers
mod duration_serde {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(duration.num_seconds())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = i64::deserialize(deserializer)?;
        Ok(Duration::seconds(seconds))
    }
}

mod graph_serde {
    use super::{LineSegment, StationNode};
    use petgraph::graph::DiGraph;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(graph: &DiGraph<StationNode, LineSegment>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Pet graph's built-in serialization
        graph.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DiGraph<StationNode, LineSegment>, D::Error>
    where
        D: Deserializer<'de>,
    {
        DiGraph::deserialize(deserializer)
    }
}
