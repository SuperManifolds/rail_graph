use petgraph::graph::NodeIndex;
use super::RailwayGraph;

/// Extension trait for route-related operations on `RailwayGraph`
pub trait Routes {
    /// Extract ordered list of stations from a route based on direction
    /// Returns Vec of (`station_name`, `NodeIndex`) in the order they're visited
    fn get_stations_from_route(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)>;

    /// Get the first and last station indices for a route based on direction
    /// Returns (Option<`first_station`>, Option<`last_station`>)
    fn get_route_endpoints(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> (Option<NodeIndex>, Option<NodeIndex>);

    /// Get available stations that can be added at the start of a route
    /// Returns station names that have edges connecting to the first station
    fn get_available_start_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String>;

    /// Get available stations that can be added at the end of a route
    /// Returns station names that have edges connecting from the last station
    fn get_available_end_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String>;
}

impl Routes for RailwayGraph {
    fn get_stations_from_route(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<(String, NodeIndex)> {
        use super::stations::Stations;
        use super::tracks::Tracks;

        let mut stations = Vec::new();

        match direction {
            crate::models::RouteDirection::Forward => {
                // Forward: extract from -> to for each edge
                if let Some((from, name)) = route.first().and_then(|segment| {
                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                    self.get_track_endpoints(edge_idx).and_then(|(from, _)| {
                        self.get_station_name(from).map(|name| (from, name.to_string()))
                    })
                }) {
                    stations.push((name, from));
                }

                for segment in route {
                    self.add_station_if_exists(segment.edge_index, false, &mut stations);
                }
            }
            crate::models::RouteDirection::Return => {
                // Return: extract to -> from for each edge (traveling backwards)
                if let Some((to, name)) = route.first().and_then(|segment| {
                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                    self.get_track_endpoints(edge_idx).and_then(|(_, to)| {
                        self.get_station_name(to).map(|name| (to, name.to_string()))
                    })
                }) {
                    stations.push((name, to));
                }

                for segment in route {
                    self.add_station_if_exists(segment.edge_index, true, &mut stations);
                }
            }
        }

        stations
    }

    fn get_route_endpoints(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> (Option<NodeIndex>, Option<NodeIndex>) {
        use super::tracks::Tracks;

        match direction {
            crate::models::RouteDirection::Forward => {
                let first = route.first()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(from, _)| from)
                    });
                let last = route.last()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(_, to)| to)
                    });
                (first, last)
            }
            crate::models::RouteDirection::Return => {
                // Return route segments travel backwards on edges
                // First segment's 'to' is the starting station
                let first = route.first()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(_, to)| to)
                    });
                // Last segment's 'from' is the ending station
                let last = route.last()
                    .and_then(|seg| {
                        let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                        self.get_track_endpoints(edge).map(|(from, _)| from)
                    });
                (first, last)
            }
        }
    }

    fn get_available_start_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String> {
        use super::stations::Stations;

        let (first_idx, _) = self.get_route_endpoints(route, direction);

        let Some(first_idx) = first_idx else {
            return Vec::new();
        };

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|station| {
                let station_idx = self.get_station_index(&station.name)?;
                // For forward: find edge from station_idx to first_idx
                // For return: find edge from first_idx to station_idx (traveling backwards)
                let has_edge = match direction {
                    crate::models::RouteDirection::Forward => self.graph.find_edge(station_idx, first_idx).is_some(),
                    crate::models::RouteDirection::Return => self.graph.find_edge(first_idx, station_idx).is_some(),
                };
                if has_edge {
                    Some(station.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_available_end_stations(
        &self,
        route: &[crate::models::RouteSegment],
        direction: crate::models::RouteDirection,
    ) -> Vec<String> {
        use super::stations::Stations;

        let (_, last_idx) = self.get_route_endpoints(route, direction);

        let Some(last_idx) = last_idx else {
            return Vec::new();
        };

        self.get_all_stations_ordered()
            .iter()
            .filter_map(|station| {
                let station_idx = self.get_station_index(&station.name)?;
                // For forward: find edge from last_idx to station_idx
                // For return: find edge from station_idx to last_idx (traveling backwards)
                let has_edge = match direction {
                    crate::models::RouteDirection::Forward => self.graph.find_edge(last_idx, station_idx).is_some(),
                    crate::models::RouteDirection::Return => self.graph.find_edge(station_idx, last_idx).is_some(),
                };
                if has_edge {
                    Some(station.name.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

impl RailwayGraph {
    /// Helper to add a station to the list if it exists
    /// If `use_from` is true, adds the 'from' station, otherwise adds the 'to' station
    fn add_station_if_exists(&self, edge_index: usize, use_from: bool, stations: &mut Vec<(String, NodeIndex)>) {
        use super::tracks::Tracks;
        use super::stations::Stations;

        let edge_idx = petgraph::graph::EdgeIndex::new(edge_index);
        let Some((from, to)) = self.get_track_endpoints(edge_idx) else {
            return;
        };
        let station_idx = if use_from { from } else { to };
        let Some(name) = self.get_station_name(station_idx) else {
            return;
        };
        stations.push((name.to_string(), station_idx));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RailwayGraph, Stations, Tracks, RouteDirection, RouteSegment};
    use crate::models::track::{Track, TrackDirection};
    use chrono::Duration;

    fn create_test_route_segment(edge_index: usize) -> RouteSegment {
        RouteSegment {
            edge_index,
            track_index: 0,
            origin_platform: 0,
            destination_platform: 0,
            duration: Duration::minutes(5),
            wait_time: Duration::seconds(30),
        }
    }

    #[test]
    fn test_get_route_endpoints_forward() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            create_test_route_segment(edge1.index()),
            create_test_route_segment(edge2.index()),
        ];

        let (first, last) = graph.get_route_endpoints(&route, RouteDirection::Forward);
        assert_eq!(first, Some(idx1));
        assert_eq!(last, Some(idx3));
    }

    #[test]
    fn test_get_route_endpoints_return() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            create_test_route_segment(edge2.index()),
            create_test_route_segment(edge1.index()),
        ];

        let (first, last) = graph.get_route_endpoints(&route, RouteDirection::Return);
        assert_eq!(first, Some(idx3));
        assert_eq!(last, Some(idx1));
    }

    #[test]
    fn test_get_route_endpoints_empty() {
        let graph = RailwayGraph::new();
        let route = vec![];

        let (first, last) = graph.get_route_endpoints(&route, RouteDirection::Forward);
        assert_eq!(first, None);
        assert_eq!(last, None);
    }

    #[test]
    fn test_get_stations_from_route_forward() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        let route = vec![
            create_test_route_segment(edge1.index()),
            create_test_route_segment(edge2.index()),
        ];

        let stations = graph.get_stations_from_route(&route, RouteDirection::Forward);
        assert_eq!(stations.len(), 3);
        assert_eq!(stations[0].0, "Station A");
        assert_eq!(stations[1].0, "Station B");
        assert_eq!(stations[2].0, "Station C");
    }

    #[test]
    fn test_get_stations_from_route_return() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Return route: C -> B -> A
        let route = vec![
            create_test_route_segment(edge2.index()),
            create_test_route_segment(edge1.index()),
        ];

        let stations = graph.get_stations_from_route(&route, RouteDirection::Return);
        assert_eq!(stations.len(), 3);
        assert_eq!(stations[0].0, "Station C");
        assert_eq!(stations[1].0, "Station B");
        assert_eq!(stations[2].0, "Station A");
    }

    #[test]
    fn test_get_available_start_stations() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        // Create: A -> B -> C
        graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Route currently starts at B
        let route = vec![create_test_route_segment(edge2.index())];

        let available = graph.get_available_start_stations(&route, RouteDirection::Forward);
        assert_eq!(available.len(), 1);
        assert!(available.contains(&"Station A".to_string()));
    }

    #[test]
    fn test_get_available_end_stations() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        // Create: A -> B -> C
        let edge1 = graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Route currently ends at B
        let route = vec![create_test_route_segment(edge1.index())];

        let available = graph.get_available_end_stations(&route, RouteDirection::Forward);
        assert_eq!(available.len(), 1);
        assert!(available.contains(&"Station C".to_string()));
    }

    #[test]
    fn test_get_available_stations_empty_route() {
        let mut graph = RailwayGraph::new();
        graph.add_or_get_station("Station A".to_string());

        let route = vec![];

        let start_stations = graph.get_available_start_stations(&route, RouteDirection::Forward);
        assert_eq!(start_stations.len(), 0);

        let end_stations = graph.get_available_end_stations(&route, RouteDirection::Forward);
        assert_eq!(end_stations.len(), 0);
    }
}
