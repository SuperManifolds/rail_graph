use crate::models::{Line, RailwayGraph, RouteSegment};
use chrono::{Duration, Timelike};
use petgraph::graph::{EdgeIndex, NodeIndex};
use std::collections::HashMap;

/// Parse CSV data into lines and railway graph
#[must_use]
pub fn parse_csv_data() -> (Vec<Line>, RailwayGraph) {
    let csv_content = include_str!("../lines.csv");
    parse_csv_string(csv_content, HashMap::new())
}

/// Parse CSV string into lines and railway graph
#[must_use]
pub fn parse_csv_string(csv_content: &str, wait_times: HashMap<String, Duration>) -> (Vec<Line>, RailwayGraph) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(csv_content.as_bytes());

    let mut records = reader.records();

    let Some(Ok(header)) = records.next() else {
        return (Vec::new(), RailwayGraph::new());
    };

    let line_ids = extract_line_ids(&header);
    let (lines, graph) = build_graph_and_routes_from_csv(records, &line_ids, &wait_times);

    (lines, graph)
}

fn extract_line_ids(header: &csv::StringRecord) -> Vec<String> {
    header.iter()
        .skip(1)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn build_graph_and_routes_from_csv(
    records: csv::StringRecordsIter<&[u8]>,
    line_ids: &[String],
    wait_times: &HashMap<String, Duration>,
) -> (Vec<Line>, RailwayGraph) {
    let mut graph = RailwayGraph::new();
    let mut lines = Line::create_from_ids(line_ids);

    // First pass: collect all station data
    let mut station_data: Vec<(String, Vec<Option<Duration>>)> = Vec::new();

    for record in records {
        let Ok(row) = record else { continue };

        let Some(station_name) = row.get(0) else { continue };
        if station_name.is_empty() {
            continue;
        }

        // Parse times for each line
        let mut times = Vec::new();
        for i in 0..line_ids.len() {
            let time_opt = row.get(i + 1)
                .filter(|s| !s.is_empty())
                .and_then(|s| {
                    // Parse as time offset (H:MM:SS)
                    crate::time::parse_time_hms(s)
                        .ok()
                        .map(|t| {
                            Duration::hours(i64::from(t.hour())) +
                            Duration::minutes(i64::from(t.minute())) +
                            Duration::seconds(i64::from(t.second()))
                        })
                });
            times.push(time_opt);
        }

        station_data.push((station_name.to_string(), times));
    }

    // Track edges by (from_node, to_node) to avoid duplicates
    let mut edge_map: HashMap<(NodeIndex, NodeIndex), EdgeIndex> = HashMap::new();

    // Second pass: build shared infrastructure and line routes
    for (line_idx, line_id) in line_ids.iter().enumerate() {
        let mut route = Vec::new();
        let mut prev_station: Option<(NodeIndex, Duration)> = None;

        // Get wait time for this line (default to 30 seconds if not found)
        let line_wait_time = wait_times.get(line_id)
            .copied()
            .unwrap_or_else(|| Duration::seconds(30));

        for (station_name, times) in &station_data {
            let Some(cumulative_time) = times[line_idx] else {
                continue;
            };

            // Check if station is a passing loop (indicated by "(P)" in name)
            let is_passing_loop = station_name.ends_with("(P)");
            let clean_name = if is_passing_loop {
                station_name.trim_end_matches("(P)").trim().to_string()
            } else {
                station_name.clone()
            };

            // Get or create station node
            let station_idx = graph.add_or_get_station(clean_name);

            // Mark as passing loop if needed
            if is_passing_loop {
                if let Some(node) = graph.graph.node_weight_mut(station_idx) {
                    node.passing_loop = true;
                }
            }

            // If there was a previous station, create or reuse edge
            let Some((prev_idx, prev_time)) = prev_station else {
                prev_station = Some((station_idx, cumulative_time));
                continue;
            };

            let travel_time = cumulative_time - prev_time;

            // Check if edge already exists, or create new track segment (initially single-tracked)
            let edge_idx = *edge_map.entry((prev_idx, station_idx))
                .or_insert_with(|| {
                    use crate::models::{Track, TrackDirection};
                    graph.add_track(prev_idx, station_idx, vec![Track { direction: TrackDirection::Bidirectional }])
                });

            // Passing loops have 0 wait time
            let station_wait_time = if is_passing_loop {
                Duration::seconds(0)
            } else {
                line_wait_time
            };

            // Add to this line's forward route (using platform 0)
            route.push(RouteSegment {
                edge_index: edge_idx.index(),
                track_index: 0,
                origin_platform: 0,
                destination_platform: 0,
                duration: travel_time,
                wait_time: station_wait_time,
            });

            prev_station = Some((station_idx, cumulative_time));
        }

        // Assign forward route to line
        lines[line_idx].forward_route = route.clone();

        // Generate return route (reverse direction, using platform 1 and opposite track)
        let mut return_route = Vec::new();
        for i in (0..route.len()).rev() {
            let forward_segment = &route[i];

            // Determine return track: use track 1 if edge has multiple tracks, else track 0 (bidirectional)
            let edge_idx = petgraph::graph::EdgeIndex::new(forward_segment.edge_index);
            let return_track_index = if let Some(track_segment) = graph.get_track(edge_idx) {
                if track_segment.tracks.len() > 1 {
                    1 // Multi-track: use track 1 for return
                } else {
                    0 // Single track: use same track (bidirectional)
                }
            } else {
                0 // Default to track 0 if edge not found
            };

            return_route.push(RouteSegment {
                edge_index: forward_segment.edge_index,
                track_index: return_track_index,
                origin_platform: 1, // Use platform 1 for return direction
                destination_platform: 1,
                duration: forward_segment.duration,
                wait_time: forward_segment.wait_time,
            });
        }
        lines[line_idx].return_route = return_route;
    }

    (lines, graph)
}