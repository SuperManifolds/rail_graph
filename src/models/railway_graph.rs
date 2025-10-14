use petgraph::stable_graph::{StableGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::node::Node;
use super::track::TrackSegment;

pub mod junctions;
pub mod stations;
pub mod tracks;
pub mod routes;

// Re-export extension traits
pub use junctions::Junctions;
pub use stations::Stations;
pub use tracks::Tracks;
pub use routes::Routes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailwayGraph {
    #[serde(with = "graph_serde")]
    pub graph: StableGraph<Node, TrackSegment>,
    pub station_name_to_index: HashMap<String, NodeIndex>,
    #[serde(default)]
    pub branch_angles: HashMap<(usize, usize), f64>,
}

impl RailwayGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: StableGraph::new(),
            station_name_to_index: HashMap::new(),
            branch_angles: HashMap::new(),
        }
    }
}

impl Default for RailwayGraph {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization helpers
mod graph_serde {
    use super::{TrackSegment, Node};
    use petgraph::stable_graph::StableGraph;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(graph: &StableGraph<Node, TrackSegment>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Petgraph's built-in serialization
        graph.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<StableGraph<Node, TrackSegment>, D::Error>
    where
        D: Deserializer<'de>,
    {
        StableGraph::deserialize(deserializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_graph_is_empty() {
        let graph = RailwayGraph::new();
        assert_eq!(graph.graph.node_count(), 0);
        assert_eq!(graph.graph.edge_count(), 0);
        assert!(graph.station_name_to_index.is_empty());
        assert!(graph.branch_angles.is_empty());
    }

    #[test]
    fn test_default_creates_empty_graph() {
        let graph = RailwayGraph::default();
        assert_eq!(graph.graph.node_count(), 0);
        assert_eq!(graph.graph.edge_count(), 0);
    }
}
