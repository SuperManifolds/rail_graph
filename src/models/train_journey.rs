use crate::models::{Line, RailwayGraph, ScheduleMode};
use crate::constants::{BASE_DATE, GENERATION_END_HOUR};
use chrono::{Duration, NaiveDateTime, Timelike};

const MAX_JOURNEYS_PER_LINE: i32 = 100; // Limit to prevent performance issues

#[derive(Debug, Clone)]
pub struct TrainJourney {
    pub line_id: String,
    pub departure_time: NaiveDateTime,
    pub station_times: Vec<(String, NaiveDateTime)>, // (station_name, arrival_time)
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
            if line.route.is_empty() {
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
            let mut cumulative_time = Duration::zero();

            // Add first station (source of first edge)
            if let Some(segment) = line.route.first() {
                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let Some((from, _)) = graph.get_track_endpoints(edge_idx) else {
                    continue;
                };
                let Some(name) = graph.get_station_name(from) else {
                    continue;
                };
                station_times.push((name.to_string(), departure_time));
            }

            // Walk the route, accumulating travel times
            for segment in &line.route {
                cumulative_time += segment.duration;
                let arrival_time = departure_time + cumulative_time;

                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let Some((_, to)) = graph.get_track_endpoints(edge_idx) else {
                    continue;
                };
                let Some(name) = graph.get_station_name(to) else {
                    continue;
                };
                station_times.push((name.to_string(), arrival_time));
            }

            if station_times.len() >= 2 {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time,
                    station_times,
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
            // Get NodeIndex for from and to stations
            let Some(from_idx) = graph.get_station_index(&manual_dep.from_station) else {
                continue;
            };
            let Some(to_idx) = graph.get_station_index(&manual_dep.to_station) else {
                continue;
            };

            // Build list of stations along the route to find positions
            let mut route_stations = Vec::new();

            // Add first station
            if let Some(segment) = line.route.first() {
                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                if let Some((from, _)) = graph.get_track_endpoints(edge_idx) {
                    route_stations.push(from);
                }
            }

            // Add all target stations from route segments
            for segment in &line.route {
                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                if let Some((_, to)) = graph.get_track_endpoints(edge_idx) {
                    route_stations.push(to);
                }
            }

            // Find positions of from and to stations
            let from_pos = route_stations.iter().position(|&idx| idx == from_idx);
            let to_pos = route_stations.iter().position(|&idx| idx == to_idx);

            let (Some(from_pos), Some(to_pos)) = (from_pos, to_pos) else {
                continue;
            };

            // Determine direction
            let is_forward = from_pos < to_pos;
            let (start_pos, end_pos) = if is_forward {
                (from_pos, to_pos)
            } else {
                (to_pos, from_pos)
            };

            // Build station times for this journey segment
            let mut station_times = Vec::new();
            station_times.push((manual_dep.from_station.clone(), manual_dep.time));

            let indices: Vec<usize> = if is_forward {
                (start_pos..end_pos).collect()
            } else {
                (start_pos..end_pos).rev().collect()
            };

            let mut cumulative_time = Duration::zero();
            for i in indices {
                cumulative_time += line.route[i].duration;
                let arrival_time = manual_dep.time + cumulative_time;

                let station_idx = if is_forward { i + 1 } else { i };
                let Some(name) = graph.get_station_name(route_stations[station_idx]) else {
                    continue;
                };
                station_times.push((name.to_string(), arrival_time));
            }

            if station_times.len() >= 2 {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time: manual_dep.time,
                    station_times,
                    color: line.color.clone(),
                    thickness: line.thickness,
                });
            }
        }
    }

    fn generate_return_journeys(
        journeys: &mut Vec<TrainJourney>,
        line: &Line,
        graph: &RailwayGraph,
        day_end: NaiveDateTime,
    ) {
        if line.route.is_empty() {
            return;
        }

        let mut return_departure_time = line.return_first_departure;
        let mut return_journey_count = 0;

        while return_departure_time <= day_end && return_journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::new();
            let mut cumulative_time = Duration::zero();

            // Add first station (target of last edge in forward route)
            if let Some(segment) = line.route.last() {
                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let Some((_, to)) = graph.get_track_endpoints(edge_idx) else {
                    continue;
                };
                let Some(name) = graph.get_station_name(to) else {
                    continue;
                };
                station_times.push((name.to_string(), return_departure_time));
            }

            // Walk the route in reverse
            for segment in line.route.iter().rev() {
                cumulative_time += segment.duration;
                let arrival_time = return_departure_time + cumulative_time;

                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                let Some((from, _)) = graph.get_track_endpoints(edge_idx) else {
                    continue;
                };
                let Some(name) = graph.get_station_name(from) else {
                    continue;
                };
                station_times.push((name.to_string(), arrival_time));
            }

            if station_times.len() >= 2 {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time: return_departure_time,
                    station_times,
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
