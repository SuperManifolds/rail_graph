use crate::models::{Line, RailwayGraph, RouteSegment, Stations, Tracks};
use chrono::{Duration, Timelike};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::HashMap;

/// Parse CSV data into lines and railway graph
#[must_use]
pub fn parse_csv_data() -> (Vec<Line>, RailwayGraph) {
    let csv_content = include_str!("../test-data/lines.csv");
    parse_csv_string(csv_content, &HashMap::new())
}

/// Parse CSV string into lines and railway graph
#[must_use]
pub fn parse_csv_string(csv_content: &str, wait_times: &HashMap<String, Duration>) -> (Vec<Line>, RailwayGraph) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(csv_content.as_bytes());

    let mut records = reader.records();

    let Some(Ok(header)) = records.next() else {
        return (Vec::new(), RailwayGraph::new());
    };

    let line_ids = extract_line_ids(&header);
    let (lines, graph) = build_graph_and_routes_from_csv(records, &line_ids, wait_times);

    (lines, graph)
}

fn extract_line_ids(header: &csv::StringRecord) -> Vec<String> {
    header.iter()
        .skip(1)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn build_graph_and_routes_from_csv(
    records: csv::StringRecordsIter<&[u8]>,
    line_ids: &[String],
    wait_times: &HashMap<String, Duration>,
) -> (Vec<Line>, RailwayGraph) {
    let mut graph = RailwayGraph::new();
    let mut lines = Line::create_from_ids(line_ids, 0);

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
                let Some(node) = graph.graph.node_weight_mut(station_idx) else { continue };
                let Some(station) = node.as_station_mut() else { continue };
                station.passing_loop = true;
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

            // Determine default platforms based on track configuration
            let origin_platforms = graph.graph.node_weight(prev_idx)
                .and_then(|n| n.as_station())
                .map_or(1, |s| s.platforms.len());

            let dest_platforms = graph.graph.node_weight(station_idx)
                .and_then(|n| n.as_station())
                .map_or(1, |s| s.platforms.len());

            let origin_platform = graph.get_default_platform_for_arrival(edge_idx, false, origin_platforms);
            let destination_platform = graph.get_default_platform_for_arrival(edge_idx, true, dest_platforms);

            // Add to this line's forward route
            route.push(RouteSegment {
                edge_index: edge_idx.index(),
                track_index: 0,
                origin_platform,
                destination_platform,
                duration: Some(travel_time),
                wait_time: station_wait_time,
            });

            prev_station = Some((station_idx, cumulative_time));
        }

        // Assign forward route to line
        lines[line_idx].forward_route.clone_from(&route);

        // Generate return route (reverse direction, using opposite track and swapped platforms)
        let mut return_route = Vec::new();
        for i in (0..route.len()).rev() {
            let forward_segment = &route[i];

            // Determine return track: use track 1 if edge has multiple tracks, else track 0 (bidirectional)
            let edge_idx = petgraph::graph::EdgeIndex::new(forward_segment.edge_index);
            let return_track_index = if let Some(track_segment) = graph.get_track(edge_idx) {
                usize::from(track_segment.tracks.len() > 1)
            } else {
                0 // Default to track 0 if edge not found
            };

            // For return route, swap origin and destination platforms since we're traveling in reverse
            return_route.push(RouteSegment {
                edge_index: forward_segment.edge_index,
                track_index: return_track_index,
                origin_platform: forward_segment.destination_platform,
                destination_platform: forward_segment.origin_platform,
                duration: forward_segment.duration,
                wait_time: forward_segment.wait_time,
            });
        }
        lines[line_idx].return_route = return_route;
    }

    (lines, graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_line_ids() {
        let mut record = csv::StringRecord::new();
        record.push_field("Station");
        record.push_field("Line 1");
        record.push_field("Line 2");
        record.push_field("");
        record.push_field("Line 3");

        let line_ids = extract_line_ids(&record);
        assert_eq!(line_ids.len(), 3);
        assert_eq!(line_ids[0], "Line 1");
        assert_eq!(line_ids[1], "Line 2");
        assert_eq!(line_ids[2], "Line 3");
    }

    #[test]
    fn test_extract_line_ids_empty() {
        let mut record = csv::StringRecord::new();
        record.push_field("Station");

        let line_ids = extract_line_ids(&record);
        assert_eq!(line_ids.len(), 0);
    }

    #[test]
    fn test_parse_csv_string_empty() {
        let csv_content = "";
        let wait_times = HashMap::new();

        let (lines, graph) = parse_csv_string(csv_content, &wait_times);

        assert_eq!(lines.len(), 0);
        assert_eq!(graph.graph.node_count(), 0);
        assert_eq!(graph.graph.edge_count(), 0);
    }

    #[test]
    fn test_parse_csv_string_simple() {
        let csv_content = "Station,Line1\nStationA,0:00:00\nStationB,0:10:00\n";
        let wait_times = HashMap::new();

        let (lines, graph) = parse_csv_string(csv_content, &wait_times);

        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].name, "Line1");
        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 1);
        assert_eq!(lines[0].forward_route.len(), 1);
        assert_eq!(lines[0].return_route.len(), 1);
    }

    #[test]
    fn test_parse_csv_string_with_passing_loop() {
        let csv_content = "Station,Line1\nStationA,0:00:00\nStationB(P),0:05:00\nStationC,0:10:00\n";
        let wait_times = HashMap::new();

        let (lines, graph) = parse_csv_string(csv_content, &wait_times);

        assert_eq!(graph.graph.node_count(), 3);
        let station_b_idx = graph.get_station_index("StationB").expect("StationB should exist");
        let node = graph.graph.node_weight(station_b_idx).expect("node should exist");
        let station_b = node.as_station().expect("should be station");
        assert!(station_b.passing_loop);

        // First segment (A->B) arrives at passing loop, should have 0 wait time
        // Second segment (B->C) departs from passing loop, should have 0 wait time
        assert_eq!(lines[0].forward_route.len(), 2);
        assert_eq!(lines[0].forward_route[0].wait_time, Duration::seconds(0));
        assert_eq!(lines[0].forward_route[1].wait_time, Duration::seconds(30));
    }

    #[test]
    fn test_parse_csv_string_with_custom_wait_times() {
        let csv_content = "Station,Line1\nStationA,0:00:00\nStationB,0:10:00\n";
        let mut wait_times = HashMap::new();
        wait_times.insert("Line1".to_string(), Duration::minutes(2));

        let (lines, _) = parse_csv_string(csv_content, &wait_times);

        assert_eq!(lines[0].forward_route[0].wait_time, Duration::minutes(2));
    }

    #[test]
    fn test_parse_csv_string_multiple_lines() {
        let csv_content = "Station,Line1,Line2\nStationA,0:00:00,0:00:00\nStationB,0:10:00,0:15:00\n";
        let wait_times = HashMap::new();

        let (lines, graph) = parse_csv_string(csv_content, &wait_times);

        assert_eq!(lines.len(), 2);
        assert_eq!(graph.graph.node_count(), 2);
        assert_eq!(graph.graph.edge_count(), 1);
        assert_eq!(lines[0].forward_route.len(), 1);
        assert_eq!(lines[1].forward_route.len(), 1);
    }

    #[test]
    fn test_parse_csv_string_sparse_route() {
        let csv_content = "Station,Line1,Line2\nStationA,0:00:00,\nStationB,0:10:00,0:05:00\nStationC,,0:15:00\n";
        let wait_times = HashMap::new();

        let (lines, graph) = parse_csv_string(csv_content, &wait_times);

        assert_eq!(lines.len(), 2);
        assert_eq!(graph.graph.node_count(), 3);
        assert_eq!(lines[0].forward_route.len(), 1); // A -> B only
        assert_eq!(lines[1].forward_route.len(), 1); // B -> C only
    }

    #[test]
    fn test_parse_csv_string_return_route_generation() {
        let csv_content = "Station,Line1\nStationA,0:00:00\nStationB,0:10:00\nStationC,0:20:00\n";
        let wait_times = HashMap::new();

        let (lines, _) = parse_csv_string(csv_content, &wait_times);

        assert_eq!(lines[0].forward_route.len(), 2);
        assert_eq!(lines[0].return_route.len(), 2);

        // Return route should be reversed
        assert_eq!(lines[0].return_route[0].edge_index, lines[0].forward_route[1].edge_index);
        assert_eq!(lines[0].return_route[1].edge_index, lines[0].forward_route[0].edge_index);

        // Return route platforms should be swapped from forward route
        // Forward route: C->B segment has destination_platform for B
        // Return route: B->C segment has origin_platform (was forward's destination) for B
        assert_eq!(lines[0].return_route[0].origin_platform, lines[0].forward_route[1].destination_platform);
        assert_eq!(lines[0].return_route[0].destination_platform, lines[0].forward_route[1].origin_platform);
    }
}