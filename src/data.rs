use crate::models::{Line, RailwayGraph, RouteSegment};
use chrono::{Duration, Timelike};
use petgraph::graph::{EdgeIndex, NodeIndex};
use std::collections::HashMap;

/// Parse CSV data into lines and railway graph
pub fn parse_csv_data() -> (Vec<Line>, RailwayGraph) {
    let csv_content = include_str!("../lines.csv");
    parse_csv_string(csv_content)
}

/// Parse CSV string into lines and railway graph
pub fn parse_csv_string(csv_content: &str) -> (Vec<Line>, RailwayGraph) {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(csv_content.as_bytes());

    let mut records = reader.records();

    let Some(Ok(header)) = records.next() else {
        return (Vec::new(), RailwayGraph::new());
    };

    let line_ids = extract_line_ids(&header);
    let (lines, graph) = build_graph_and_routes_from_csv(records, &line_ids);

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
                    chrono::NaiveTime::parse_from_str(s, "%H:%M:%S")
                        .ok()
                        .map(|t| {
                            Duration::hours(t.hour() as i64) +
                            Duration::minutes(t.minute() as i64) +
                            Duration::seconds(t.second() as i64)
                        })
                });
            times.push(time_opt);
        }

        station_data.push((station_name.to_string(), times));
    }

    // Track edges by (from_node, to_node) to avoid duplicates
    let mut edge_map: HashMap<(NodeIndex, NodeIndex), EdgeIndex> = HashMap::new();

    // Second pass: build shared infrastructure and line routes
    for (line_idx, _line_id) in line_ids.iter().enumerate() {
        let mut route = Vec::new();
        let mut prev_station: Option<(NodeIndex, Duration)> = None;

        for (station_name, times) in &station_data {
            if let Some(cumulative_time) = times[line_idx] {
                // Get or create station node
                let station_idx = graph.add_or_get_station(station_name.clone());

                // If there was a previous station, create or reuse edge
                if let Some((prev_idx, prev_time)) = prev_station {
                    let travel_time = cumulative_time - prev_time;

                    // Check if edge already exists
                    let edge_idx = *edge_map.entry((prev_idx, station_idx))
                        .or_insert_with(|| {
                            // Create new track segment (initially single-tracked)
                            graph.add_track(prev_idx, station_idx, false)
                        });

                    // Add to this line's route
                    route.push(RouteSegment {
                        edge_index: edge_idx.index(),
                        duration: travel_time,
                    });
                }

                prev_station = Some((station_idx, cumulative_time));
            }
        }

        // Assign route to line
        lines[line_idx].route = route;
    }

    (lines, graph)
}