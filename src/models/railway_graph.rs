use petgraph::stable_graph::{EdgeIndex, StableGraph, NodeIndex};
use petgraph::algo::dijkstra;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

    /// Finds the longest simple path in the graph (path with most nodes, no cycles).
    ///
    /// Tries starting from each node and returns the longest path found using DFS.
    #[must_use]
    pub fn find_longest_path(&self) -> Vec<NodeIndex> {
        let mut longest_path = Vec::new();

        // Try starting from each node
        for start_node in self.graph.node_indices() {
            let mut visited = HashSet::new();
            let mut current_path = Vec::new();

            Self::dfs_longest_path(self, start_node, &mut visited, &mut current_path, &mut longest_path);
        }

        longest_path
    }

    /// Finds the longest path starting from a specific node, avoiding already visited nodes.
    ///
    /// # Arguments
    /// * `start` - Starting node for the search
    /// * `global_visited` - Set of nodes to exclude from the search
    #[must_use]
    pub fn find_longest_path_from(
        &self,
        start: NodeIndex,
        global_visited: &HashSet<NodeIndex>,
    ) -> Vec<NodeIndex> {
        let mut longest_path = Vec::new();
        let mut visited = global_visited.clone();
        let mut current_path = Vec::new();

        Self::dfs_longest_path_excluding(self, start, &mut visited, &mut current_path, &mut longest_path);

        longest_path
    }

    /// DFS helper to find longest simple path.
    fn dfs_longest_path(
        &self,
        current: NodeIndex,
        visited: &mut HashSet<NodeIndex>,
        current_path: &mut Vec<NodeIndex>,
        longest_path: &mut Vec<NodeIndex>,
    ) {
        visited.insert(current);
        current_path.push(current);

        // Update longest if current is longer
        if current_path.len() > longest_path.len() {
            *longest_path = current_path.clone();
        }

        // Try extending path to each unvisited neighbor
        for neighbor in self.graph.neighbors_undirected(current) {
            if !visited.contains(&neighbor) {
                // Check if this transition is allowed through junction routing rules
                if self.is_path_allowed(current_path, current, neighbor) {
                    Self::dfs_longest_path(self, neighbor, visited, current_path, longest_path);
                }
            }
        }

        // Backtrack
        current_path.pop();
        visited.remove(&current);
    }

    /// Finds the main trunk path by greedily following highest-weight edges.
    ///
    /// Strategy:
    /// 1. Find the highest-weight edge - this is on the main trunk
    /// 2. From both endpoints, greedily extend by following the highest-weight unvisited neighbor
    /// 3. Combine the two directions into the spine
    ///
    /// This ensures we stay on the "main corridor" rather than meandering through branches.
    /// Falls back to longest path if all edges have zero weight.
    #[must_use]
    pub fn find_heaviest_path(&self, edge_weights: &HashMap<EdgeIndex, usize>) -> Vec<NodeIndex> {
        // Check if there are any non-zero weights
        let has_weights = edge_weights.values().any(|&w| w > 0);
        if !has_weights {
            return self.find_longest_path();
        }

        // Find the highest-weight edge - this is definitely on the main trunk
        let Some((&best_edge, _)) = edge_weights.iter().max_by_key(|(_, &w)| w) else {
            return self.find_longest_path();
        };

        let Some((node_a, node_b)) = self.graph.edge_endpoints(best_edge) else {
            return self.find_longest_path();
        };

        // Greedily extend from node_a (away from node_b)
        let mut visited = HashSet::new();
        visited.insert(node_a);
        visited.insert(node_b);

        let path_from_a = self.greedy_extend(node_a, Some(node_b), &mut visited.clone(), edge_weights);

        // Greedily extend from node_b (away from node_a)
        let path_from_b = self.greedy_extend(node_b, Some(node_a), &mut visited, edge_weights);

        // Combine: reverse path_from_a + node_a + node_b + path_from_b
        let mut spine: Vec<NodeIndex> = path_from_a.into_iter().rev().collect();
        spine.push(node_a);
        spine.push(node_b);
        spine.extend(path_from_b);

        spine
    }

    /// Greedily extend a path by always following the highest-weight unvisited neighbor.
    fn greedy_extend(
        &self,
        start: NodeIndex,
        came_from: Option<NodeIndex>,
        visited: &mut HashSet<NodeIndex>,
        edge_weights: &HashMap<EdgeIndex, usize>,
    ) -> Vec<NodeIndex> {
        let mut path = Vec::new();
        let mut current = start;
        let mut prev = came_from;

        loop {
            // Find the highest-weight unvisited neighbor
            let mut best_neighbor: Option<NodeIndex> = None;
            let mut best_weight: usize = 0;

            for neighbor in self.graph.neighbors_undirected(current) {
                if visited.contains(&neighbor) {
                    continue;
                }

                // Build a minimal path for junction routing check
                let check_path: Vec<NodeIndex> = if let Some(p) = prev {
                    vec![p, current]
                } else {
                    vec![current]
                };

                if !self.is_path_allowed(&check_path, current, neighbor) {
                    continue;
                }

                let edge = self.graph.find_edge(current, neighbor)
                    .or_else(|| self.graph.find_edge(neighbor, current));
                let weight = edge.and_then(|e| edge_weights.get(&e).copied()).unwrap_or(0);

                if weight > best_weight || (weight == best_weight && best_neighbor.is_none()) {
                    best_weight = weight;
                    best_neighbor = Some(neighbor);
                }
            }

            // If no unvisited neighbor, we're done
            let Some(next) = best_neighbor else {
                break;
            };

            visited.insert(next);
            path.push(next);
            prev = Some(current);
            current = next;
        }

        path
    }

    /// DFS helper that respects global visited set.
    fn dfs_longest_path_excluding(
        &self,
        current: NodeIndex,
        visited: &mut HashSet<NodeIndex>,
        current_path: &mut Vec<NodeIndex>,
        longest_path: &mut Vec<NodeIndex>,
    ) {
        if visited.contains(&current) {
            return;
        }

        visited.insert(current);
        current_path.push(current);

        if current_path.len() > longest_path.len() {
            *longest_path = current_path.clone();
        }

        for neighbor in self.graph.neighbors_undirected(current) {
            if !visited.contains(&neighbor) {
                // Check if this transition is allowed through junction routing rules
                if self.is_path_allowed(current_path, current, neighbor) {
                    Self::dfs_longest_path_excluding(self, neighbor, visited, current_path, longest_path);
                }
            }
        }

        current_path.pop();
        visited.remove(&current);
    }

    /// Check if moving from current to neighbor is allowed by junction routing rules.
    ///
    /// If current is a junction, checks if the transition from the incoming edge
    /// (from previous node in path) to the outgoing edge (to neighbor) is allowed.
    fn is_path_allowed(
        &self,
        current_path: &[NodeIndex],
        current: NodeIndex,
        neighbor: NodeIndex,
    ) -> bool {
        use crate::models::Junctions;

        // If current is not a junction, allow the transition
        if !self.is_junction(current) {
            return true;
        }

        // If this is the first or second node in the path, allow (no incoming edge to check)
        if current_path.len() < 2 {
            return true;
        }

        // Get the previous node (where we came from)
        let prev_node = current_path[current_path.len() - 2];

        // Find the incoming edge (prev_node -> current)
        let incoming_edge = self
            .graph
            .edges_connecting(prev_node, current)
            .next()
            .or_else(|| self.graph.edges_connecting(current, prev_node).next())
            .map(|e| e.id());

        // Find the outgoing edge (current -> neighbor)
        let outgoing_edge = self
            .graph
            .edges_connecting(current, neighbor)
            .next()
            .or_else(|| self.graph.edges_connecting(neighbor, current).next())
            .map(|e| e.id());

        // Check junction routing rules
        match (incoming_edge, outgoing_edge) {
            (Some(inc), Some(out)) => {
                if let Some(junction) = self.get_junction(current) {
                    junction.is_routing_allowed(inc, out)
                } else {
                    true
                }
            }
            _ => true, // If we can't find edges, allow by default
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
