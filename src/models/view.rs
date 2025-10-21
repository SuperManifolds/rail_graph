use serde::{Deserialize, Serialize};
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
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
    #[serde(default)]
    pub viewport_state: ViewportState,
    /// Start and end stations for station range views
    pub station_range: Option<(NodeIndex, NodeIndex)>,
    /// Optional specific edge path to follow (for line views)
    #[serde(default)]
    pub edge_path: Option<Vec<usize>>,
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
    /// Returns a view even if the graph is empty (`station_range` will be None until data is imported)
    #[must_use]
    pub fn default_main_line(graph: &RailwayGraph) -> Self {
        let path = find_longest_path(graph);

        let station_range = if let (Some(&from), Some(&to)) = (path.first(), path.last()) {
            if path.len() >= 2 {
                Some((from, to))
            } else {
                None
            }
        } else {
            None
        };

        Self {
            id: Uuid::new_v4(),
            name: "Main Line".to_string(),
            viewport_state: ViewportState::default(),
            station_range,
            edge_path: None,
        }
    }

    /// Create a view from a station range
    ///
    /// # Errors
    /// Returns an error if no path exists between stations
    pub fn from_station_range(
        name: String,
        from: NodeIndex,
        to: NodeIndex,
        graph: &RailwayGraph,
    ) -> Result<Self, String> {
        // Verify path exists
        graph.find_path_between_nodes(from, to)
            .ok_or_else(|| "No path exists between the selected stations".to_string())?;

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            viewport_state: ViewportState::default(),
            station_range: Some((from, to)),
            edge_path: None,
        })
    }

    /// Create a view from a specific edge path (e.g., following a line's route)
    ///
    /// # Errors
    /// Returns an error if the edge path is empty or invalid
    pub fn from_edge_path(
        name: String,
        edge_path: Vec<usize>,
        graph: &RailwayGraph,
    ) -> Result<Self, String> {
        if edge_path.is_empty() {
            return Err("Edge path cannot be empty".to_string());
        }

        // Verify all edges exist and construct the node path
        let mut current: Option<NodeIndex> = None;
        let mut from: Option<NodeIndex> = None;
        let mut to: Option<NodeIndex> = None;

        for &edge_idx in &edge_path {
            let edge_index = EdgeIndex::new(edge_idx);
            let Some(endpoints) = graph.graph.edge_endpoints(edge_index) else {
                return Err(format!("Edge {edge_idx} does not exist"));
            };

            if let Some(curr) = current {
                // Determine which endpoint is next
                current = if endpoints.0 == curr {
                    Some(endpoints.1)
                } else if endpoints.1 == curr {
                    Some(endpoints.0)
                } else {
                    return Err("Edge path is not continuous".to_string());
                };
            } else {
                // First edge - start from first endpoint
                from = Some(endpoints.0);
                current = Some(endpoints.1);
            }

            to = current;
        }

        let (from, to) = from.zip(to).ok_or_else(|| "Could not determine start/end nodes".to_string())?;

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            viewport_state: ViewportState::default(),
            station_range: Some((from, to)),
            edge_path: Some(edge_path),
        })
    }

    /// Calculate the path for this view based on current graph state
    /// Returns None if the view shows everything (no station range), or if path cannot be calculated
    #[must_use]
    pub fn calculate_path(&self, graph: &RailwayGraph) -> Option<Vec<NodeIndex>> {
        let (from, to) = self.station_range?;

        // Use stored edge path if available, otherwise find any path
        let edge_indices = if let Some(ref stored_path) = self.edge_path {
            // Convert stored usize indices to EdgeIndex
            stored_path.iter().map(|&idx| EdgeIndex::new(idx)).collect()
        } else {
            // Use existing pathfinding that respects track directions
            graph.find_path_between_nodes(from, to)?
        };

        // Convert edge path to node path
        let mut path = vec![from];
        let mut current = from;

        for edge_idx in edge_indices {
            let edge = graph.graph.edge_endpoints(edge_idx)?;
            let next = if edge.0 == current {
                edge.1
            } else if edge.1 == current {
                edge.0
            } else {
                return None; // Path reconstruction failed
            };
            path.push(next);
            current = next;
        }

        Some(path)
    }

    /// Rename this view
    pub fn set_name(&mut self, new_name: String) {
        self.name = new_name;
    }

    /// Get the set of stations visible in this view
    #[must_use]
    pub fn visible_stations(&self, graph: &RailwayGraph) -> HashSet<NodeIndex> {
        if let Some(path) = self.calculate_path(graph) {
            path.iter().copied().collect()
        } else {
            // No station range means show all stations
            graph.graph.node_indices().collect()
        }
    }

    /// Get the ordered list of nodes (stations and junctions) for rendering this view
    /// Returns Vec<(`NodeIndex`, `Node`)>
    #[must_use]
    pub fn get_nodes_for_display(&self, graph: &RailwayGraph) -> Vec<(NodeIndex, crate::models::Node)> {
        if let Some(path) = self.calculate_path(graph) {
            path.iter()
                .filter_map(|&node_idx| {
                    graph.graph.node_weight(node_idx).map(|node| (node_idx, node.clone()))
                })
                .collect()
        } else {
            // No station range means show all nodes
            graph.get_all_nodes_ordered()
        }
    }

    /// Build a mapping from full-graph node indices to view display indices
    /// This is used for rendering conflicts/crossings which store indices from the full graph
    /// The display index accounts for ALL nodes (stations and junctions) in the view
    #[must_use]
    pub fn build_station_index_map(&self, graph: &RailwayGraph) -> std::collections::HashMap<usize, usize> {
        // Build a map from conflict detection indices (enumeration of node_indices())
        // to display indices (view order)
        // This matches how conflicts are created in worker_bridge.rs

        // First, create NodeIndex -> enumeration index (what conflicts use)
        let node_to_enum_idx: std::collections::HashMap<_, _> = graph.graph.node_indices()
            .enumerate()
            .map(|(enum_idx, node_idx)| (node_idx, enum_idx))
            .collect();

        if let Some(path) = self.calculate_path(graph) {
            // Map enumeration indices to display positions in the view
            path.iter()
                .enumerate()
                .filter_map(|(display_idx, &node_idx)| {
                    node_to_enum_idx.get(&node_idx).map(|&enum_idx| (enum_idx, display_idx))
                })
                .collect()
        } else {
            // No station range - get all nodes in BFS order
            let all_nodes = graph.get_all_nodes_ordered();
            all_nodes.iter()
                .enumerate()
                .filter_map(|(display_idx, (node_idx, _))| {
                    node_to_enum_idx.get(node_idx).map(|&enum_idx| (enum_idx, display_idx))
                })
                .collect()
        }
    }

    /// Filter journeys to only show the section visible in this view
    /// Journeys simply start/end at the view boundaries (which may be junctions)
    #[must_use]
    pub fn filter_journeys(&self, journeys: &[TrainJourney], graph: &RailwayGraph) -> Vec<TrainJourney> {
        let visible_stations = self.visible_stations(graph);

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
    pub fn filter_conflicts(&self, conflicts: &[Conflict], graph: &RailwayGraph, all_nodes: &[(petgraph::stable_graph::NodeIndex, crate::models::Node)]) -> Vec<Conflict> {
        let visible_nodes = self.visible_stations(graph);

        conflicts.iter()
            .filter(|conflict| {
                // Convert conflict indices to NodeIndex values
                let node1 = all_nodes.get(conflict.station1_idx).map(|(node_idx, _)| node_idx);
                let node2 = all_nodes.get(conflict.station2_idx).map(|(node_idx, _)| node_idx);

                // Include if both nodes involved are in the visible set
                match (node1, node2) {
                    (Some(n1), Some(n2)) => visible_nodes.contains(n1) && visible_nodes.contains(n2),
                    _ => false,
                }
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Track, TrackDirection};
    use crate::models::railway_graph::tracks::Tracks;

    #[test]
    fn test_view_structure() {
        let view = GraphView {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            viewport_state: ViewportState::default(),
            station_range: Some((NodeIndex::new(0), NodeIndex::new(2))),
            edge_path: None,
        };

        assert_eq!(view.name, "Test");
        assert!(view.station_range.is_some());
    }

    #[test]
    fn test_default_main_line_empty_graph() {
        let graph = RailwayGraph::new();
        let view = GraphView::default_main_line(&graph);

        assert_eq!(view.name, "Main Line");
        assert_eq!(view.station_range, None);
    }

    #[test]
    fn test_default_main_line_with_stations() {
        let mut graph = RailwayGraph::new();
        let s1 = graph.add_or_get_station("A".to_string());
        let s2 = graph.add_or_get_station("B".to_string());
        graph.add_track(s1, s2, vec![Track { direction: TrackDirection::Bidirectional }]);

        let view = GraphView::default_main_line(&graph);

        assert_eq!(view.name, "Main Line");
        assert!(view.station_range.is_some());
        let (from, to) = view.station_range.expect("station range should exist");
        assert!(from == s1 || from == s2);
        assert!(to == s1 || to == s2);
        assert_ne!(from, to);
    }

    #[test]
    fn test_calculate_path_with_graph() {
        let mut graph = RailwayGraph::new();
        let s1 = graph.add_or_get_station("A".to_string());
        let s2 = graph.add_or_get_station("B".to_string());
        let s3 = graph.add_or_get_station("C".to_string());
        graph.add_track(s1, s2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(s2, s3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let view = GraphView {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            viewport_state: ViewportState::default(),
            station_range: Some((s1, s3)),
            edge_path: None,
        };

        let path = view.calculate_path(&graph);
        assert!(path.is_some());
        let path = path.expect("path should be calculable");
        assert_eq!(path.len(), 3);
        assert_eq!(path[0], s1);
        assert_eq!(path[1], s2);
        assert_eq!(path[2], s3);
    }

    #[test]
    fn test_calculate_path_no_station_range() {
        let graph = RailwayGraph::new();
        let view = GraphView {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            viewport_state: ViewportState::default(),
            station_range: None,
            edge_path: None,
        };

        let path = view.calculate_path(&graph);
        assert_eq!(path, None);
    }
}
