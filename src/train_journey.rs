use crate::models::{Line, RailwayGraph, ScheduleMode, Stations, Tracks};
use crate::constants::{BASE_DATE, GENERATION_END_HOUR};
use chrono::{Duration, NaiveDateTime, Timelike};
use std::collections::HashMap;

const MAX_JOURNEYS_PER_LINE: i32 = 100; // Limit to prevent performance issues

#[derive(Debug, Clone)]
pub struct JourneySegment {
    pub edge_index: usize,
    pub track_index: usize,
    pub origin_platform: usize,
    pub destination_platform: usize,
}

#[derive(Debug, Clone)]
pub struct TrainJourney {
    pub id: uuid::Uuid,
    pub line_id: String,
    pub departure_time: NaiveDateTime,
    pub station_times: Vec<(String, NaiveDateTime, NaiveDateTime)>, // (station_name, arrival_time, departure_time)
    pub segments: Vec<JourneySegment>, // Track and platform info for each segment
    pub color: String,
    pub thickness: f64,
}

impl TrainJourney {
    /// Generate train journeys for all lines throughout the day
    #[must_use]
    pub fn generate_journeys(lines: &[Line], graph: &RailwayGraph) -> HashMap<uuid::Uuid, TrainJourney> {
        let Some(day_end) = BASE_DATE.and_hms_opt(23, 59, 59) else {
            return HashMap::new();
        };

        let mut journeys = HashMap::new();

        for line in lines {
            if line.forward_route.is_empty() && line.return_route.is_empty() {
                continue;
            }

            match line.schedule_mode {
                ScheduleMode::Auto => {
                    // Generate forward journeys
                    Self::generate_forward_journeys(&mut journeys, line, graph, day_end);

                    // Generate return journeys
                    Self::generate_return_journeys(&mut journeys, line, graph, day_end);
                }
                ScheduleMode::Manual => {
                    // Generate journeys from manual departures
                    Self::generate_manual_journeys(&mut journeys, line, graph);
                }
            }
        }

        journeys
    }

    fn generate_forward_journeys(
        journeys: &mut HashMap<uuid::Uuid, TrainJourney>,
        line: &Line,
        graph: &RailwayGraph,
        day_end: NaiveDateTime,
    ) {
        let mut departure_time = line.first_departure;
        let mut journey_count = 0;

        while departure_time <= day_end && journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::new();
            let mut segments = Vec::new();
            let mut cumulative_time = Duration::zero();

            // Add first station (source of first edge)
            if let Some(segment) = line.forward_route.first() {
                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let _ = graph.get_track_endpoints(edge_idx)
                    .and_then(|(from, _)| graph.get_station_name(from))
                    .map(|name| station_times.push((name.to_string(), departure_time, departure_time)));
            }

            // Walk the route, accumulating travel times and wait times
            for segment in &line.forward_route {
                cumulative_time += segment.duration;
                let arrival_time = departure_time + cumulative_time;

                // Add wait time to get departure time
                cumulative_time += segment.wait_time;
                let departure_from_station = departure_time + cumulative_time;

                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let Some((_, to)) = graph.get_track_endpoints(edge_idx) else {
                    continue;
                };
                let Some(name) = graph.get_station_name(to) else {
                    continue;
                };
                station_times.push((name.to_string(), arrival_time, departure_from_station));

                // Add segment info
                segments.push(JourneySegment {
                    edge_index: segment.edge_index,
                    track_index: segment.track_index,
                    origin_platform: segment.origin_platform,
                    destination_platform: segment.destination_platform,
                });
            }

            if station_times.len() >= 2 {
                let id = uuid::Uuid::new_v4();
                journeys.insert(id, TrainJourney {
                    id,
                    line_id: line.id.clone(),
                    departure_time,
                    station_times,
                    segments,
                    color: line.color.clone(),
                    thickness: line.thickness,
                });
                journey_count += 1;
            }

            departure_time += line.frequency;

            if departure_time.hour() > GENERATION_END_HOUR {
                break;
            }
        }
    }

    fn generate_manual_journeys(
        journeys: &mut HashMap<uuid::Uuid, TrainJourney>,
        line: &Line,
        graph: &RailwayGraph,
    ) {
        for manual_dep in &line.manual_departures {
            let from_idx = manual_dep.from_station;
            let to_idx = manual_dep.to_station;

            // Try forward route first
            if let Some(journey) = Self::generate_manual_journey_for_route(
                &line.forward_route,
                line,
                graph,
                manual_dep.time,
                from_idx,
                to_idx,
            ) {
                journeys.insert(journey.id, journey);
                continue;
            }

            // Try return route if forward didn't work
            if let Some(journey) = Self::generate_manual_journey_for_route(
                &line.return_route,
                line,
                graph,
                manual_dep.time,
                from_idx,
                to_idx,
            ) {
                journeys.insert(journey.id, journey);
            }
        }
    }

    fn generate_manual_journey_for_route(
        route: &[crate::models::RouteSegment],
        line: &Line,
        graph: &RailwayGraph,
        departure_time: NaiveDateTime,
        from_idx: petgraph::graph::NodeIndex,
        to_idx: petgraph::graph::NodeIndex,
    ) -> Option<TrainJourney> {
        // Build list of stations along this route
        let mut route_stations = Vec::new();

        // Add first station
        if let Some(segment) = route.first() {
            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
            if let Some((from, _)) = graph.get_track_endpoints(edge_idx) {
                route_stations.push(from);
            }
        }

        // Add all target stations from route segments
        for segment in route {
            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
            if let Some((_, to)) = graph.get_track_endpoints(edge_idx) {
                route_stations.push(to);
            }
        }

        // Find positions of from and to stations
        let from_pos = route_stations.iter().position(|&idx| idx == from_idx)?;
        let to_pos = route_stations.iter().position(|&idx| idx == to_idx)?;

        // Check if this is a valid path (from before to in this route)
        if from_pos >= to_pos {
            return None;
        }

        // Build station times for this journey segment
        let mut station_times = Vec::new();
        let mut segments = Vec::new();

        // Get from station name for display
        let from_name = graph.get_station_name(from_idx)?;
        station_times.push((from_name.to_string(), departure_time, departure_time));

        let mut cumulative_time = Duration::zero();
        for i in from_pos..to_pos {
            cumulative_time += route[i].duration;
            let arrival_time = departure_time + cumulative_time;

            // Add wait time to get departure time
            cumulative_time += route[i].wait_time;
            let departure_from_station = departure_time + cumulative_time;

            let name = graph.get_station_name(route_stations[i + 1])?;
            station_times.push((name.to_string(), arrival_time, departure_from_station));

            // Add segment info
            segments.push(JourneySegment {
                edge_index: route[i].edge_index,
                track_index: route[i].track_index,
                origin_platform: route[i].origin_platform,
                destination_platform: route[i].destination_platform,
            });
        }

        if station_times.len() >= 2 {
            Some(TrainJourney {
                id: uuid::Uuid::new_v4(),
                line_id: line.id.clone(),
                departure_time,
                station_times,
                segments,
                color: line.color.clone(),
                thickness: line.thickness,
            })
        } else {
            None
        }
    }

    fn generate_return_journeys(
        journeys: &mut HashMap<uuid::Uuid, TrainJourney>,
        line: &Line,
        graph: &RailwayGraph,
        day_end: NaiveDateTime,
    ) {
        if line.return_route.is_empty() {
            return;
        }

        let mut return_departure_time = line.return_first_departure;
        let mut return_journey_count = 0;

        while return_departure_time <= day_end && return_journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::new();
            let mut segments = Vec::new();
            let mut cumulative_time = Duration::zero();

            // Add first station (destination of first edge in return route, since we travel backwards)
            if let Some(segment) = line.return_route.first() {
                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let _ = graph.get_track_endpoints(edge_idx)
                    .and_then(|(_, to)| graph.get_station_name(to))
                    .map(|name| station_times.push((name.to_string(), return_departure_time, return_departure_time)));
            }

            // Walk the return route
            for segment in &line.return_route {
                cumulative_time += segment.duration;
                let arrival_time = return_departure_time + cumulative_time;

                // Add wait time to get departure time
                cumulative_time += segment.wait_time;
                let departure_from_station = return_departure_time + cumulative_time;

                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let Some((from, _)) = graph.get_track_endpoints(edge_idx) else {
                    continue;
                };
                let Some(name) = graph.get_station_name(from) else {
                    continue;
                };
                station_times.push((name.to_string(), arrival_time, departure_from_station));

                // Add segment info
                segments.push(JourneySegment {
                    edge_index: segment.edge_index,
                    track_index: segment.track_index,
                    origin_platform: segment.origin_platform,
                    destination_platform: segment.destination_platform,
                });
            }

            if station_times.len() >= 2 {
                let id = uuid::Uuid::new_v4();
                journeys.insert(id, TrainJourney {
                    id,
                    line_id: line.id.clone(),
                    departure_time: return_departure_time,
                    station_times,
                    segments,
                    color: line.color.clone(),
                    thickness: line.thickness,
                });
                return_journey_count += 1;
            }

            return_departure_time += line.frequency;

            if return_departure_time.hour() > GENERATION_END_HOUR {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RouteSegment, RailwayGraph, Line, ScheduleMode, Track, TrackDirection};

    const TEST_COLOR: &str = "#FF0000";
    const TEST_THICKNESS: f64 = 2.0;

    fn create_test_graph() -> RailwayGraph {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("Station A".to_string());
        let idx2 = graph.add_or_get_station("Station B".to_string());
        let idx3 = graph.add_or_get_station("Station C".to_string());

        graph.add_track(idx1, idx2, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(idx2, idx3, vec![Track { direction: TrackDirection::Bidirectional }]);

        graph
    }

    fn create_test_line(graph: &RailwayGraph) -> Line {
        let idx1 = graph.get_station_index("Station A").expect("Station A exists");
        let idx2 = graph.get_station_index("Station B").expect("Station B exists");
        let idx3 = graph.get_station_index("Station C").expect("Station C exists");

        let edge1 = graph.graph.find_edge(idx1, idx2).expect("edge exists");
        let edge2 = graph.graph.find_edge(idx2, idx3).expect("edge exists");

        Line {
            id: "Test Line".to_string(),
            color: TEST_COLOR.to_string(),
            thickness: TEST_THICKNESS,
            visible: true,
            forward_route: vec![
                RouteSegment {
                    edge_index: edge1.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Duration::minutes(10),
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: edge2.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Duration::minutes(15),
                    wait_time: Duration::seconds(30),
                },
            ],
            return_route: vec![],
            first_departure: BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"),
            return_first_departure: BASE_DATE.and_hms_opt(8, 30, 0).expect("valid time"),
            frequency: Duration::hours(1),
            schedule_mode: ScheduleMode::Auto,
            manual_departures: vec![],
        }
    }

    #[test]
    fn test_journey_segment_creation() {
        let segment = JourneySegment {
            edge_index: 0,
            track_index: 1,
            origin_platform: 2,
            destination_platform: 3,
        };

        assert_eq!(segment.edge_index, 0);
        assert_eq!(segment.track_index, 1);
        assert_eq!(segment.origin_platform, 2);
        assert_eq!(segment.destination_platform, 3);
    }

    #[test]
    fn test_generate_journeys_empty_lines() {
        let graph = RailwayGraph::new();
        let lines = vec![];

        let journeys = TrainJourney::generate_journeys(&lines, &graph);

        assert_eq!(journeys.len(), 0);
    }

    #[test]
    fn test_generate_journeys_line_with_no_route() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);
        line.forward_route = vec![];
        line.return_route = vec![];

        let journeys = TrainJourney::generate_journeys(&[line], &graph);

        assert_eq!(journeys.len(), 0);
    }

    #[test]
    fn test_generate_forward_journeys() {
        let graph = create_test_graph();
        let line = create_test_line(&graph);

        let journeys = TrainJourney::generate_journeys(&[line], &graph);

        assert!(!journeys.is_empty());

        let first_journey = journeys.values().next().expect("has journey");
        assert_eq!(first_journey.line_id, "Test Line");
        assert_eq!(first_journey.color, TEST_COLOR);
        assert_eq!(first_journey.thickness, TEST_THICKNESS);
        assert_eq!(first_journey.station_times.len(), 3);
        assert_eq!(first_journey.segments.len(), 2);
        assert_eq!(first_journey.station_times[0].0, "Station A");
        assert_eq!(first_journey.station_times[1].0, "Station B");
        assert_eq!(first_journey.station_times[2].0, "Station C");
    }

    #[test]
    fn test_generate_journeys_respects_frequency() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);
        line.frequency = Duration::hours(2);

        let journeys = TrainJourney::generate_journeys(&[line], &graph);

        let mut departure_times: Vec<_> = journeys.values()
            .map(|j| j.departure_time)
            .collect();
        departure_times.sort();

        // Check that journeys are spaced by 2 hours
        for i in 1..departure_times.len() {
            let diff = departure_times[i] - departure_times[i - 1];
            assert_eq!(diff, Duration::hours(2));
        }
    }

    #[test]
    fn test_generate_journeys_stops_at_end_hour() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);
        line.first_departure = BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time");
        line.frequency = Duration::minutes(30);

        let journeys = TrainJourney::generate_journeys(&[line], &graph);

        // Should only generate journeys up to GENERATION_END_HOUR (22)
        for journey in journeys.values() {
            assert!(journey.departure_time.hour() <= GENERATION_END_HOUR);
        }
    }

    #[test]
    fn test_generate_journeys_respects_max_journeys() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);
        line.first_departure = BASE_DATE.and_hms_opt(0, 0, 0).expect("valid time");
        line.frequency = Duration::minutes(1); // Very frequent

        let journeys = TrainJourney::generate_journeys(&[line], &graph);

        assert!(journeys.len() as i32 <= MAX_JOURNEYS_PER_LINE);
    }

    #[test]
    fn test_generate_return_journeys() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);

        let idx1 = graph.get_station_index("Station A").expect("Station A exists");
        let idx2 = graph.get_station_index("Station B").expect("Station B exists");
        let idx3 = graph.get_station_index("Station C").expect("Station C exists");

        // For return journey, generate_return_journeys looks at:
        // - Starting station: destination of first edge (line 269)
        // - Next stations: source of each edge (line 283)
        // Bidirectional tracks create edges in both directions
        // Find C→B edge (if it exists) or use B→C and handle accordingly
        let edge_c_b = graph.graph.find_edge(idx3, idx2);
        let edge_b_a = graph.graph.find_edge(idx2, idx1);

        // If bidirectional edges exist, use them
        if let (Some(e1), Some(e2)) = (edge_c_b, edge_b_a) {
            line.return_route = vec![
                RouteSegment {
                    edge_index: e1.index(),
                    track_index: 0,
                    origin_platform: 1,
                    destination_platform: 1,
                    duration: Duration::minutes(15),
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: e2.index(),
                    track_index: 0,
                    origin_platform: 1,
                    destination_platform: 1,
                    duration: Duration::minutes(10),
                    wait_time: Duration::seconds(30),
                },
            ];

            let journeys = TrainJourney::generate_journeys(&[line], &graph);

            // Should have both forward and return journeys
            let return_journeys: Vec<_> = journeys.values()
                .filter(|j| j.departure_time >= BASE_DATE.and_hms_opt(8, 30, 0).expect("valid time"))
                .collect();

            assert!(!return_journeys.is_empty());

            let first_return = return_journeys[0];
            assert_eq!(first_return.station_times[0].0, "Station C");
            assert_eq!(first_return.station_times.last().unwrap().0, "Station A");
        }
    }

    #[test]
    fn test_journey_timing_calculation() {
        let graph = create_test_graph();
        let line = create_test_line(&graph);

        let journeys = TrainJourney::generate_journeys(&[line], &graph);

        // Find a forward journey starting at 8:00
        let journey = journeys.values()
            .find(|j| j.departure_time == BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"))
            .expect("has 8:00 journey");

        let start_time = BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time");

        // First station: immediate departure
        assert_eq!(journey.station_times[0].1, start_time); // arrival
        assert_eq!(journey.station_times[0].2, start_time); // departure

        // Second station: 10 minutes travel + 30 seconds wait
        let expected_arrival_b = start_time + Duration::minutes(10);
        let expected_departure_b = expected_arrival_b + Duration::seconds(30);
        assert_eq!(journey.station_times[1].1, expected_arrival_b);
        assert_eq!(journey.station_times[1].2, expected_departure_b);

        // Third station: previous + 15 minutes travel
        let expected_arrival_c = expected_departure_b + Duration::minutes(15);
        let expected_departure_c = expected_arrival_c + Duration::seconds(30);
        assert_eq!(journey.station_times[2].1, expected_arrival_c);
        assert_eq!(journey.station_times[2].2, expected_departure_c);
    }
}
