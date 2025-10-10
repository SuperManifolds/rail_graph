use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use super::RailwayGraph;
use crate::models::station::{StationNode, default_platforms};

/// Extension trait for station-related operations on `RailwayGraph`
pub trait Stations {
    /// Add a station node if it doesn't exist, return its `NodeIndex`
    fn add_or_get_station(&mut self, name: String) -> NodeIndex;

    /// Update station position
    fn set_station_position(&mut self, index: NodeIndex, position: (f64, f64));

    /// Get station position
    fn get_station_position(&self, index: NodeIndex) -> Option<(f64, f64)>;

    /// Get station name by `NodeIndex`
    fn get_station_name(&self, index: NodeIndex) -> Option<&str>;

    /// Get `NodeIndex` by station name
    fn get_station_index(&self, name: &str) -> Option<NodeIndex>;

    /// Get all edge indices connected to a station
    fn get_station_edges(&self, index: NodeIndex) -> Vec<usize>;

    /// Find stations connected through a given station
    /// Returns a Vec of (`station_before`, `station_after`, tracks) tuples
    fn find_connections_through_station(&self, station_idx: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Vec<crate::models::track::Track>)>;

    /// Delete a station and reconnect around it
    /// Returns (`removed_edges`, `bypass_mapping`) where `bypass_mapping` maps (`old_edge1`, `old_edge2`) -> `new_edge`
    fn delete_station(&mut self, index: NodeIndex) -> (Vec<usize>, std::collections::HashMap<(usize, usize), usize>);

    /// Get all stations in order by traversing the graph
    /// Performs a breadth-first traversal starting from the first station
    fn get_all_stations_ordered(&self) -> Vec<StationNode>;
}

impl Stations for RailwayGraph {
    fn add_or_get_station(&mut self, name: String) -> NodeIndex {
        if let Some(&index) = self.station_name_to_index.get(&name) {
            index
        } else {
            let index = self.graph.add_node(StationNode {
                name: name.clone(),
                position: None,
                passing_loop: false,
                platforms: default_platforms(),
            });
            self.station_name_to_index.insert(name, index);
            index
        }
    }

    fn set_station_position(&mut self, index: NodeIndex, position: (f64, f64)) {
        if let Some(node) = self.graph.node_weight_mut(index) {
            node.position = Some(position);
        }
    }

    fn get_station_position(&self, index: NodeIndex) -> Option<(f64, f64)> {
        self.graph.node_weight(index).and_then(|node| node.position)
    }

    fn get_station_name(&self, index: NodeIndex) -> Option<&str> {
        self.graph.node_weight(index).map(|node| node.name.as_str())
    }

    fn get_station_index(&self, name: &str) -> Option<NodeIndex> {
        self.station_name_to_index.get(name).copied()
    }

    fn get_station_edges(&self, index: NodeIndex) -> Vec<usize> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;

        self.graph.edges(index)
            .map(|e| e.id().index())
            .chain(self.graph.edges_directed(index, Direction::Incoming).map(|e| e.id().index()))
            .collect()
    }

    fn find_connections_through_station(&self, station_idx: NodeIndex) -> Vec<(NodeIndex, NodeIndex, Vec<crate::models::track::Track>)> {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;
        use crate::models::track::{Track, TrackDirection};

        let mut connections = Vec::new();

        // Get incoming edges (edges pointing to this station)
        let incoming: Vec<_> = self.graph.edges_directed(station_idx, Direction::Incoming)
            .map(|e| (e.source(), &e.weight().tracks))
            .collect();

        // Get outgoing edges (edges from this station)
        let outgoing: Vec<_> = self.graph.edges(station_idx)
            .map(|e| (e.target(), &e.weight().tracks))
            .collect();

        // Create connections from each incoming station to each outgoing station
        for (from_station, from_tracks) in &incoming {
            for (to_station, to_tracks) in &outgoing {
                // Use the max number of tracks from either segment
                let track_count = from_tracks.len().max(to_tracks.len());
                let tracks = (0..track_count).map(|_| Track { direction: TrackDirection::Bidirectional }).collect();
                connections.push((*from_station, *to_station, tracks));
            }
        }

        connections
    }

    fn delete_station(&mut self, index: NodeIndex) -> (Vec<usize>, std::collections::HashMap<(usize, usize), usize>) {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;
        use super::tracks::Tracks;

        // Find connections through this station to create bypass edges
        let connections = self.find_connections_through_station(index);

        // Create bypass edges and track the mapping
        let mut bypass_mapping = std::collections::HashMap::new();

        for (from_station, to_station, tracks) in connections {
            // Find the incoming and outgoing edges for this connection
            let incoming_edge = self.graph.edges_directed(index, Direction::Incoming)
                .find(|e| e.source() == from_station)
                .map(|e| e.id().index());

            let outgoing_edge = self.graph.edges(index)
                .find(|e| e.target() == to_station)
                .map(|e| e.id().index());

            if let (Some(edge1), Some(edge2)) = (incoming_edge, outgoing_edge) {
                let new_edge = self.add_track(from_station, to_station, tracks);
                bypass_mapping.insert((edge1, edge2), new_edge.index());
            }
        }

        // Get edges that will be removed
        let removed_edges = self.get_station_edges(index);

        // Remove station from name mapping
        if let Some(station) = self.graph.node_weight(index) {
            self.station_name_to_index.remove(&station.name);
        }

        // Remove the station node (this also removes all connected edges)
        self.graph.remove_node(index);

        (removed_edges, bypass_mapping)
    }

    fn get_all_stations_ordered(&self) -> Vec<StationNode> {
        if self.graph.node_count() == 0 {
            return Vec::new();
        }

        let mut ordered = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let Some(start_node) = self.graph.node_indices().next() else {
            return Vec::new();
        };

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
