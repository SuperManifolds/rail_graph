use crate::models::{Line, RailwayGraph, ScheduleMode, Tracks, DaysOfWeek};
use crate::constants::BASE_DATE;
use chrono::{Duration, NaiveDateTime, Timelike, Weekday};
use std::collections::HashMap;

const MAX_JOURNEYS_PER_LINE: usize = 100; // Limit to prevent performance issues

/// Generate a train number from a format string
/// Supports: {line} for line ID, {seq:04} for sequence number with padding
fn generate_train_number(format: &str, line_id: &str, sequence: usize) -> String {
    format
        .replace("{line}", line_id)
        .replace("{seq:04}", &format!("{sequence:04}"))
        .replace("{seq:03}", &format!("{sequence:03}"))
        .replace("{seq:02}", &format!("{sequence:02}"))
        .replace("{seq}", &sequence.to_string())
}

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
    pub line_id: uuid::Uuid,
    pub train_number: String,
    pub departure_time: NaiveDateTime,
    pub station_times: Vec<(petgraph::stable_graph::NodeIndex, NaiveDateTime, NaiveDateTime)>, // (station_node, arrival_time, departure_time)
    pub segments: Vec<JourneySegment>, // Track and platform info for each segment
    pub color: String,
    pub thickness: f64,
    pub route_start_node: Option<petgraph::stable_graph::NodeIndex>, // First node of the complete route
    pub route_end_node: Option<petgraph::stable_graph::NodeIndex>, // Last node of the complete route
}

impl TrainJourney {
    /// Process segments without duration (fallback for missing durations)
    #[allow(clippy::too_many_arguments)]
    fn process_segments_without_duration(
        segments_without_duration: &[usize],
        route: &[crate::models::RouteSegment],
        route_nodes: &[Option<petgraph::stable_graph::NodeIndex>],
        graph: &RailwayGraph,
        departure_time: NaiveDateTime,
        cumulative_time: &mut Duration,
        station_times: &mut Vec<(petgraph::stable_graph::NodeIndex, NaiveDateTime, NaiveDateTime)>,
        segments: &mut Vec<JourneySegment>,
    ) {
        for &seg_idx in segments_without_duration {
            let seg = &route[seg_idx];
            let arrival_time = departure_time + *cumulative_time;

            // Only add wait time if the destination node is not a junction
            let is_junction = route_nodes.get(seg_idx + 1)
                .and_then(|&node_idx| node_idx)
                .and_then(|node_idx| graph.graph.node_weight(node_idx))
                .is_some_and(|node| node.as_junction().is_some());

            if !is_junction {
                *cumulative_time += seg.wait_time;
            }
            let departure_from_station = departure_time + *cumulative_time;

            if let Some(node_idx) = route_nodes[seg_idx + 1] {
                station_times.push((node_idx, arrival_time, departure_from_station));

                segments.push(JourneySegment {
                    edge_index: seg.edge_index,
                    track_index: seg.track_index,
                    origin_platform: seg.origin_platform,
                    destination_platform: seg.destination_platform,
                });
            }
        }
    }

    /// Find how many consecutive segments have no duration starting from index+1
    fn count_segments_without_duration(route: &[crate::models::RouteSegment], start_index: usize) -> Vec<usize> {
        let mut segments_to_cover = vec![start_index];
        let mut j = start_index + 1;
        while j < route.len() && route[j].duration.is_none() {
            segments_to_cover.push(j);
            j += 1;
        }
        segments_to_cover
    }

    fn count_segments_from_duration_list(durations: &[Option<Duration>], start_index: usize) -> Vec<usize> {
        let mut segments_to_cover = vec![start_index];
        let mut j = start_index + 1;
        while j < durations.len() && durations[j].is_none() {
            segments_to_cover.push(j);
            j += 1;
        }
        segments_to_cover
    }

    /// Build return route duration map from forward route, mirroring inheritance pattern
    fn build_synced_return_durations(
        forward_route: &[crate::models::RouteSegment],
        return_route_len: usize,
    ) -> Vec<Option<Duration>> {
        // If routes have mismatched lengths, fall back to empty durations
        if forward_route.len() != return_route_len {
            #[cfg(target_arch = "wasm32")]
            web_sys::console::warn_1(&wasm_bindgen::JsValue::from_str(&format!(
                "⚠️  Forward route has {} segments but return route has {} - durations may not sync correctly",
                forward_route.len(), return_route_len
            )));
            return vec![None; return_route_len];
        }

        let mut durations = vec![None; return_route_len];

        // Walk forward route to find segments with durations and their spans
        let mut i = 0;
        while i < forward_route.len() {
            if let Some(duration) = forward_route[i].duration {
                // Count how many segments this duration covers in forward route
                let forward_span = Self::count_segments_without_duration(forward_route, i);
                let span_len = forward_span.len();

                // Mirror this span to return route
                // Forward segment i covering span_len segments maps to:
                // Return segment starting at (len - i - span_len)
                let return_start = return_route_len.saturating_sub(i + span_len);
                if return_start < durations.len() {
                    durations[return_start] = Some(duration);
                }

                i += span_len;
            } else {
                i += 1;
            }
        }

        durations
    }

    /// Process segments with duration inheritance and add station times/segments
    #[allow(clippy::too_many_arguments)]
    fn process_segments_with_duration(
        segments_since_duration: &[usize],
        duration: Duration,
        route: &[crate::models::RouteSegment],
        route_nodes: &[Option<petgraph::stable_graph::NodeIndex>],
        graph: &RailwayGraph,
        departure_time: NaiveDateTime,
        cumulative_time: &mut Duration,
        station_times: &mut Vec<(petgraph::stable_graph::NodeIndex, NaiveDateTime, NaiveDateTime)>,
        segments: &mut Vec<JourneySegment>,
    ) {
        let segments_to_cover = segments_since_duration.len();
        let duration_per_segment = if segments_to_cover > 0 {
            duration / i32::try_from(segments_to_cover).unwrap_or(1)
        } else {
            duration
        };

        for &seg_idx in segments_since_duration {
            let seg = &route[seg_idx];
            *cumulative_time += duration_per_segment;
            let arrival_time = departure_time + *cumulative_time;

            // Only add wait time if the destination node is not a junction
            let is_junction = route_nodes.get(seg_idx + 1)
                .and_then(|&node_idx| node_idx)
                .and_then(|node_idx| graph.graph.node_weight(node_idx))
                .is_some_and(|node| node.as_junction().is_some());

            if !is_junction {
                *cumulative_time += seg.wait_time;
            }
            let departure_from_station = departure_time + *cumulative_time;

            if let Some(node_idx) = route_nodes[seg_idx + 1] {
                station_times.push((node_idx, arrival_time, departure_from_station));

                segments.push(JourneySegment {
                    edge_index: seg.edge_index,
                    track_index: seg.track_index,
                    origin_platform: seg.origin_platform,
                    destination_platform: seg.destination_platform,
                });
            }
        }
    }

    /// Generate train journeys for all lines throughout the day
    ///
    /// # Arguments
    /// * `lines` - The lines to generate journeys for
    /// * `graph` - The railway graph
    /// * `selected_day` - Optional day of week filter. If provided, only generates journeys for lines operating on that day
    ///
    /// # Panics
    /// Panics if `BASE_DATE` cannot be converted to a valid datetime at midnight (00:00:00)
    #[must_use]
    pub fn generate_journeys(lines: &[Line], graph: &RailwayGraph, selected_day: Option<Weekday>) -> HashMap<uuid::Uuid, TrainJourney> {
        let mut journeys = HashMap::new();

        // Determine which days to simulate
        let days_to_simulate: Vec<(Weekday, i64)> = if let Some(day) = selected_day {
            // Only simulate the selected day
            vec![(day, 0)]
        } else {
            // Simulate Sunday from previous week (day -1) to catch late Sunday trains
            // that extend into Monday, then the full week (days 0-6)
            vec![
                (Weekday::Sun, -1), // Previous week's Sunday for wraparound conflicts
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
                        // Generate auto-scheduled forward journeys
                        Self::generate_forward_journeys(&mut journeys, line, graph, current_date, day_end);

                        // Generate auto-scheduled return journeys
                        Self::generate_return_journeys(&mut journeys, line, graph, current_date, day_end);

                        // Also generate any manual departures (for special services)
                        Self::generate_manual_journeys(&mut journeys, line, graph, current_date, day_filter);
                    }
                    ScheduleMode::Manual => {
                        // Generate journeys from manual departures only
                        Self::generate_manual_journeys(&mut journeys, line, graph, current_date, day_filter);
                    }
                }
            }
        }

        // Filter out journeys from day -1 (previous Sunday) that don't extend into the current week
        // Keep only journeys that have at least one station time >= Monday 00:00:00
        if selected_day.is_none() {
            let week_start = BASE_DATE.and_hms_opt(0, 0, 0).expect("Valid datetime");
            journeys.retain(|_, journey| {
                // Check if any station time (arrival or departure) is within the current week
                journey.station_times.iter().any(|(_, arrival, departure)| {
                    *arrival >= week_start || *departure >= week_start
                })
            });
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
        let line_id = line.id;
        let line_name = line.name.clone();
        let color = line.color.clone();
        let thickness = line.thickness;

        while departure_time <= day_end && journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::with_capacity(route_nodes.len());
            let mut segments = Vec::with_capacity(line.forward_route.len());

            // Apply first stop wait time to the first station
            let first_wait_time = line.first_stop_wait_time;
            let mut cumulative_time = first_wait_time;

            // Add first node (station or junction) with wait time
            if let Some(node_idx) = route_nodes[0] {
                station_times.push((node_idx, departure_time, departure_time + first_wait_time));
            }

            // Walk the route, handling duration inheritance
            // When a segment has a duration, it covers all segments until the next duration
            let mut i = 0;
            while i < line.forward_route.len() {
                if let Some(duration) = line.forward_route[i].duration {
                    let segments_to_cover = Self::count_segments_without_duration(&line.forward_route, i);
                    let next_index = segments_to_cover.last().copied().unwrap_or(i) + 1;

                    Self::process_segments_with_duration(
                        &segments_to_cover,
                        duration,
                        &line.forward_route,
                        &route_nodes,
                        graph,
                        departure_time,
                        &mut cumulative_time,
                        &mut station_times,
                        &mut segments,
                    );

                    i = next_index;
                } else {
                    // Segment without duration and no previous duration - use fallback
                    Self::process_segments_without_duration(
                        &[i],
                        &line.forward_route,
                        &route_nodes,
                        graph,
                        departure_time,
                        &mut cumulative_time,
                        &mut station_times,
                        &mut segments,
                    );
                    i += 1;
                }
            }

            if station_times.len() >= 2 {
                // Validate journey integrity
                if segments.len() != station_times.len() - 1 {
                    #[cfg(target_arch = "wasm32")]
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format!(
                        "❌ Journey construction error for line '{}': {} segments but {} stations (expected {})",
                        line_name, segments.len(), station_times.len(), station_times.len() - 1
                    )));
                    // Skip this invalid journey
                    departure_time += line.frequency;
                    continue;
                }

                let id = uuid::Uuid::new_v4();
                let train_number = generate_train_number(&line.auto_train_number_format, &line_name, journey_count + 1);
                let route_start_node = station_times.first().map(|(node_idx, _, _)| *node_idx);
                let route_end_node = station_times.last().map(|(node_idx, _, _)| *node_idx);
                journeys.insert(id, TrainJourney {
                    id,
                    line_id,
                    train_number,
                    departure_time,
                    station_times,
                    segments,
                    color: color.clone(),
                    thickness,
                    route_start_node,
                    route_end_node,
                });
                journey_count += 1;
            }

            departure_time += line.frequency;

            // Check if next departure would be after the last departure time
            let Some(last_departure_on_date) = time_on_date(line.last_departure, current_date) else {
                break;
            };
            if departure_time > last_departure_on_date {
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
    ) {
        let mut sequence = 1;

        // Determine end of day
        let Some(end_of_day) = current_date.and_hms_opt(23, 59, 59) else {
            return;
        };

        for manual_dep in &line.manual_departures {
            // Filter by day of week
            if !manual_dep.days_of_week.contains(day_filter) {
                continue;
            }

            // Convert the manual departure time to the current date
            let Some(initial_departure_time) = time_on_date(manual_dep.time, current_date) else {
                continue;
            };

            let from_idx = manual_dep.from_station;
            let to_idx = manual_dep.to_station;

            // Check if this is a repeating departure
            if let Some(repeat_interval) = manual_dep.repeat_interval {
                // Determine when to stop repeating
                let repeat_until = if let Some(until_time) = manual_dep.repeat_until {
                    time_on_date(until_time, current_date).unwrap_or(end_of_day)
                } else {
                    end_of_day
                };

                // Generate multiple journeys at the repeat interval
                let mut current_departure = initial_departure_time;

                while current_departure <= repeat_until {
                    Self::try_generate_manual_journey(
                        journeys,
                        line,
                        graph,
                        current_departure,
                        from_idx,
                        to_idx,
                        manual_dep.train_number.as_ref(),
                        &mut sequence,
                    );

                    // Move to next departure time
                    current_departure += repeat_interval;
                }
            } else {
                // Single departure (no repeat)
                Self::try_generate_manual_journey(
                    journeys,
                    line,
                    graph,
                    initial_departure_time,
                    from_idx,
                    to_idx,
                    manual_dep.train_number.as_ref(),
                    &mut sequence,
                );
            }
        }
    }

    /// Try to generate a single manual journey on either forward or return route
    /// Returns true if a journey was successfully generated
    fn try_generate_manual_journey(
        journeys: &mut HashMap<uuid::Uuid, TrainJourney>,
        line: &Line,
        graph: &RailwayGraph,
        departure_time: NaiveDateTime,
        from_idx: petgraph::graph::NodeIndex,
        to_idx: petgraph::graph::NodeIndex,
        custom_train_number: Option<&String>,
        sequence: &mut usize,
    ) -> bool {
        // Use custom train number if provided, otherwise generate one
        let train_number = custom_train_number.cloned()
            .unwrap_or_else(|| generate_train_number(&line.auto_train_number_format, &line.name, *sequence));

        // Try forward route first
        if let Some(journey) = Self::generate_manual_journey_for_route(
            &line.forward_route,
            line,
            graph,
            departure_time,
            from_idx,
            to_idx,
            &train_number,
        ) {
            journeys.insert(journey.id, journey);
            *sequence += 1;
            return true;
        }

        // Try return route if forward didn't work
        if let Some(journey) = Self::generate_manual_journey_for_route(
            &line.return_route,
            line,
            graph,
            departure_time,
            from_idx,
            to_idx,
            &train_number,
        ) {
            journeys.insert(journey.id, journey);
            *sequence += 1;
            return true;
        }

        false
    }

    fn generate_manual_journey_for_route(
        route: &[crate::models::RouteSegment],
        line: &Line,
        graph: &RailwayGraph,
        departure_time: NaiveDateTime,
        from_idx: petgraph::graph::NodeIndex,
        to_idx: petgraph::graph::NodeIndex,
        train_number: &str,
    ) -> Option<TrainJourney> {
        // Use the same route node building logic as auto-generated journeys
        let route_nodes_opt = Self::build_route_nodes(route, graph);
        let route_nodes: Vec<_> = route_nodes_opt.iter().filter_map(|&n| n).collect();

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

        // Convert route_nodes to Option<NodeIndex> for compatibility with helper functions
        let route_nodes_opt: Vec<Option<petgraph::stable_graph::NodeIndex>> = route_nodes.iter().map(|&idx| Some(idx)).collect();

        let mut i = from_pos;
        while i < to_pos {
            if let Some(duration) = route[i].duration {
                // Find all segments until the next duration (or end of route segment)
                let mut segments_to_cover = vec![i];
                let mut j = i + 1;
                while j < to_pos && route[j].duration.is_none() {
                    segments_to_cover.push(j);
                    j += 1;
                }

                Self::process_segments_with_duration(
                    &segments_to_cover,
                    duration,
                    route,
                    &route_nodes_opt,
                    graph,
                    departure_time,
                    &mut cumulative_time,
                    &mut station_times,
                    &mut segments,
                );

                i = j;
            } else {
                Self::process_segments_without_duration(
                    &[i],
                    route,
                    &route_nodes_opt,
                    graph,
                    departure_time,
                    &mut cumulative_time,
                    &mut station_times,
                    &mut segments,
                );
                i += 1;
            }
        }

        if station_times.len() >= 2 {
            let route_start_node = station_times.first().map(|(node_idx, _, _)| *node_idx);
            let route_end_node = station_times.last().map(|(node_idx, _, _)| *node_idx);
            Some(TrainJourney {
                id: uuid::Uuid::new_v4(),
                line_id: line.id,
                train_number: train_number.to_string(),
                departure_time,
                station_times,
                segments,
                color: line.color.clone(),
                thickness: line.thickness,
                route_start_node,
                route_end_node,
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
        let line_id = line.id;
        let line_name = line.name.clone();
        let color = line.color.clone();
        let thickness = line.thickness;

        while return_departure_time <= day_end && return_journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::with_capacity(route_nodes.len());
            let mut segments = Vec::with_capacity(line.return_route.len());

            // Apply first stop wait time to the first station
            let first_wait_time = line.return_first_stop_wait_time;
            let mut cumulative_time = first_wait_time;

            // Add first node (station or junction) with wait time
            if let Some(node_idx) = route_nodes[0] {
                station_times.push((node_idx, return_departure_time, return_departure_time + first_wait_time));
            }

            // Build duration lookup from forward route if sync is enabled
            // This mirrors the forward route's duration inheritance pattern in reverse
            let return_durations: Vec<Option<Duration>> = if line.sync_routes {
                Self::build_synced_return_durations(&line.forward_route, line.return_route.len())
            } else {
                // Use return route's own durations
                line.return_route.iter().map(|seg| seg.duration).collect()
            };

            // Walk the return route, handling duration inheritance
            let mut i = 0;
            while i < line.return_route.len() {
                if let Some(duration) = return_durations.get(i).and_then(|d| *d) {
                    // Count segments covered by this duration (including segments without duration that follow)
                    let segments_to_cover = Self::count_segments_from_duration_list(&return_durations, i);
                    let next_index = segments_to_cover.last().copied().unwrap_or(i) + 1;

                    Self::process_segments_with_duration(
                        &segments_to_cover,
                        duration,
                        &line.return_route,
                        &route_nodes,
                        graph,
                        return_departure_time,
                        &mut cumulative_time,
                        &mut station_times,
                        &mut segments,
                    );

                    i = next_index;
                } else {
                    Self::process_segments_without_duration(
                        &[i],
                        &line.return_route,
                        &route_nodes,
                        graph,
                        return_departure_time,
                        &mut cumulative_time,
                        &mut station_times,
                        &mut segments,
                    );
                    i += 1;
                }
            }

            if station_times.len() >= 2 {
                // Validate journey integrity
                if segments.len() != station_times.len() - 1 {
                    #[cfg(target_arch = "wasm32")]
                    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&format!(
                        "❌ Journey construction error for line '{}' (return): {} segments but {} stations (expected {})",
                        line_name, segments.len(), station_times.len(), station_times.len() - 1
                    )));
                    // Skip this invalid journey
                    return_departure_time += line.frequency;
                    continue;
                }

                let id = uuid::Uuid::new_v4();
                let train_number = generate_train_number(&line.auto_train_number_format, &line_name, return_journey_count + 1);
                let route_start_node = station_times.first().map(|(node_idx, _, _)| *node_idx);
                let route_end_node = station_times.last().map(|(node_idx, _, _)| *node_idx);
                journeys.insert(id, TrainJourney {
                    id,
                    line_id,
                    train_number,
                    departure_time: return_departure_time,
                    station_times,
                    segments,
                    color: color.clone(),
                    thickness,
                    route_start_node,
                    route_end_node,
                });
                return_journey_count += 1;
            }

            return_departure_time += line.frequency;

            // Check if next departure would be after the last departure time
            let Some(last_departure_on_date) = time_on_date(line.return_last_departure, current_date) else {
                break;
            };
            if return_departure_time > last_departure_on_date {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RouteSegment, RailwayGraph, Line, ScheduleMode, Track, TrackDirection, Stations, Tracks};

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
            id: uuid::Uuid::new_v4(),
            name: "Test Line".to_string(),
            color: TEST_COLOR.to_string(),
            thickness: TEST_THICKNESS,
            visible: true,
            forward_route: vec![
                RouteSegment {
                    edge_index: edge1.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(10)),
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: edge2.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(15)),
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
            auto_train_number_format: "{line} {seq:04}".to_string(),
                last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
                return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
                default_wait_time: Duration::seconds(30),
                first_stop_wait_time: Duration::zero(),
                return_first_stop_wait_time: Duration::zero(),
            sort_index: None,
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
        let line_id = line.id;

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

        assert!(!journeys.is_empty());

        let first_journey = journeys.values().next().expect("has journey");
        assert_eq!(first_journey.line_id, line_id);
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
    fn test_generate_journeys_stops_at_last_departure() {
        let graph = create_test_graph();
        let mut line = create_test_line(&graph);
        line.first_departure = BASE_DATE.and_hms_opt(20, 0, 0).expect("valid time");
        line.return_first_departure = BASE_DATE.and_hms_opt(20, 0, 0).expect("valid time");
        line.last_departure = BASE_DATE.and_hms_opt(21, 0, 0).expect("valid time");
        line.return_last_departure = BASE_DATE.and_hms_opt(21, 0, 0).expect("valid time");
        line.frequency = Duration::minutes(30);

        let journeys = TrainJourney::generate_journeys(&[line], &graph, Some(Weekday::Mon));

        // Should only generate journeys up to and including last_departure (21:00)
        // With 20:00 start, 30 min frequency, and 21:00 end: expect 20:00, 20:30, 21:00
        assert!(!journeys.is_empty());

        for journey in journeys.values() {
            assert!(journey.departure_time <= BASE_DATE.and_hms_opt(21, 0, 0).expect("valid time"),
                "Journey departed at {:?}, which is after 21:00", journey.departure_time);
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
                    duration: Some(Duration::minutes(15)),
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: e2.index(),
                    track_index: 0,
                    origin_platform: 1,
                    destination_platform: 1,
                    duration: Some(Duration::minutes(10)),
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
                id: uuid::Uuid::new_v4(),
                time: BASE_DATE.and_hms_opt(10, 0, 0).expect("valid time"),
                from_station: idx1,
                to_station: idx2,
                days_of_week: DaysOfWeek::MONDAY | DaysOfWeek::WEDNESDAY | DaysOfWeek::FRIDAY,
                train_number: None,
                repeat_interval: None,
                repeat_until: None,
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
            id: uuid::Uuid::new_v4(),
            name: "Test Line with Junction".to_string(),
            color: TEST_COLOR.to_string(),
            thickness: TEST_THICKNESS,
            visible: true,
            forward_route: vec![
                RouteSegment {
                    edge_index: edge1.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(5)),
                    wait_time: Duration::seconds(0), // No wait at junction
                },
                RouteSegment {
                    edge_index: edge2.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(5)),
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
            auto_train_number_format: "{line} {seq:04}".to_string(),
                last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
                return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
                default_wait_time: Duration::seconds(30),
                first_stop_wait_time: Duration::zero(),
                return_first_stop_wait_time: Duration::zero(),
            sort_index: None,
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

    #[test]
    fn test_sync_routes_with_gaps() {
        // Create a graph with 6 stations: A -> B -> C -> D -> E -> F
        let mut graph = RailwayGraph::new();
        let station_a = graph.add_or_get_station("Station A".to_string());
        let station_b = graph.add_or_get_station("Station B".to_string());
        let station_c = graph.add_or_get_station("Station C".to_string());
        let station_d = graph.add_or_get_station("Station D".to_string());
        let station_e = graph.add_or_get_station("Station E".to_string());
        let station_f = graph.add_or_get_station("Station F".to_string());

        graph.add_track(station_a, station_b, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(station_b, station_c, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(station_c, station_d, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(station_d, station_e, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(station_e, station_f, vec![Track { direction: TrackDirection::Bidirectional }]);

        let edge_ab = graph.graph.find_edge(station_a, station_b).expect("edge exists");
        let edge_bc = graph.graph.find_edge(station_b, station_c).expect("edge exists");
        let edge_cd = graph.graph.find_edge(station_c, station_d).expect("edge exists");
        let edge_de = graph.graph.find_edge(station_d, station_e).expect("edge exists");
        let edge_ef = graph.graph.find_edge(station_e, station_f).expect("edge exists");

        // Create a line with gaps in travel times
        // Forward: A->B (12 min covering A->B, B->C, C->D), D->E (None), E->F (8 min covering E->F)
        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test Line with Gaps".to_string(),
            color: TEST_COLOR.to_string(),
            thickness: TEST_THICKNESS,
            visible: true,
            forward_route: vec![
                RouteSegment {
                    edge_index: edge_ab.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(12)), // Covers segments 0, 1, 2
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: edge_bc.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: None, // Gap
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: edge_cd.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: None, // Gap
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: edge_de.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(6)), // Covers segments 3, 4
                    wait_time: Duration::seconds(30),
                },
                RouteSegment {
                    edge_index: edge_ef.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: None, // Gap
                    wait_time: Duration::seconds(30),
                },
            ],
            return_route: vec![],
            first_departure: BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"),
            return_first_departure: BASE_DATE.and_hms_opt(9, 0, 0).expect("valid time"),
            frequency: Duration::hours(2),
            schedule_mode: ScheduleMode::Auto,
            days_of_week: crate::models::DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(10, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(11, 0, 0).expect("valid time"),
            default_wait_time: Duration::seconds(30),
            first_stop_wait_time: Duration::zero(),
            return_first_stop_wait_time: Duration::zero(),
            sort_index: None,
        };

        // Apply sync to create return route
        line.apply_route_sync_if_enabled();

        // Test the synced return durations helper function
        let return_durations = TrainJourney::build_synced_return_durations(&line.forward_route, line.return_route.len());

        println!("Forward route durations:");
        for (i, seg) in line.forward_route.iter().enumerate() {
            println!("  Segment {i}: {:?}", seg.duration);
        }

        println!("\nComputed return durations:");
        for (i, dur) in return_durations.iter().enumerate() {
            println!("  Segment {i}: {dur:?}");
        }

        // Expected return durations:
        // Forward seg 0 (12min) covers forward 0,1,2 -> should map to return 2,3,4
        // Forward seg 3 (6min) covers forward 3,4 -> should map to return 0,1
        // So return durations should be: [Some(6min), None, Some(12min), None, None]

        assert_eq!(return_durations.len(), 5, "Return route should have 5 segments");
        assert_eq!(return_durations[0], Some(Duration::minutes(6)), "Return seg 0 should have 6min (from forward seg 3)");
        assert_eq!(return_durations[1], None, "Return seg 1 should be None");
        assert_eq!(return_durations[2], Some(Duration::minutes(12)), "Return seg 2 should have 12min (from forward seg 0)");
        assert_eq!(return_durations[3], None, "Return seg 3 should be None");
        assert_eq!(return_durations[4], None, "Return seg 4 should be None");

        // Now test actual journey generation
        let journeys = TrainJourney::generate_journeys(&[line.clone()], &graph, None);

        println!("\nGenerated {} journeys", journeys.len());

        // We should have forward and return journeys
        let forward_journeys: Vec<_> = journeys.values()
            .filter(|j| j.train_number.contains("0001"))
            .collect();
        let return_journeys: Vec<_> = journeys.values()
            .filter(|j| j.train_number.contains("0002"))
            .collect();

        println!("Forward journeys: {}", forward_journeys.len());
        println!("Return journeys: {}", return_journeys.len());

        assert!(!forward_journeys.is_empty(), "Should have forward journeys");
        assert!(!return_journeys.is_empty(), "Should have return journeys");

        // Check first forward journey timing
        let forward = forward_journeys[0];
        println!("\nForward journey stations: {}", forward.station_times.len());
        for (i, (node, arrival, departure)) in forward.station_times.iter().enumerate() {
            println!("  Stop {}: node={}, arrival={}, departure={}",
                i, node.index(), arrival.format("%H:%M:%S"), departure.format("%H:%M:%S"));
        }

        assert_eq!(forward.station_times.len(), 6, "Forward should stop at 6 stations");

        // Forward timing: Start at 08:00
        // Seg 0,1,2: 12 min total = 4 min each
        // Station A: 08:00 (depart)
        // Station B: 08:04 arrive, 08:04:30 depart
        // Station C: 08:08 arrive, 08:08:30 depart
        // Station D: 08:12 arrive, 08:12:30 depart
        // Seg 3,4: 6 min total = 3 min each
        // Station E: 08:15:30 arrive, 08:16 depart
        // Station F: 08:19 arrive

        // Check first return journey timing
        let return_j = return_journeys[0];
        println!("\nReturn journey stations: {}", return_j.station_times.len());
        println!("Return journey segments: {}", return_j.segments.len());
        for (i, (node, arrival, departure)) in return_j.station_times.iter().enumerate() {
            println!("  Stop {}: node={}, arrival={}, departure={}",
                i, node.index(), arrival.format("%H:%M:%S"), departure.format("%H:%M:%S"));
        }

        assert_eq!(return_j.station_times.len(), 6, "Return should stop at 6 stations");
        assert_eq!(return_j.segments.len(), 5, "Return should have 5 segments");

        // Return timing should mirror forward:
        // Return seg 0,1: 6 min total = 3 min each (from forward seg 3)
        // Station F: 09:00 (depart)
        // Station E: 09:03 arrive, 09:03:30 depart
        // Station D: 09:06:30 arrive, 09:07 depart
        // Return seg 2,3,4: 12 min total = 4 min each (from forward seg 0)
        // Station C: 09:11 arrive, 09:11:30 depart
        // Station B: 09:15 arrive, 09:15:30 depart
        // Station A: 09:19 arrive
    }

    #[test]
    fn test_sync_routes_with_standalone_gaps() {
        // Test case where forward has: [10min covers just 0, gap at 1, 6min covers 2-3]
        // This should map to return: [6min covers 0-1, gap at 2, 10min covers just 3]

        let mut graph = RailwayGraph::new();
        let station_a = graph.add_or_get_station("Station A".to_string());
        let station_b = graph.add_or_get_station("Station B".to_string());
        let station_c = graph.add_or_get_station("Station C".to_string());
        let station_d = graph.add_or_get_station("Station D".to_string());

        graph.add_track(station_a, station_b, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(station_b, station_c, vec![Track { direction: TrackDirection::Bidirectional }]);
        graph.add_track(station_c, station_d, vec![Track { direction: TrackDirection::Bidirectional }]);

        let edge_ab = graph.graph.find_edge(station_a, station_b).expect("edge exists");
        let edge_bc = graph.graph.find_edge(station_b, station_c).expect("edge exists");
        let edge_cd = graph.graph.find_edge(station_c, station_d).expect("edge exists");

        // Forward: seg 0 (10min, just covers itself), seg 1 (None, gap), seg 2 (6min, covers 2-3)
        let forward_route = vec![
            RouteSegment {
                edge_index: edge_ab.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(10)), // Only covers segment 0
                wait_time: Duration::seconds(30),
            },
            RouteSegment {
                edge_index: edge_bc.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: None, // Standalone gap - not covered by anything
                wait_time: Duration::seconds(30),
            },
            RouteSegment {
                edge_index: edge_cd.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(6)), // Covers segments 2-3 (but there's only seg 2, so just itself)
                wait_time: Duration::seconds(30),
            },
        ];

        println!("Forward route durations:");
        for (i, seg) in forward_route.iter().enumerate() {
            println!("  Segment {i}: {:?}", seg.duration);
        }

        // Build return durations
        let return_durations = TrainJourney::build_synced_return_durations(&forward_route, 3);

        println!("\nComputed return durations:");
        for (i, dur) in return_durations.iter().enumerate() {
            println!("  Segment {i}: {dur:?}");
        }

        // Expected mapping with inheritance:
        // Forward: [A->B: 10min covering [0,1], B->C: None (inherited), C->D: 6min covering [2]]
        // Return:  [D->C: 6min covering [0], C->B: 10min covering [1,2], B->A: None (inherited)]

        println!("\nExpected return durations: [Some(6min), Some(10min), None]");

        assert_eq!(return_durations.len(), 3);
        assert_eq!(return_durations[0], Some(Duration::minutes(6)), "Return seg 0 should be 6min");
        assert_eq!(return_durations[1], Some(Duration::minutes(10)), "Return seg 1 should be 10min");
        assert_eq!(return_durations[2], None, "Return seg 2 should be None (inherited from seg 1)");

        // Now test that actual journeys are generated and valid
        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Standalone Gap Test".to_string(),
            color: TEST_COLOR.to_string(),
            thickness: TEST_THICKNESS,
            visible: true,
            forward_route,
            return_route: vec![],
            first_departure: BASE_DATE.and_hms_opt(8, 0, 0).expect("valid time"),
            return_first_departure: BASE_DATE.and_hms_opt(9, 0, 0).expect("valid time"),
            frequency: Duration::hours(1),
            schedule_mode: ScheduleMode::Auto,
            days_of_week: crate::models::DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(10, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(11, 0, 0).expect("valid time"),
            default_wait_time: Duration::seconds(30),
            first_stop_wait_time: Duration::zero(),
            return_first_stop_wait_time: Duration::zero(),
            sort_index: None,
        };

        line.apply_route_sync_if_enabled();

        let journeys = TrainJourney::generate_journeys(&[line], &graph, None);

        println!("\nGenerated {} total journeys", journeys.len());

        let forward_journeys: Vec<_> = journeys.values()
            .filter(|j| j.departure_time.time() == chrono::NaiveTime::from_hms_opt(8, 0, 0).expect("valid time"))
            .collect();
        let return_journeys: Vec<_> = journeys.values()
            .filter(|j| j.departure_time.time() == chrono::NaiveTime::from_hms_opt(9, 0, 0).expect("valid time"))
            .collect();

        println!("Forward journeys: {}", forward_journeys.len());
        println!("Return journeys: {}", return_journeys.len());

        assert!(!return_journeys.is_empty(), "Should have return journeys");

        let return_j = return_journeys[0];
        println!("\nReturn journey validation:");
        println!("  Stations: {}", return_j.station_times.len());
        println!("  Segments: {}", return_j.segments.len());
        println!("  Has route_start_node: {}", return_j.route_start_node.is_some());
        println!("  Has route_end_node: {}", return_j.route_end_node.is_some());

        assert_eq!(return_j.station_times.len(), 4, "Return should have 4 stations");
        assert_eq!(return_j.segments.len(), 3, "Return should have 3 segments");
        assert!(return_j.route_start_node.is_some(), "Return should have start node");
        assert!(return_j.route_end_node.is_some(), "Return should have end node");

        // Verify times are monotonically increasing
        for i in 1..return_j.station_times.len() {
            let prev_departure = return_j.station_times[i-1].2;
            let curr_arrival = return_j.station_times[i].1;
            assert!(curr_arrival >= prev_departure,
                "Station {i} arrival ({curr_arrival}) should be >= previous departure ({prev_departure})");
        }

        println!("\n✓ Return journey is valid and renderable");
    }

}
