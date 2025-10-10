use petgraph::graph::NodeIndex;
use super::RailwayGraph;
use crate::models::track::{Track, TrackSegment};

/// Extension trait for track-related operations on `RailwayGraph`
pub trait Tracks {
    /// Add a track segment between two stations, returns the `EdgeIndex`
    fn add_track(&mut self, from: NodeIndex, to: NodeIndex, tracks: Vec<Track>) -> petgraph::graph::EdgeIndex;

    /// Get track segment by edge index
    fn get_track(&self, edge_idx: petgraph::graph::EdgeIndex) -> Option<&TrackSegment>;

    /// Get endpoints of a track segment
    fn get_track_endpoints(&self, edge_idx: petgraph::graph::EdgeIndex) -> Option<(NodeIndex, NodeIndex)>;

    /// Toggle between single and double track for edges between two stations
    /// Returns a Vec of (`edge_index`, `new_track_count`) for all modified edges
    fn toggle_segment_double_track(&mut self, station1_name: &str, station2_name: &str) -> Vec<(usize, usize)>;
}

impl Tracks for RailwayGraph {
    fn add_track(&mut self, from: NodeIndex, to: NodeIndex, tracks: Vec<Track>) -> petgraph::graph::EdgeIndex {
        self.graph.add_edge(from, to, TrackSegment { tracks, distance: None })
    }

    fn get_track(&self, edge_idx: petgraph::graph::EdgeIndex) -> Option<&TrackSegment> {
        self.graph.edge_weight(edge_idx)
    }

    fn get_track_endpoints(&self, edge_idx: petgraph::graph::EdgeIndex) -> Option<(NodeIndex, NodeIndex)> {
        self.graph.edge_endpoints(edge_idx)
    }

    fn toggle_segment_double_track(&mut self, station1_name: &str, station2_name: &str) -> Vec<(usize, usize)> {
        use super::stations::Stations;

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
}
