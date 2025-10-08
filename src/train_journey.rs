use crate::models::{Line, RailwayGraph, ScheduleMode};
use crate::constants::{BASE_DATE, GENERATION_END_HOUR};
use chrono::{Duration, NaiveDateTime, Timelike};

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
    pub line_id: String,
    pub departure_time: NaiveDateTime,
    pub station_times: Vec<(String, NaiveDateTime, NaiveDateTime)>, // (station_name, arrival_time, departure_time)
    pub segments: Vec<JourneySegment>, // Track and platform info for each segment
    pub color: String,
    pub thickness: f64,
}

impl TrainJourney {
    /// Generate train journeys for all lines throughout the day
    pub fn generate_journeys(lines: &[Line], graph: &RailwayGraph) -> Vec<TrainJourney> {
        let Some(day_end) = BASE_DATE.and_hms_opt(23, 59, 59) else {
            return Vec::new();
        };

        let mut journeys = Vec::new();

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
        journeys: &mut Vec<TrainJourney>,
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
                journeys.push(TrainJourney {
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
        journeys: &mut Vec<TrainJourney>,
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
                journeys.push(journey);
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
                journeys.push(journey);
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
        journeys: &mut Vec<TrainJourney>,
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

            // Add first station (source of first edge in return route)
            if let Some(segment) = line.return_route.first() {
                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let _ = graph.get_track_endpoints(edge_idx)
                    .and_then(|(from, _)| graph.get_station_name(from))
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
                journeys.push(TrainJourney {
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
