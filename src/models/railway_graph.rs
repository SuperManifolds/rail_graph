use petgraph::stable_graph::{StableGraph, NodeIndex};
use petgraph::algo::dijkstra;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use super::node::Node;
use super::track::TrackSegment;
use super::project::SpacingMode;

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

    /// Calculate Y positions for stations based on spacing mode
    ///
    /// # Arguments
    /// * `stations` - Ordered list of stations to position
    /// * `spacing_mode` - Whether to use equal spacing or distance-based spacing
    /// * `total_height` - Total height available for positioning
    /// * `top_margin` - Top margin offset for Y positions
    ///
    /// # Returns
    /// Vector of Y positions, one for each station (at their vertical center)
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn calculate_station_positions(
        &self,
        stations: &[(NodeIndex, Node)],
        spacing_mode: SpacingMode,
        total_height: f64,
        top_margin: f64,
    ) -> Vec<f64> {
        if stations.is_empty() {
            return Vec::new();
        }

        match spacing_mode {
            SpacingMode::Equal => {
                let station_height = total_height / stations.len() as f64;
                stations
                    .iter()
                    .enumerate()
                    .map(|(idx, _)| top_margin + (idx as f64 * station_height) + (station_height / 2.0))
                    .collect()
            }
            SpacingMode::DistanceBased => {
                self.calculate_distance_based_positions(stations, total_height, top_margin)
            }
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn calculate_distance_based_positions(
        &self,
        stations: &[(NodeIndex, Node)],
        total_height: f64,
        top_margin: f64,
    ) -> Vec<f64> {
        // First pass: collect all valid distances to calculate average
        let mut valid_distances = Vec::new();
        for i in 0..stations.len() - 1 {
            let from_idx = stations[i].0;
            let to_idx = stations[i + 1].0;
            let distance = self.find_shortest_distance(from_idx, to_idx);
            if distance > 0.0 {
                valid_distances.push(distance);
            }
        }

        // Calculate fallback distance (average of valid distances, or 1.0 if none)
        let fallback_distance = if valid_distances.is_empty() {
            1.0
        } else {
            valid_distances.iter().sum::<f64>() / valid_distances.len() as f64
        };

        // Second pass: build cumulative distances using fallback for invalid segments
        let mut cumulative_distances = vec![0.0];
        for i in 0..stations.len() - 1 {
            let from_idx = stations[i].0;
            let to_idx = stations[i + 1].0;

            let distance = self.find_shortest_distance(from_idx, to_idx);
            let segment_distance = if distance > 0.0 {
                distance
            } else {
                fallback_distance
            };

            let last_cumulative = cumulative_distances.last().copied().unwrap_or(0.0);
            cumulative_distances.push(last_cumulative + segment_distance);
        }

        // Normalize to fit within total_height
        let total_distance = cumulative_distances.last().copied().unwrap_or(1.0);
        let scale = if total_distance > 0.0 {
            total_height / total_distance
        } else {
            1.0
        };

        // Convert cumulative distances to Y positions (centered in each station's area)
        cumulative_distances
            .iter()
            .map(|&cum_dist| top_margin + (cum_dist * scale))
            .collect()
    }

    fn find_shortest_distance(&self, from: NodeIndex, to: NodeIndex) -> f64 {
        // Use Dijkstra's algorithm with distance as edge weight
        let distances = dijkstra(
            &self.graph,
            from,
            Some(to),
            |edge| {
                edge.weight()
                    .distance
                    .filter(|&d| d > 0.0) // Only use valid positive distances
                    .unwrap_or(1.0) // Default to 1.0 for missing distances (normalization is handled in calculate_distance_based_positions)
            },
        );

        distances.get(&to).copied().unwrap_or(0.0)
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
