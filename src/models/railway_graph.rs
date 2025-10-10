use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::station::StationNode;
use super::track::TrackSegment;

pub mod stations;
pub mod tracks;
pub mod routes;

// Re-export extension traits
pub use stations::Stations;
pub use tracks::Tracks;
pub use routes::Routes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RailwayGraph {
    #[serde(with = "graph_serde")]
    pub graph: DiGraph<StationNode, TrackSegment>,
    pub station_name_to_index: HashMap<String, NodeIndex>,
    #[serde(default)]
    pub branch_angles: HashMap<(usize, usize), f64>,
}

impl RailwayGraph {
    #[must_use]
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
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
