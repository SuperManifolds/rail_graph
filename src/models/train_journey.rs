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
}

impl TrainJourney {
    /// Generate train journeys for all lines throughout the day
    pub fn generate_journeys(lines: &[Line], graph: &RailwayGraph) -> Vec<TrainJourney> {
        let Some(day_end) = BASE_DATE.and_hms_opt(23, 59, 59) else {
            return Vec::new();
        };

        let mut journeys = Vec::new();

        for line in lines {
            // Get the path for this line from the graph
            let line_path = graph.get_line_path(&line.id);

            if line_path.is_empty() {
                continue;
            }

            match line.schedule_mode {
                ScheduleMode::Auto => {
                    // Generate forward journeys
                    Self::generate_forward_journeys(&mut journeys, line, &line_path, graph, day_end);

                    // Generate return journeys
                    Self::generate_return_journeys(&mut journeys, line, &line_path, graph, day_end);
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
        line_path: &[(petgraph::graph::NodeIndex, petgraph::graph::NodeIndex, Duration)],
        graph: &RailwayGraph,
        day_end: NaiveDateTime,
    ) {
        let mut departure_time = line.first_departure;
        let mut journey_count = 0;

        while departure_time <= day_end && journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::new();
            let mut cumulative_time = Duration::zero();

            // Add first station (source of first edge)
            if let Some((first_from, _, _)) = line_path.first() {
                if let Some(name) = graph.get_station_name(*first_from) {
                    station_times.push((name.to_string(), departure_time));
                }
            }

            // Walk the path, accumulating travel times
            for (_from, to, travel_time) in line_path {
                cumulative_time = cumulative_time + *travel_time;
                let arrival_time = departure_time + cumulative_time;

                if let Some(name) = graph.get_station_name(*to) {
                    station_times.push((name.to_string(), arrival_time));
                }
            }

            if station_times.len() >= 2 {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time,
                    station_times,
                    color: line.color.clone(),
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

            // Get the full line path
            let line_path = graph.get_line_path(&line.id);
            if line_path.is_empty() {
                continue;
            }

            // Find positions of from and to stations in the path
            let from_pos = line_path.iter().position(|(src, _, _)| *src == from_idx)
                .or_else(|| line_path.iter().position(|(_, tgt, _)| *tgt == from_idx));
            let to_pos = line_path.iter().position(|(_, tgt, _)| *tgt == to_idx)
                .or_else(|| line_path.iter().position(|(src, _, _)| *src == to_idx));

            let (Some(from_pos), Some(to_pos)) = (from_pos, to_pos) else {
                continue;
            };

            // Determine direction and extract journey segment
            let is_forward = from_pos < to_pos;
            let (start_pos, end_pos) = if is_forward {
                (from_pos, to_pos)
            } else {
                (to_pos, from_pos)
            };

            // Build station times for this journey segment
            let mut station_times = Vec::new();
            let departure_time = manual_dep.time;
            let mut cumulative_time = Duration::zero();

            // Add starting station
            let start_node = if is_forward {
                line_path[start_pos].0
            } else {
                line_path[end_pos].1
            };
            if let Some(name) = graph.get_station_name(start_node) {
                station_times.push((name.to_string(), departure_time));
            }

            // Walk the path segment
            for i in start_pos..=end_pos {
                cumulative_time = cumulative_time + line_path[i].2;
                let arrival_time = departure_time + cumulative_time;

                if let Some(name) = graph.get_station_name(line_path[i].1) {
                    station_times.push((name.to_string(), arrival_time));
                }
            }

            // If going backwards, reverse the station times
            if !is_forward {
                station_times.reverse();
            }

            if station_times.len() >= 2 {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time,
                    station_times,
                    color: line.color.clone(),
                });
            }
        }
    }

    fn generate_return_journeys(
        journeys: &mut Vec<TrainJourney>,
        line: &Line,
        line_path: &[(petgraph::graph::NodeIndex, petgraph::graph::NodeIndex, Duration)],
        graph: &RailwayGraph,
        day_end: NaiveDateTime,
    ) {
        // Build reverse path
        let return_path: Vec<_> = line_path.iter()
            .rev()
            .map(|(from, to, travel_time)| (*to, *from, *travel_time))
            .collect();

        if return_path.is_empty() {
            return;
        }

        let mut return_departure_time = line.return_first_departure;
        let mut return_journey_count = 0;

        while return_departure_time <= day_end && return_journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::new();
            let mut cumulative_time = Duration::zero();

            // Add first station (source of first edge in return path)
            if let Some((first_from, _, _)) = return_path.first() {
                if let Some(name) = graph.get_station_name(*first_from) {
                    station_times.push((name.to_string(), return_departure_time));
                }
            }

            // Walk the return path
            for (_from, to, travel_time) in &return_path {
                cumulative_time = cumulative_time + *travel_time;
                let arrival_time = return_departure_time + cumulative_time;

                if let Some(name) = graph.get_station_name(*to) {
                    station_times.push((name.to_string(), arrival_time));
                }
            }

            if station_times.len() >= 2 {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time: return_departure_time,
                    station_times,
                    color: line.color.clone(),
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
