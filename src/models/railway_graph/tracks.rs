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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RailwayGraph, Stations};
    use crate::models::track::{Track, TrackDirection};

    #[test]
    fn test_add_track() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        let edge = graph.add_track(idx1, idx2, tracks);

        assert_eq!(graph.graph.edge_count(), 1);
        assert!(graph.get_track(edge).is_some());
        assert_eq!(graph.get_track_endpoints(edge), Some((idx1, idx2)));
    }

    #[test]
    fn test_get_track() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        let edge = graph.add_track(idx1, idx2, tracks);

        let track_segment = graph.get_track(edge).expect("track should exist");
        assert_eq!(track_segment.tracks.len(), 1);
    }

    #[test]
    fn test_get_track_endpoints() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        let edge = graph.add_track(idx1, idx2, tracks);

        let endpoints = graph.get_track_endpoints(edge);
        assert_eq!(endpoints, Some((idx1, idx2)));
    }

    #[test]
    fn test_toggle_segment_double_track() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());

        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        let edge = graph.add_track(idx1, idx2, tracks);

        // Initially single track
        assert_eq!(graph.get_track(edge).expect("track should exist").tracks.len(), 1);

        // Toggle to double track
        let changes = graph.toggle_segment_double_track("Station A", "Station B");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].1, 2); // 2 tracks now
        assert_eq!(graph.get_track(edge).expect("track should exist").tracks.len(), 2);

        // Toggle back to single track
        let changes = graph.toggle_segment_double_track("Station A", "Station B");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].1, 1); // 1 track now
        assert_eq!(graph.get_track(edge).expect("track should exist").tracks.len(), 1);
    }

    #[test]
    fn test_toggle_nonexistent_stations() {
        let mut graph = RailwayGraph::new();
        graph.add_or_get_station("Station A".to_string());

        let changes = graph.toggle_segment_double_track("Station A", "Nonexistent");
        assert_eq!(changes.len(), 0);

        let changes = graph.toggle_segment_double_track("Nonexistent1", "Nonexistent2");
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_toggle_bidirectional_edges() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());

        // Add edges in both directions
        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx1, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Toggle should affect both edges
        let changes = graph.toggle_segment_double_track("Station A", "Station B");
        assert_eq!(changes.len(), 2);

        assert_eq!(graph.get_track(edge1).expect("track should exist").tracks.len(), 2);
        assert_eq!(graph.get_track(edge2).expect("track should exist").tracks.len(), 2);
    }
}
