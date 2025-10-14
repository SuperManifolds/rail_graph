use serde::{Deserialize, Serialize};
use petgraph::stable_graph::NodeIndex;
use uuid::Uuid;
use std::collections::HashSet;
use super::RailwayGraph;
use super::railway_graph::stations::Stations;
use super::railway_graph::routes::Routes;
use crate::train_journey::TrainJourney;
use crate::conflict::Conflict;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct GraphView {
    pub id: Uuid,
    pub name: String,
    pub path: Vec<NodeIndex>,
    #[serde(default)]
    pub viewport_state: ViewportState,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct ViewportState {
    #[serde(default = "default_zoom")]
    pub zoom_level: f64,
    #[serde(default)]
    pub zoom_level_x: Option<f64>,
    #[serde(default)]
    pub pan_offset_x: f64,
    #[serde(default)]
    pub pan_offset_y: f64,
}

fn default_zoom() -> f64 {
    1.0
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            zoom_level: 1.0,
            zoom_level_x: None,
            pan_offset_x: 0.0,
            pan_offset_y: 0.0,
        }
    }
}

/// Find the longest simple path in the graph (the "main line")
/// Uses DFS to find the longest path starting from each node
fn find_longest_path(graph: &RailwayGraph) -> Vec<NodeIndex> {
    use std::collections::HashSet;

    let mut longest_path = Vec::new();

    // Try starting from each node
    for start_node in graph.graph.node_indices() {
        let mut visited = HashSet::new();
        let mut current_path = Vec::new();
        dfs_longest_path(graph, start_node, &mut visited, &mut current_path, &mut longest_path);
    }

    longest_path
}

/// DFS helper to find longest path
fn dfs_longest_path(
    graph: &RailwayGraph,
    current: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    current_path: &mut Vec<NodeIndex>,
    longest_path: &mut Vec<NodeIndex>,
) {
    visited.insert(current);
    current_path.push(current);

    // Update longest path if current is longer
    if current_path.len() > longest_path.len() {
        *longest_path = current_path.clone();
    }

    // Try each unvisited neighbor
    for neighbor in graph.graph.neighbors_undirected(current) {
        if !visited.contains(&neighbor) {
            dfs_longest_path(graph, neighbor, visited, current_path, longest_path);
        }
    }

    // Backtrack
    current_path.pop();
    visited.remove(&current);
}

impl GraphView {
    /// Create a default view showing the longest path in the graph (the "main line")
    #[must_use]
    pub fn default_main_line(graph: &RailwayGraph) -> Option<Self> {
        let path = find_longest_path(graph);
        if path.is_empty() {
            return None;
        }

        Some(Self {
            id: Uuid::new_v4(),
            name: "Main Line".to_string(),
            path,
            viewport_state: ViewportState::default(),
        })
    }

    /// Create a view from a station range by finding the shortest path
    ///
    /// # Errors
    /// Returns an error if no path exists between stations or path reconstruction fails
    pub fn from_station_range(
        name: String,
        from: NodeIndex,
        to: NodeIndex,
        graph: &RailwayGraph,
    ) -> Result<Self, String> {
        // Use existing pathfinding that respects track directions
        let edge_path = graph.find_path_between_nodes(from, to)
            .ok_or_else(|| "No path exists between the selected stations".to_string())?;

        // Convert edge path to node path
        let mut path = vec![from];
        let mut current = from;

        for edge_idx in edge_path {
            if let Some(edge) = graph.graph.edge_endpoints(edge_idx) {
                // Determine which endpoint is the next node
                let next = if edge.0 == current {
                    edge.1
                } else if edge.1 == current {
                    edge.0
                } else {
                    return Err("Path reconstruction failed: edge doesn't connect to current node".to_string());
                };
                path.push(next);
                current = next;
            } else {
                return Err("Path reconstruction failed: edge not found".to_string());
            }
        }

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            path,
            viewport_state: ViewportState::default(),
        })
    }

    /// Create a new view with the given name and path
    #[must_use]
    pub fn new(name: String, path: Vec<NodeIndex>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            path,
            viewport_state: ViewportState::default(),
        }
    }

    /// Rename this view
    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name;
    }

    /// Get the set of stations visible in this view
    #[must_use]
    pub fn visible_stations(&self) -> HashSet<NodeIndex> {
        self.path.iter().copied().collect()
    }

    /// Get the ordered list of nodes (stations and junctions) for rendering this view
    /// Returns Vec<(`NodeIndex`, `Node`)>
    #[must_use]
    pub fn get_nodes_for_display(&self, graph: &RailwayGraph) -> Vec<(NodeIndex, crate::models::Node)> {
        self.path.iter()
            .filter_map(|&node_idx| {
                graph.graph.node_weight(node_idx).map(|node| (node_idx, node.clone()))
            })
            .collect()
    }

    /// Build a mapping from full-graph station indices to view display indices
    /// This is used for rendering conflicts/crossings which store indices from the full graph
    /// The display index accounts for ALL nodes (stations and junctions) in the view
    #[must_use]
    pub fn build_station_index_map(&self, graph: &RailwayGraph) -> std::collections::HashMap<usize, usize> {
        let all_stations = graph.get_all_stations_ordered();

        // Build a reverse map: NodeIndex -> station index in full graph
        let node_to_station_idx: std::collections::HashMap<NodeIndex, usize> = all_stations
            .iter()
            .enumerate()
            .map(|(idx, (node_idx, _))| (*node_idx, idx))
            .collect();

        // Map station indices to display positions (which include junctions)
        self.path.iter()
            .enumerate()
            .filter_map(|(display_idx, &node_idx)| {
                // Only map if this node is a station
                node_to_station_idx.get(&node_idx).map(|&station_idx| (station_idx, display_idx))
            })
            .collect()
    }

    /// Filter journeys to only show the section visible in this view
    /// Journeys simply start/end at the view boundaries (which may be junctions)
    #[must_use]
    pub fn filter_journeys(&self, journeys: &[TrainJourney], _graph: &RailwayGraph) -> Vec<TrainJourney> {
        let visible_stations: HashSet<NodeIndex> = self.path.iter().copied().collect();

        journeys.iter()
            .filter_map(|journey| {
                // Simply filter to only visible nodes, keeping original times
                let filtered_times: Vec<_> = journey.station_times.iter()
                    .filter(|(node_idx, _, _)| visible_stations.contains(node_idx))
                    .copied()
                    .collect();

                if filtered_times.is_empty() {
                    None
                } else {
                    let mut filtered_journey = journey.clone();
                    filtered_journey.station_times = filtered_times;
                    Some(filtered_journey)
                }
            })
            .collect()
    }

    /// Filter conflicts to only those within this path
    #[must_use]
    pub fn filter_conflicts(&self, conflicts: &[Conflict]) -> Vec<Conflict> {
        let visible_stations = self.visible_stations();

        conflicts.iter()
            .filter(|conflict| {
                // Include if both stations involved are in the visible set
                visible_stations.contains(&NodeIndex::new(conflict.station1_idx)) &&
                visible_stations.contains(&NodeIndex::new(conflict.station2_idx))
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visible_stations() {
        let view = GraphView {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            path: vec![NodeIndex::new(0), NodeIndex::new(1), NodeIndex::new(2)],
            viewport_state: ViewportState::default(),
        };

        let visible = view.visible_stations();
        assert_eq!(visible.len(), 3);
        assert!(visible.contains(&NodeIndex::new(0)));
        assert!(visible.contains(&NodeIndex::new(1)));
        assert!(visible.contains(&NodeIndex::new(2)));
    }
}
