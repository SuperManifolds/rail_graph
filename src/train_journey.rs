use crate::models::{Line, RailwayGraph, ScheduleMode, Stations, Tracks, DaysOfWeek};
use crate::constants::{BASE_DATE, GENERATION_END_HOUR};
use chrono::{Duration, NaiveDateTime, Timelike, Weekday};
use std::collections::HashMap;

const MAX_JOURNEYS_PER_LINE: usize = 100; // Limit to prevent performance issues

/// Convert `chrono::Weekday` to our `DaysOfWeek` bitflag
fn weekday_to_days_of_week(weekday: Weekday) -> DaysOfWeek {
    match weekday {
        Weekday::Mon => DaysOfWeek::MONDAY,
        Weekday::Tue => DaysOfWeek::TUESDAY,
        Weekday::Wed => DaysOfWeek::WEDNESDAY,
        Weekday::Thu => DaysOfWeek::THURSDAY,
        Weekday::Fri => DaysOfWeek::FRIDAY,
        Weekday::Sat => DaysOfWeek::SATURDAY,
        Weekday::Sun => DaysOfWeek::SUNDAY,
    }
}

/// Convert a `NaiveDateTime` to a specific date while preserving time components
fn time_on_date(datetime: NaiveDateTime, date: chrono::NaiveDate) -> Option<NaiveDateTime> {
    date.and_hms_opt(datetime.hour(), datetime.minute(), datetime.second())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JourneySegment {
    pub edge_index: usize,
    pub track_index: usize,
    pub origin_platform: usize,
    pub destination_platform: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainJourney {
    pub id: uuid::Uuid,
    pub line_id: String,
    pub departure_time: NaiveDateTime,
    pub station_times: Vec<(petgraph::stable_graph::NodeIndex, NaiveDateTime, NaiveDateTime)>, // (station_node, arrival_time, departure_time)
    pub segments: Vec<JourneySegment>, // Track and platform info for each segment
    pub color: String,
    pub thickness: f64,
}

impl TrainJourney {
    /// Generate train journeys for all lines throughout the day
    ///
    /// # Arguments
    /// * `lines` - The lines to generate journeys for
    /// * `graph` - The railway graph
    /// * `selected_day` - Optional day of week filter. If provided, only generates journeys for lines operating on that day
    #[must_use]
    pub fn generate_journeys(lines: &[Line], graph: &RailwayGraph, selected_day: Option<Weekday>) -> HashMap<uuid::Uuid, TrainJourney> {
        #[cfg(target_arch = "wasm32")]
        let (performance, start) = {
            let window = web_sys::window().expect("should have a window");
            let performance = window.performance().expect("should have performance");
            let start = performance.now();
            (performance, start)
        };

        // Build station name → index map for fast lookups
        let stations = graph.get_all_stations_ordered();
        let station_map: HashMap<&str, usize> = stations
            .iter()
            .enumerate()
            .map(|(idx, (_, station))| (station.name.as_str(), idx))
            .collect();

        let mut journeys = HashMap::new();

        // Determine which days to simulate
        let days_to_simulate: Vec<(Weekday, i64)> = if let Some(day) = selected_day {
            // Only simulate the selected day
            vec![(day, 0)]
        } else {
            // Simulate all 7 days of the week
            vec![
                (Weekday::Mon, 0),
                (Weekday::Tue, 1),
                (Weekday::Wed, 2),
                (Weekday::Thu, 3),
                (Weekday::Fri, 4),
                (Weekday::Sat, 5),
                (Weekday::Sun, 6),
            ]
        };

        for (weekday, day_offset) in days_to_simulate {
            let day_filter = weekday_to_days_of_week(weekday);
            let current_date = BASE_DATE + Duration::days(day_offset);

            let Some(day_end) = current_date.and_hms_opt(23, 59, 59) else {
                continue;
            };

            for line in lines {
                if line.forward_route.is_empty() && line.return_route.is_empty() {
                    continue;
                }

                // Filter by day of week
                if !line.days_of_week.contains(day_filter) {
                    continue;
                }

                match line.schedule_mode {
                    ScheduleMode::Auto => {
                        // Generate forward journeys
                        Self::generate_forward_journeys(&mut journeys, line, graph, current_date, day_end, &station_map);

                        // Generate return journeys
                        Self::generate_return_journeys(&mut journeys, line, graph, current_date, day_end, &station_map);
                    }
                    ScheduleMode::Manual => {
                        // Generate journeys from manual departures
                        Self::generate_manual_journeys(&mut journeys, line, graph, current_date, day_filter, &station_map);
                    }
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let duration = performance.now() - start;
            web_sys::console::log_1(&format!("Journey generation took: {:.2}ms ({} journeys)", duration, journeys.len()).into());
        }

        journeys
    }

    fn determine_start_node(
        first_segment: &crate::models::RouteSegment,
        second_segment: Option<&crate::models::RouteSegment>,
        graph: &RailwayGraph,
    ) -> Option<petgraph::stable_graph::NodeIndex> {
        let first_edge_idx = petgraph::graph::EdgeIndex::new(first_segment.edge_index);
        let (from1, to1) = graph.get_track_endpoints(first_edge_idx)?;

        let Some(second_seg) = second_segment else {
            return Some(from1);
        };

        let second_edge_idx = petgraph::graph::EdgeIndex::new(second_seg.edge_index);
        let Some((from2, to2)) = graph.get_track_endpoints(second_edge_idx) else {
            return Some(from1);
        };

        // Start from the endpoint NOT shared with the second edge
        if from1 == from2 || from1 == to2 {
            Some(to1)
        } else {
            Some(from1)
        }
    }

    fn build_route_nodes(
        route: &[crate::models::RouteSegment],
        graph: &RailwayGraph,
    ) -> Vec<Option<petgraph::stable_graph::NodeIndex>> {
        let mut route_nodes: Vec<Option<petgraph::stable_graph::NodeIndex>> = Vec::with_capacity(route.len() + 1);

        // Determine the starting node
        if let Some(first_segment) = route.first() {
            let start_node = Self::determine_start_node(first_segment, route.get(1), graph);
            route_nodes.push(start_node);
        }

        // Build remaining nodes by following connections
        for segment in route {
            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
            let Some(endpoints) = graph.get_track_endpoints(edge_idx) else {
                route_nodes.push(None);
                continue;
            };

            let Some(Some(prev_node)) = route_nodes.last() else {
                route_nodes.push(Some(endpoints.1));
                continue;
            };

            let next_node = if endpoints.0 == *prev_node {
                endpoints.1
            } else {
                endpoints.0
            };
            route_nodes.push(Some(next_node));
        }

        route_nodes
    }

    fn generate_forward_journeys(
        journeys: &mut HashMap<uuid::Uuid, TrainJourney>,
        line: &Line,
        graph: &RailwayGraph,
        current_date: chrono::NaiveDate,
        day_end: NaiveDateTime,
        _station_map: &HashMap<&str, usize>,
    ) {
        if line.forward_route.is_empty() {
            return;
        }

        // Convert the line's first_departure time to the current date
        let Some(mut departure_time) = time_on_date(line.first_departure, current_date) else {
            return;
        };

        // Pre-compute route node indices
        let route_nodes = Self::build_route_nodes(&line.forward_route, graph);


        let mut journey_count = 0;
        let line_id = line.id.clone();
        let color = line.color.clone();
        let thickness = line.thickness;

        while departure_time <= day_end && journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::with_capacity(route_nodes.len());
            let mut segments = Vec::with_capacity(line.forward_route.len());
            let mut cumulative_time = Duration::zero();

            // Add first node (station or junction)
            if let Some(node_idx) = route_nodes[0] {
                station_times.push((node_idx, departure_time, departure_time));
            }

            // Walk the route, accumulating travel times and wait times
            for (i, segment) in line.forward_route.iter().enumerate() {
                cumulative_time += segment.duration;
                let arrival_time = departure_time + cumulative_time;

                // Add wait time to get departure time
                cumulative_time += segment.wait_time;
                let departure_from_station = departure_time + cumulative_time;

                // Add all nodes (stations and junctions)
                if let Some(node_idx) = route_nodes[i + 1] {
                    station_times.push((node_idx, arrival_time, departure_from_station));
                }

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
                    line_id: line_id.clone(),
                    departure_time,
                    station_times,
                    segments,
                    color: color.clone(),
                    thickness,
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
        current_date: chrono::NaiveDate,
        day_filter: DaysOfWeek,
        station_map: &HashMap<&str, usize>,
    ) {
        for manual_dep in &line.manual_departures {
            // Filter by day of week
            if !manual_dep.days_of_week.contains(day_filter) {
                continue;
            }

            // Convert the manual departure time to the current date
            let Some(departure_time) = time_on_date(manual_dep.time, current_date) else {
                continue;
            };

            let from_idx = manual_dep.from_station;
            let to_idx = manual_dep.to_station;

            // Try forward route first
            if let Some(journey) = Self::generate_manual_journey_for_route(
                &line.forward_route,
                line,
                graph,
                departure_time,
                from_idx,
                to_idx,
                station_map,
            ) {
                journeys.insert(journey.id, journey);
                continue;
            }

            // Try return route if forward didn't work
            if let Some(journey) = Self::generate_manual_journey_for_route(
                &line.return_route,
                line,
                graph,
                departure_time,
                from_idx,
                to_idx,
                station_map,
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
        _station_map: &HashMap<&str, usize>,
    ) -> Option<TrainJourney> {
        

        // Build list of node indices along this route
        let mut route_nodes = Vec::new();

        // Add first node
        if let Some(segment) = route.first() {
            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
            if let Some((from, _)) = graph.get_track_endpoints(edge_idx) {
                route_nodes.push(from);
            }
        }

        // Add all target nodes from route segments
        for segment in route {
            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
            if let Some((_, to)) = graph.get_track_endpoints(edge_idx) {
                route_nodes.push(to);
            }
        }

        // Find positions of from and to stations
        let from_pos = route_nodes.iter().position(|&idx| idx == from_idx)?;
        let to_pos = route_nodes.iter().position(|&idx| idx == to_idx)?;

        // Check if this is a valid path (from before to in this route)
        if from_pos >= to_pos {
            return None;
        }

        // Build station times for this journey segment
        let mut station_times = Vec::new();
        let mut segments = Vec::new();

        // Add first node (station or junction)
        station_times.push((from_idx, departure_time, departure_time));

        let mut cumulative_time = Duration::zero();
        for i in from_pos..to_pos {
            cumulative_time += route[i].duration;
            let arrival_time = departure_time + cumulative_time;

            // Add wait time to get departure time
            cumulative_time += route[i].wait_time;
            let departure_from_station = departure_time + cumulative_time;

            // Add all nodes (stations and junctions)
            let node_idx = route_nodes[i + 1];
            station_times.push((node_idx, arrival_time, departure_from_station));

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
        current_date: chrono::NaiveDate,
        day_end: NaiveDateTime,
        _station_map: &HashMap<&str, usize>,
    ) {
        if line.return_route.is_empty() {
            return;
        }

        // Convert the line's return_first_departure time to the current date
        let Some(mut return_departure_time) = time_on_date(line.return_first_departure, current_date) else {
            return;
        };

        // Pre-compute route node indices
        let route_nodes = Self::build_route_nodes(&line.return_route, graph);


        let mut return_journey_count = 0;
        let line_id = line.id.clone();
        let color = line.color.clone();
        let thickness = line.thickness;

        while return_departure_time <= day_end && return_journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::with_capacity(route_nodes.len());
            let mut segments = Vec::with_capacity(line.return_route.len());
            let mut cumulative_time = Duration::zero();

            // Add first node (station or junction)
            if let Some(node_idx) = route_nodes[0] {
                station_times.push((node_idx, return_departure_time, return_departure_time));
            }

            // Walk the return route
            for (i, segment) in line.return_route.iter().enumerate() {
                cumulative_time += segment.duration;
                let arrival_time = return_departure_time + cumulative_time;

                // Add wait time to get departure time
                cumulative_time += segment.wait_time;
                let departure_from_station = return_departure_time + cumulative_time;

                // Add all nodes (stations and junctions)
                if let Some(node_idx) = route_nodes[i + 1] {
                    station_times.push((node_idx, arrival_time, departure_from_station));
                }

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
                    line_id: line_id.clone(),
                    departure_time: return_departure_time,
                    station_times,
                    segments,
                    color: color.clone(),
                    thickness,
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
            days_of_week: crate::models::DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            sync_routes: true,
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

        let journeys = TrainJourney::generate_journeys(&lines, &graph, None);

        assert_eq!(journeys.len(), 0);
    }

    #[test]
    fn test_generate_journeys_line_with_no_route() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);
        line.forward_route = vec![];
        line.return_route = vec![];

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

        assert_eq!(journeys.len(), 0);
    }

    #[test]
    fn test_generate_forward_journeys() {
        let graph = create_test_graph();
        let line = create_test_line(&graph);

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

        assert!(!journeys.is_empty());

        let first_journey = journeys.values().next().expect("has journey");
        assert_eq!(first_journey.line_id, "Test Line");
        assert_eq!(first_journey.color, TEST_COLOR);
        assert_eq!(first_journey.thickness, TEST_THICKNESS);
        assert_eq!(first_journey.station_times.len(), 3);
        assert_eq!(first_journey.segments.len(), 2);

        // Verify stations by looking up their names
        let idx1 = graph.get_station_index("Station A").expect("Station A exists");
        let idx2 = graph.get_station_index("Station B").expect("Station B exists");
        let idx3 = graph.get_station_index("Station C").expect("Station C exists");
        assert_eq!(first_journey.station_times[0].0, idx1);
        assert_eq!(first_journey.station_times[1].0, idx2);
        assert_eq!(first_journey.station_times[2].0, idx3);
    }

    #[test]
    fn test_generate_journeys_respects_frequency() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);
        line.frequency = Duration::hours(2);

        // Test with a single day filter to check frequency within one day
        let journeys = TrainJourney::generate_journeys(&[line], &graph, Some(Weekday::Mon));

        let mut departure_times: Vec<_> = journeys.values()
            .map(|j| j.departure_time)
            .collect();
        departure_times.sort();

        // Check that journeys are spaced by 2 hours within the same day
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

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

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

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

        // With 7 days, we should have at most MAX_JOURNEYS_PER_LINE per day
        assert!(journeys.len() <= MAX_JOURNEYS_PER_LINE * 7);
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

            let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

            // Should have both forward and return journeys
            let return_journeys: Vec<_> = journeys.values()
                .filter(|j| j.departure_time >= BASE_DATE.and_hms_opt(8, 30, 0).expect("valid time"))
                .collect();

            assert!(!return_journeys.is_empty());

            let first_return = return_journeys[0];
            assert_eq!(first_return.station_times[0].0, idx3);
            assert_eq!(first_return.station_times.last().expect("has stations").0, idx1);
        }
    }

    #[test]
    fn test_journey_timing_calculation() {
        let graph = create_test_graph();
        let line = create_test_line(&graph);

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

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

    #[test]
    fn test_weekday_to_days_of_week_conversion() {
        assert_eq!(weekday_to_days_of_week(Weekday::Mon), DaysOfWeek::MONDAY);
        assert_eq!(weekday_to_days_of_week(Weekday::Tue), DaysOfWeek::TUESDAY);
        assert_eq!(weekday_to_days_of_week(Weekday::Wed), DaysOfWeek::WEDNESDAY);
        assert_eq!(weekday_to_days_of_week(Weekday::Thu), DaysOfWeek::THURSDAY);
        assert_eq!(weekday_to_days_of_week(Weekday::Fri), DaysOfWeek::FRIDAY);
        assert_eq!(weekday_to_days_of_week(Weekday::Sat), DaysOfWeek::SATURDAY);
        assert_eq!(weekday_to_days_of_week(Weekday::Sun), DaysOfWeek::SUNDAY);
    }

    #[test]
    fn test_generate_journeys_filters_by_day() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);

        // Line only operates on weekdays
        line.days_of_week = DaysOfWeek::WEEKDAYS;

        // Generate for Monday - should have journeys
        let monday_journeys = TrainJourney::generate_journeys(std::slice::from_ref(&line), &graph, Some(Weekday::Mon));
        assert!(!monday_journeys.is_empty());

        // Generate for Saturday - should have no journeys
        let saturday_journeys = TrainJourney::generate_journeys(std::slice::from_ref(&line), &graph, Some(Weekday::Sat));
        assert!(saturday_journeys.is_empty());
    }

    #[test]
    fn test_generate_journeys_seven_days() {
        let graph = create_test_graph();
        let line = create_test_line(&graph);

        // Generate for all 7 days
        let all_journeys = TrainJourney::generate_journeys(std::slice::from_ref(&line), &graph, None);

        // Generate for a single day
        let single_day_journeys = TrainJourney::generate_journeys(std::slice::from_ref(&line), &graph, Some(Weekday::Mon));

        // Should have approximately 7x more journeys when generating for all days
        // (approximately because of daily cutoff times)
        assert!(all_journeys.len() >= single_day_journeys.len() * 6);
        assert!(all_journeys.len() <= single_day_journeys.len() * 8);
    }

    #[test]
    fn test_manual_departure_respects_days_of_week() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);

        let idx1 = graph.get_station_index("Station A").expect("Station A exists");
        let idx2 = graph.get_station_index("Station B").expect("Station B exists");

        line.schedule_mode = ScheduleMode::Manual;
        line.manual_departures = vec![
            crate::models::ManualDeparture {
                time: BASE_DATE.and_hms_opt(10, 0, 0).expect("valid time"),
                from_station: idx1,
                to_station: idx2,
                days_of_week: DaysOfWeek::MONDAY | DaysOfWeek::WEDNESDAY | DaysOfWeek::FRIDAY,
            },
        ];

        // Generate for Monday - should have the departure
        let monday_journeys = TrainJourney::generate_journeys(std::slice::from_ref(&line), &graph, Some(Weekday::Mon));
        assert_eq!(monday_journeys.len(), 1);

        // Generate for Tuesday - should not have the departure
        let tuesday_journeys = TrainJourney::generate_journeys(std::slice::from_ref(&line), &graph, Some(Weekday::Tue));
        assert_eq!(tuesday_journeys.len(), 0);
    }

    #[test]
    fn test_journey_skips_junctions() {
        use crate::models::{Junction, Junctions};

        let mut graph = RailwayGraph::new();

        // Create network: Station A -> Junction -> Station B
        let idx_a = graph.add_or_get_station("Station A".to_string());
        let junction = Junction {
            name: Some("Junction 1".to_string()),
            position: Some((50.0, 50.0)),
            routing_rules: vec![],
        };
        let idx_junction = graph.add_junction(junction);
        let idx_b = graph.add_or_get_station("Station B".to_string());

        let edge1 = graph.add_track(idx_a, idx_junction, vec![Track { direction: TrackDirection::Bidirectional }]);
        let edge2 = graph.add_track(idx_junction, idx_b, vec![Track { direction: TrackDirection::Bidirectional }]);

        let line = Line {
            id: "Test Line with Junction".to_string(),
            color: TEST_COLOR.to_string(),
            thickness: TEST_THICKNESS,
            visible: true,
            forward_route: vec![
                RouteSegment {
                    edge_index: edge1.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Duration::minutes(5),
                    wait_time: Duration::seconds(0), // No wait at junction
                },
                RouteSegment {
                    edge_index: edge2.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Duration::minutes(5),
                    wait_time: Duration::seconds(30),
                },
            ],
            return_route: vec![],
            first_departure: BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"),
            return_first_departure: BASE_DATE.and_hms_opt(8, 30, 0).expect("valid time"),
            frequency: Duration::hours(1),
            schedule_mode: ScheduleMode::Auto,
            days_of_week: crate::models::DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            sync_routes: true,
        };

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

        assert!(!journeys.is_empty());

        // Find the 8:00 departure specifically
        let journey = journeys.values()
            .find(|j| j.departure_time == BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"))
            .expect("has 8:00 journey");

        // Journey should have 3 nodes (A, junction, and B)
        assert_eq!(journey.station_times.len(), 3);
        assert_eq!(journey.station_times[0].0, idx_a);
        assert_eq!(journey.station_times[1].0, idx_junction);
        assert_eq!(journey.station_times[2].0, idx_b);

        // But it should still have 2 segments (A->Junction, Junction->B)
        assert_eq!(journey.segments.len(), 2);

        // Timing should account for travel through junction without stop
        // Travel: 5 min to junction + 0 wait + 5 min to B = 10 min total
        // Then 30 sec wait at B
        let start_time = BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time");

        // Check junction arrival (5 min from start)
        let expected_arrival_junction = start_time + Duration::minutes(5);
        assert_eq!(journey.station_times[1].1, expected_arrival_junction);
        assert_eq!(journey.station_times[1].2, expected_arrival_junction); // No wait at junction

        // Check Station B arrival (10 min from start)
        let expected_arrival_b = start_time + Duration::minutes(10);
        let expected_departure_b = expected_arrival_b + Duration::seconds(30);
        assert_eq!(journey.station_times[2].1, expected_arrival_b);
        assert_eq!(journey.station_times[2].2, expected_departure_b);
    }
}
