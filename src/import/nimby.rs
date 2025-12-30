//! NIMBY Rails JSON import module
//!
//! Parses NIMBY Rails export files and imports lines into the railway graph.

use chrono::Duration;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use serde::Deserialize;
use std::collections::HashMap;

use crate::components::infrastructure_canvas::auto_layout::{self, GeographicHints};
use crate::constants::BASE_MIDNIGHT;
use crate::models::{
    Line, Node, ProjectSettings, RailwayGraph, RouteSegment, Routes, ScheduleMode, StationNode, Stations,
    TrackHandedness, Tracks,
};

/// Raw NIMBY Rails station from JSON
#[derive(Debug, Clone, Deserialize)]
pub struct NimbyStation {
    pub id: String,
    pub name: String,
    pub lonlat: (f64, f64),
}

/// Raw NIMBY Rails stop within a line
#[derive(Debug, Clone, Deserialize)]
pub struct NimbyStop {
    pub idx: usize,
    pub leg_distance: f64,
    pub station_id: String,
    pub arrival: i64,
    pub departure: i64,
}

/// Raw NIMBY Rails line from JSON
#[derive(Debug, Clone, Deserialize)]
pub struct NimbyLine {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub code: String,
    pub color: String,
    pub stops: Vec<NimbyStop>,
}

/// Tagged enum for parsing any NIMBY JSON record
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "class")]
pub enum NimbyRecord {
    ExportMeta {
        company_name: String,
        #[serde(default)]
        model_version: u32,
    },
    Station {
        id: String,
        name: String,
        lonlat: (f64, f64),
    },
    Line {
        id: String,
        name: String,
        #[serde(default)]
        code: String,
        color: String,
        stops: Vec<NimbyStop>,
    },
    // Skip Schedule, Shift, etc.
    #[serde(other)]
    Other,
}

/// Parsed and categorized import data
#[derive(Debug, Default, Clone)]
pub struct NimbyImportData {
    pub company_name: String,
    pub stations: HashMap<String, NimbyStation>,
    pub lines: Vec<NimbyLine>,
}

/// Summary of a line for display in the UI
#[derive(Debug, Clone)]
pub struct NimbyLineSummary {
    pub id: String,
    pub name: String,
    pub code: String,
    pub color: String,
    pub text_color: &'static str,
    pub stop_count: usize,
    pub station_count: usize,
}

/// Calculate readable text color (white or black) based on background luminance
fn calculate_text_color(hex_color: &str) -> &'static str {
    let trimmed = hex_color.trim_start_matches('#');
    if trimmed.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&trimmed[0..2], 16),
            u8::from_str_radix(&trimmed[2..4], 16),
            u8::from_str_radix(&trimmed[4..6], 16),
        ) {
            #[allow(clippy::cast_precision_loss)]
            let luminance = 0.2126 * f64::from(r) / 255.0
                + 0.7152 * f64::from(g) / 255.0
                + 0.0722 * f64::from(b) / 255.0;
            return if luminance < 0.5 { "#fff" } else { "#000" };
        }
    }
    "#fff"
}

impl NimbyImportData {
    /// Get a summary of available lines for the UI
    /// Excludes lines with fewer than 2 stops and lines starting with "D-" (deadhead runs)
    #[must_use]
    pub fn get_line_summaries(&self) -> Vec<NimbyLineSummary> {
        self.lines
            .iter()
            .filter(|line| line.stops.len() >= 2)
            .filter(|line| !line.code.starts_with("D-"))
            .map(|line| {
                let station_count = line.stops.iter()
                    .filter(|s| s.station_id != "0x0")
                    .count();
                let color = parse_nimby_color(&line.color);
                let text_color = calculate_text_color(&color);
                NimbyLineSummary {
                    id: line.id.clone(),
                    name: line.name.clone(),
                    code: line.code.clone(),
                    color,
                    text_color,
                    stop_count: line.stops.len(),
                    station_count,
                }
            })
            .collect()
    }
}

/// Parse NIMBY Rails JSON content into structured data
///
/// # Errors
/// Returns an error if JSON parsing fails
pub fn parse_nimby_json(content: &str) -> Result<NimbyImportData, String> {
    let records: Vec<NimbyRecord> = serde_json::from_str(content)
        .map_err(|e| format!("JSON parse error: {e}"))?;

    let mut data = NimbyImportData::default();

    for record in records {
        match record {
            NimbyRecord::ExportMeta { company_name, .. } => {
                data.company_name = company_name;
            }
            NimbyRecord::Station { id, name, lonlat } => {
                data.stations.insert(id.clone(), NimbyStation { id, name, lonlat });
            }
            NimbyRecord::Line { id, name, code, color, stops } => {
                data.lines.push(NimbyLine { id, name, code, color, stops });
            }
            NimbyRecord::Other => {}
        }
    }

    Ok(data)
}

/// Convert NIMBY color format (0xAABBGGRR) to CSS hex (#RRGGBB)
#[must_use]
pub fn parse_nimby_color(color: &str) -> String {
    // Format: "0xAABBGGRR" - need to swap BB and RR to get #RRGGBB
    if let Some(hex) = color.strip_prefix("0x") {
        if hex.len() >= 8 {
            let bb = &hex[2..4];
            let gg = &hex[4..6];
            let rr = &hex[6..8];
            return format!("#{rr}{gg}{bb}");
        }
    }
    "#808080".to_string()
}

/// Configuration for NIMBY Rails import
#[derive(Debug, Clone)]
pub struct NimbyImportConfig {
    /// Create new stations/tracks vs use existing infrastructure
    pub create_infrastructure: bool,
    /// IDs of lines to import (empty = all)
    pub selected_line_ids: Vec<String>,
    /// Track handedness setting
    pub handedness: TrackHandedness,
    /// Station spacing in pixels (from `ProjectSettings`)
    pub station_spacing: f64,
}

impl Default for NimbyImportConfig {
    fn default() -> Self {
        Self {
            create_infrastructure: true,
            selected_line_ids: Vec::new(),
            handedness: TrackHandedness::RightHand,
            station_spacing: 2.0 * GRID_SIZE, // default 2 grid squares
        }
    }
}

/// Build a map of how many NIMBY lines use each edge in the graph.
/// This is used to determine the "heaviest" path for spine layout.
fn build_edge_usage_map(
    data: &NimbyImportData,
    station_id_to_node: &HashMap<String, NodeIndex>,
    graph: &RailwayGraph,
) -> HashMap<EdgeIndex, usize> {
    let mut edge_usage: HashMap<EdgeIndex, usize> = HashMap::new();

    // Count edge usage for ALL lines (not just selected) to get accurate spine
    for nimby_line in &data.lines {
        // Skip very short lines and deadhead runs
        if nimby_line.stops.len() < 2 || nimby_line.code.starts_with("D-") {
            continue;
        }

        // Get valid stops (non-null station IDs that exist in graph)
        let valid_stops: Vec<_> = nimby_line
            .stops
            .iter()
            .filter(|s| s.station_id != "0x0")
            .filter_map(|s| station_id_to_node.get(&s.station_id).copied())
            .collect();

        // Count edges between consecutive stops
        for window in valid_stops.windows(2) {
            let from = window[0];
            let to = window[1];

            // Find edge between these nodes
            if let Some(edge) = graph.graph.find_edge(from, to)
                .or_else(|| graph.graph.find_edge(to, from))
            {
                *edge_usage.entry(edge).or_insert(0) += 1;
            }
        }
    }

    edge_usage
}

/// Import NIMBY Rails lines into the railway graph
///
/// Uses a two-phase approach:
/// 1. Analyze all lines to find densest paths between station pairs
/// 2. Create infrastructure based on densest paths (so express lines route through local stops)
/// 3. Create routes for each line using pathfinding
///
/// # Arguments
/// * `data` - Parsed NIMBY import data
/// * `config` - Import configuration
/// * `graph` - Railway graph to modify
/// * `existing_line_count` - Number of existing lines (for color offset)
///
/// # Returns
/// Vector of created Line objects
///
/// # Errors
/// Returns an error if import fails (e.g., station not found in pathfinding mode)
pub fn import_nimby_lines(
    data: &NimbyImportData,
    config: &NimbyImportConfig,
    graph: &mut RailwayGraph,
    existing_line_count: usize,
) -> Result<Vec<Line>, String> {
    // Filter lines to import (exclude short lines and deadhead runs)
    let lines_to_import: Vec<&NimbyLine> = if config.selected_line_ids.is_empty() {
        data.lines.iter().collect()
    } else {
        data.lines
            .iter()
            .filter(|l| config.selected_line_ids.contains(&l.id))
            .collect()
    };

    let valid_lines: Vec<&NimbyLine> = lines_to_import
        .into_iter()
        .filter(|line| line.stops.len() >= 2)
        .filter(|line| !line.code.starts_with("D-"))
        .collect();

    if config.create_infrastructure {
        // Collect nodes that already have positions (to preserve them during layout)
        let pinned_nodes: std::collections::HashSet<NodeIndex> = graph
            .graph
            .node_indices()
            .filter(|&idx| {
                graph.graph.node_weight(idx)
                    .and_then(|n| n.as_station())
                    .and_then(|s| s.position)
                    .is_some_and(|(x, y)| x != 0.0 || y != 0.0)
            })
            .collect();

        // Phase 1: Analyze all lines to build segment map
        let segment_map = build_segment_map(&valid_lines);
        leptos::logging::log!("NIMBY import: analyzed {} segment pairs", segment_map.len());

        // Phase 2: Create all stations first
        let mut station_id_to_node: HashMap<String, NodeIndex> = HashMap::new();
        for nimby_line in &valid_lines {
            for stop in &nimby_line.stops {
                if stop.station_id != "0x0" && !station_id_to_node.contains_key(&stop.station_id) {
                    let node = find_or_create_station(
                        graph,
                        &stop.station_id,
                        data,
                        true,
                        None, // No connection context during initial station creation
                    )?;
                    station_id_to_node.insert(stop.station_id.clone(), node);
                }
            }
        }
        leptos::logging::log!("NIMBY import: created {} stations", station_id_to_node.len());

        // Phase 3: Create infrastructure using densest paths
        // Process each unique consecutive station pair across all lines
        create_infrastructure_from_segments(
            graph,
            &segment_map,
            &station_id_to_node,
            config.handedness,
        );

        // Phase 3b: Create edges for consecutive station pairs, using segment map
        // to skip express-only segments that have local alternatives with matching distance
        let consecutive_count = create_consecutive_edges(
            graph,
            &valid_lines,
            &station_id_to_node,
            &segment_map,
            config.handedness,
        );
        if consecutive_count > 0 {
            leptos::logging::log!(
                "NIMBY import: created {} additional consecutive edges",
                consecutive_count
            );
        }

        // Apply geographic-aware layout only to new nodes (preserve existing positions)
        let geo_hints = build_geographic_hints(graph, data);
        leptos::logging::log!("NIMBY import: built geographic hints for {} stations", geo_hints.len());

        let settings = ProjectSettings {
            default_node_distance_grid_squares: config.station_spacing / GRID_SIZE,
            ..Default::default()
        };

        // Build edge usage map for spine detection (uses ALL lines, not just selected)
        let edge_usage = build_edge_usage_map(data, &station_id_to_node, graph);
        leptos::logging::log!("NIMBY import: built edge usage map with {} edges", edge_usage.len());

        auto_layout::apply_layout_with_edge_weights(
            graph,
            1000.0,
            &settings,
            Some(&geo_hints),
            &pinned_nodes,
            &edge_usage,
        );

        // Infrastructure mode: return empty vec (no Line objects created)
        return Ok(Vec::new());
    }

    // Schedules mode: Create routes for each selected line using pathfinding
    let mut lines = Vec::new();
    let mut edge_map: HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>> = HashMap::new();

    for (idx, nimby_line) in valid_lines.iter().enumerate() {
        let line = import_single_line(
            nimby_line,
            data,
            config,
            graph,
            &mut edge_map,
            existing_line_count + idx,
        )?;

        if let Some(l) = line {
            lines.push(l);
        }
    }

    Ok(lines)
}

/// Create infrastructure (tracks) based on the densest paths found in segment analysis
fn create_infrastructure_from_segments(
    graph: &mut RailwayGraph,
    segment_map: &SegmentMap,
    station_id_to_node: &HashMap<String, NodeIndex>,
    handedness: TrackHandedness,
) {
    // Track which edges we've already created to avoid duplicates
    let mut created_edges: std::collections::HashSet<(NodeIndex, NodeIndex)> =
        std::collections::HashSet::new();

    // For each segment in the map, create edges for the densest path's consecutive pairs
    for ((from_id, to_id), paths) in segment_map {
        // Get the densest path (most intermediate stations)
        let Some(densest) = paths.iter().max_by_key(|p| p.intermediates.len()) else {
            continue;
        };

        // Skip pairs with no intermediates - these are express-only pairs that should
        // use infrastructure from consecutive pairs created by create_consecutive_edges
        if densest.intermediates.is_empty() {
            continue;
        }

        // Build the full station sequence: from -> intermediates -> to
        let mut station_sequence = vec![from_id.as_str()];
        for intermediate in &densest.intermediates {
            station_sequence.push(intermediate.as_str());
        }
        station_sequence.push(to_id.as_str());

        // Create edges for consecutive pairs in this sequence
        for i in 0..(station_sequence.len() - 1) {
            let seg_from = station_sequence[i];
            let seg_to = station_sequence[i + 1];

            let Some(&from_node) = station_id_to_node.get(seg_from) else {
                continue;
            };
            let Some(&to_node) = station_id_to_node.get(seg_to) else {
                continue;
            };

            // Skip if edge already exists (in either direction)
            if created_edges.contains(&(from_node, to_node))
                || created_edges.contains(&(to_node, from_node))
                || graph.graph.find_edge(from_node, to_node).is_some()
                || graph.graph.find_edge(to_node, from_node).is_some()
            {
                continue;
            }

            // Check if THIS specific consecutive pair has a denser path in the segment map
            // This prevents creating express-skip edges when a denser local path exists
            let pair_key = (seg_from.to_string(), seg_to.to_string());
            let reverse_pair_key = (seg_to.to_string(), seg_from.to_string());
            let pair_has_denser_path = segment_map
                .get(&pair_key)
                .or_else(|| segment_map.get(&reverse_pair_key))
                .is_some_and(|paths| paths.iter().any(|p| !p.intermediates.is_empty()));

            if pair_has_denser_path {
                // Skip - this pair has its own denser path that will be processed separately
                continue;
            }

            // Create the edge
            let tracks = super::shared::create_tracks_with_count(1, handedness);
            graph.add_track(from_node, to_node, tracks);
            created_edges.insert((from_node, to_node));
        }
    }

    leptos::logging::log!("NIMBY import: created {} track segments", created_edges.len());
}

/// Create edges for consecutive station pairs, using the segment map to determine
/// whether a pair should get a direct edge or route through intermediates.
fn create_consecutive_edges(
    graph: &mut RailwayGraph,
    lines: &[&NimbyLine],
    station_id_to_node: &HashMap<String, NodeIndex>,
    segment_map: &SegmentMap,
    handedness: TrackHandedness,
) -> usize {
    let mut created_edges: std::collections::HashSet<(NodeIndex, NodeIndex)> =
        std::collections::HashSet::new();

    for line in lines {
        // Get actual stations with their original indices (needed for distance calculation)
        let stations: Vec<(usize, &NimbyStop)> = line
            .stops
            .iter()
            .enumerate()
            .filter(|(_, s)| s.station_id != "0x0")
            .collect();

        // Create edges for consecutive station pairs
        for window in stations.windows(2) {
            let (from_idx, from_stop) = &window[0];
            let (to_idx, to_stop) = &window[1];
            let from_id = &from_stop.station_id;
            let to_id = &to_stop.station_id;

            // Calculate total distance including any waypoints between stations
            let total_distance: f64 = line.stops[*from_idx + 1..=*to_idx]
                .iter()
                .map(|s| s.leg_distance)
                .sum();

            let Some(&from_node) = station_id_to_node.get(from_id) else {
                continue;
            };
            let Some(&to_node) = station_id_to_node.get(to_id) else {
                continue;
            };

            // Skip if edge already exists (either direction)
            if created_edges.contains(&(from_node, to_node))
                || created_edges.contains(&(to_node, from_node))
                || graph.graph.find_edge(from_node, to_node).is_some()
                || graph.graph.find_edge(to_node, from_node).is_some()
            {
                continue;
            }

            // Check segment map - skip if there's an intermediate path with matching distance
            // This ensures express trains route through local infrastructure when distances match
            // BUT: Skip this check for terminal stations (leg_distance=0 at turnarounds)
            // These MUST have edges created regardless of segment map analysis
            //
            // IMPORTANT: Only skip if the intermediate path is SHORTER or EQUAL to the direct path.
            // If the direct path is noticeably shorter, it indicates a dedicated high-speed track
            // (straighter route) that should get its own edge.
            if total_distance > 0.0 {
                let key = (from_id.clone(), to_id.clone());
                let reverse_key = (to_id.clone(), from_id.clone());

                let has_matching_intermediate_path = segment_map
                    .get(&key)
                    .or_else(|| segment_map.get(&reverse_key))
                    .is_some_and(|paths| {
                        paths.iter().any(|p| is_same_track_path(p, total_distance))
                    });

                if has_matching_intermediate_path {
                    continue;
                }
            }

            let tracks = super::shared::create_tracks_with_count(1, handedness);
            graph.add_track(from_node, to_node, tracks);
            created_edges.insert((from_node, to_node));
        }
    }

    created_edges.len()
}

/// Import a single line from NIMBY data
fn import_single_line(
    nimby_line: &NimbyLine,
    data: &NimbyImportData,
    config: &NimbyImportConfig,
    graph: &mut RailwayGraph,
    edge_map: &mut HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>>,
    color_seed: usize,
) -> Result<Option<Line>, String> {
    let mut forward_route: Vec<RouteSegment> = Vec::new();
    let mut prev_station: Option<(NodeIndex, &NimbyStop)> = None;

    // Detect turnaround (line returns to starting station)
    let turnaround_idx = detect_turnaround(&nimby_line.stops);

    // Track accumulated distance from skipped waypoints
    let mut accumulated_distance: f64 = 0.0;

    // Process stops to build route
    for (stop_idx, stop) in nimby_line.stops.iter().enumerate() {
        // Accumulate distance from waypoints
        if stop.station_id == "0x0" {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        // Total leg distance includes any skipped waypoints
        let total_leg_distance = accumulated_distance + stop.leg_distance;
        accumulated_distance = 0.0; // Reset for next segment

        // Build connection context if we have a previous station
        let connection = prev_station.map(|(prev_idx, _)| ConnectionContext {
            from_node: prev_idx,
            leg_distance: total_leg_distance,
        });

        // Get or create station (with connection context for better matching)
        let station_idx = find_or_create_station(
            graph,
            &stop.station_id,
            data,
            config.create_infrastructure,
            connection.as_ref(),
        )?;

        // Create edge(s) and route segment(s) if we have a previous station
        if let Some((prev_idx, prev_stop)) = prev_station {
            let Some(edges) = get_or_create_edges(
                graph,
                edge_map,
                prev_idx,
                station_idx,
                total_leg_distance,
            ) else {
                // No path found - skip this line
                leptos::logging::warn!(
                    "NIMBY import: No path from {:?} to {:?} for line '{}', skipping",
                    graph.graph[prev_idx].display_name(),
                    graph.graph[station_idx].display_name(),
                    nimby_line.name
                );
                return Ok(None);
            };

            // Calculate total travel time for this leg
            let total_travel_secs = if stop.arrival > prev_stop.departure {
                stop.arrival - prev_stop.departure
            } else {
                60 // Default 1 minute if timing doesn't make sense
            };

            // Calculate wait time (departure - arrival) - only applies to last edge
            let wait_duration = Duration::seconds((stop.departure - stop.arrival).max(0));

            // Distribute travel time proportionally across edges based on distance
            let total_distance: f64 = edges.iter()
                .filter_map(|&e| graph.graph.edge_weight(e))
                .filter_map(|seg| seg.distance)
                .sum();

            // Set distance on new single edge if we have distance data and it's not already set
            let should_set_distance = edges.len() == 1
                && total_leg_distance > 0.0
                && graph.graph.edge_weight(edges[0]).is_some_and(|s| s.distance.is_none());
            if should_set_distance {
                if let Some(track_segment) = graph.graph.edge_weight_mut(edges[0]) {
                    track_segment.distance = Some(total_leg_distance);
                }
            }

            for (i, &edge_idx) in edges.iter().enumerate() {
                let is_last = i == edges.len() - 1;

                // Calculate proportional travel time
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
                let edge_duration = if total_distance > 0.0 {
                    let edge_dist = graph.graph.edge_weight(edge_idx)
                        .and_then(|s| s.distance)
                        .unwrap_or(1.0);
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = (total_travel_secs as f64 * edge_dist / total_distance) as i64;
                    Duration::seconds(secs)
                } else {
                    // Equal distribution if no distance info
                    #[allow(clippy::cast_possible_truncation)]
                    Duration::seconds(total_travel_secs / edges.len() as i64)
                };

                forward_route.push(RouteSegment {
                    edge_index: edge_idx.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(edge_duration),
                    wait_time: if is_last { wait_duration } else { Duration::zero() },
                });
            }
        }

        prev_station = Some((station_idx, stop));

        // If we've reached turnaround point, stop forward route here
        if turnaround_idx == Some(stop_idx) {
            break;
        }
    }

    // Need at least one route segment
    if forward_route.is_empty() {
        return Ok(None);
    }

    // Create line with forward route
    let line_color = parse_nimby_color(&nimby_line.color);
    let line_code = if nimby_line.code.is_empty() {
        nimby_line.name.clone()
    } else {
        nimby_line.code.clone()
    };

    // Build return route if there's a turnaround
    let return_route = if let Some(turnaround) = turnaround_idx {
        build_return_route(&nimby_line.stops, turnaround, data, config, graph, edge_map)?
    } else {
        Vec::new()
    };

    let line = Line {
        id: uuid::Uuid::new_v4(),
        name: nimby_line.name.clone(),
        code: line_code,
        color: line_color,
        frequency: Duration::minutes(30),
        thickness: 2.0,
        first_departure: BASE_MIDNIGHT,
        return_first_departure: BASE_MIDNIGHT,
        last_departure: BASE_MIDNIGHT + Duration::hours(22),
        return_last_departure: BASE_MIDNIGHT + Duration::hours(22),
        visible: true,
        schedule_mode: ScheduleMode::Auto,
        days_of_week: crate::models::DaysOfWeek::default(),
        manual_departures: Vec::new(),
        forward_route,
        return_route,
        sync_routes: false, // Don't sync since we built both routes
        auto_train_number_format: "{line} {seq:04}".to_string(),
        default_wait_time: Duration::seconds(30),
        first_stop_wait_time: Duration::zero(),
        return_first_stop_wait_time: Duration::zero(),
        #[allow(clippy::cast_precision_loss)]
        sort_index: Some(color_seed as f64),
        sync_departure_offsets: false,
        folder_id: None,
        style: crate::models::LineStyle::default(),
        forward_turnaround: turnaround_idx.is_some(),
        return_turnaround: turnaround_idx.is_some(),
    };

    Ok(Some(line))
}

/// Connection context for station matching - helps disambiguate stations with same name
struct ConnectionContext {
    /// Node index of the station we're connecting from
    from_node: NodeIndex,
    /// Expected distance between the stations (leg distance from NIMBY)
    leg_distance: f64,
}

/// Find or create a station by NIMBY ID
///
/// Matching priority:
/// 1. `external_id` exact match
/// 2. Name match + edge distance match (if connection context provided)
/// 3. Name match only
fn find_or_create_station(
    graph: &mut RailwayGraph,
    nimby_id: &str,
    data: &NimbyImportData,
    create_if_missing: bool,
    connection: Option<&ConnectionContext>,
) -> Result<NodeIndex, String> {
    // First try to find by external_id
    for (idx, station) in graph.get_all_stations_ordered() {
        if station.external_id.as_deref() == Some(nimby_id) {
            return Ok(idx);
        }
    }

    // Get station data from NIMBY
    let nimby_station = data.stations.get(nimby_id)
        .ok_or_else(|| format!("Station ID {nimby_id} not found in NIMBY data"))?;

    // Only try name matching in schedules mode (create_if_missing=false)
    // In infrastructure mode, each NIMBY ID represents a distinct physical station,
    // so we should NOT merge stations that happen to share the same name
    if !create_if_missing {
        // Collect all stations with matching name
        let name_matches: Vec<NodeIndex> = graph
            .get_all_stations_ordered()
            .into_iter()
            .filter(|(_, station)| station.name == nimby_station.name)
            .map(|(idx, _)| idx)
            .collect();

        // If we have connection context, prefer matches where edge distance also matches
        if let Some(ctx) = connection {
            for idx in &name_matches {
                if edge_distance_matches(graph, ctx.from_node, *idx, ctx.leg_distance) {
                    return Ok(*idx);
                }
            }
        }

        // Fall back to first name match (without edge distance check)
        if let Some(idx) = name_matches.first() {
            return Ok(*idx);
        }

        return Err(format!(
            "Station '{}' not found in existing infrastructure",
            nimby_station.name
        ));
    }

    // Create new station
    let station = StationNode {
        name: nimby_station.name.clone(),
        external_id: Some(nimby_id.to_string()),
        position: None,
        passing_loop: false,
        platforms: vec![
            crate::models::Platform { name: "1".to_string() },
            crate::models::Platform { name: "2".to_string() },
        ],
        label_position: None,
    };
    let idx = graph.graph.add_node(Node::Station(station));
    Ok(idx)
}

/// Distance matching tolerance (10%)
const DISTANCE_TOLERANCE_PERCENT: f64 = 0.10;
/// Minimum distance tolerance in meters
const DISTANCE_TOLERANCE_MIN_METERS: f64 = 100.0;

/// Check if an edge exists between two nodes with distance matching the expected value
fn edge_distance_matches(
    graph: &RailwayGraph,
    from: NodeIndex,
    to: NodeIndex,
    expected_distance: f64,
) -> bool {
    let edge = graph.graph.find_edge(from, to).or_else(|| graph.graph.find_edge(to, from));
    if let Some(edge_idx) = edge {
        if let Some(segment) = graph.graph.edge_weight(edge_idx) {
            if let Some(distance) = segment.distance {
                return distances_match(distance, expected_distance);
            }
        }
    }
    false
}

/// Grid size in pixels for snapping
const GRID_SIZE: f64 = 30.0;

/// A path between two stations with intermediate stops and distance info
#[derive(Debug, Clone)]
struct SegmentPath {
    /// Intermediate station IDs between start and end (not including start/end)
    intermediates: Vec<String>,
    /// Total distance for this segment (sum of `leg_distance` values)
    total_distance: f64,
}

/// Map from station ID pairs to different paths found across all lines
type SegmentMap = HashMap<(String, String), Vec<SegmentPath>>;

/// Check if two distances match within tolerance (10% or 100m, whichever is greater)
fn distances_match(d1: f64, d2: f64) -> bool {
    let diff = (d1 - d2).abs();
    let tolerance = (d1.max(d2) * DISTANCE_TOLERANCE_PERCENT).max(DISTANCE_TOLERANCE_MIN_METERS);
    diff < tolerance
}

/// Check if an intermediate path matches the direct distance and is not a shortcut bypass.
/// Returns true if the intermediate path is equivalent to the same physical track.
fn is_same_track_path(path: &SegmentPath, direct_distance: f64) -> bool {
    !path.intermediates.is_empty()
        && distances_match(path.total_distance, direct_distance)
        // Only consider it the same track if intermediate path is not longer
        // (a longer intermediate path suggests a more circuitous local route,
        // while the direct route may be a dedicated high-speed shortcut)
        && path.total_distance <= direct_distance * 1.02
}

/// Add a path to the segment map if it's not dominated by an existing path.
/// A path is dominated if there's another with same distance but more intermediates.
fn add_path_to_segment_map(paths: &mut Vec<SegmentPath>, path: SegmentPath) {
    // Check if dominated by an existing path
    let dominated = paths.iter().any(|existing| {
        distances_match(existing.total_distance, path.total_distance)
            && existing.intermediates.len() >= path.intermediates.len()
    });

    if dominated {
        return;
    }

    // Remove any paths this one dominates (same distance, fewer intermediates)
    let path_dist = path.total_distance;
    let path_len = path.intermediates.len();
    paths.retain(|existing| {
        !distances_match(existing.total_distance, path_dist)
            || existing.intermediates.len() >= path_len
    });
    paths.push(path);
}

/// Build a segment map analyzing all lines to find paths between station pairs
///
/// For each pair of stations (A, D) that appear across multiple lines, this records
/// all the different paths found (with intermediates and distances). This allows us
/// to later pick the "densest" path (most intermediate stations) for infrastructure.
fn build_segment_map(lines: &[&NimbyLine]) -> SegmentMap {
    let mut map = SegmentMap::new();

    for line in lines {
        // Get all actual station stops (exclude waypoints with station_id "0x0")
        let stations: Vec<(usize, &NimbyStop)> = line.stops.iter()
            .enumerate()
            .filter(|(_, s)| s.station_id != "0x0")
            .collect();

        // For each pair of stations in this line, record the path
        for i in 0..stations.len() {
            for j in (i + 1)..stations.len() {
                let (from_idx, from_stop) = &stations[i];
                let (to_idx, _) = &stations[j];
                let from_id = &from_stop.station_id;
                let to_id = &stations[j].1.station_id;

                // Collect intermediate station IDs
                let intermediates: Vec<String> = stations[i + 1..j]
                    .iter()
                    .map(|(_, s)| s.station_id.clone())
                    .collect();

                // Skip if this path crosses a turnaround (contains duplicate stations or the endpoints)
                // This avoids creating nonsensical paths like "A via B,C,D,C,B to E"
                let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
                seen.insert(from_id.as_str());
                seen.insert(to_id.as_str());
                let has_duplicates = intermediates.iter().any(|id| !seen.insert(id.as_str()));
                if has_duplicates {
                    continue;
                }

                // Sum distances for all legs between these stations (including waypoints)
                let total_distance: f64 = line.stops[*from_idx + 1..=*to_idx]
                    .iter()
                    .map(|s| s.leg_distance)
                    .sum();

                let path = SegmentPath { intermediates, total_distance };
                let key = (from_id.clone(), to_id.clone());
                let paths = map.entry(key).or_default();
                add_path_to_segment_map(paths, path);
            }
        }
    }
    map
}

/// Build geographic hints from NIMBY lonlat data for use with `auto_layout`
fn build_geographic_hints(
    graph: &RailwayGraph,
    data: &NimbyImportData,
) -> GeographicHints {
    let mut lonlat_map = HashMap::new();

    for node_idx in graph.graph.node_indices() {
        if let Some(station) = graph.graph.node_weight(node_idx).and_then(|n| n.as_station()) {
            if let Some(ext_id) = &station.external_id {
                if let Some(nimby_station) = data.stations.get(ext_id) {
                    lonlat_map.insert(node_idx, nimby_station.lonlat);
                }
            }
        }
    }

    GeographicHints::new(lonlat_map)
}

/// Check if two distances match within tolerance (10% or 100m, whichever is greater)
fn distance_matches(existing: Option<f64>, nimby_distance: f64) -> bool {
    match existing {
        Some(d) if nimby_distance > 0.0 => {
            let diff = (d - nimby_distance).abs();
            let tolerance = (nimby_distance * DISTANCE_TOLERANCE_PERCENT).max(DISTANCE_TOLERANCE_MIN_METERS);
            diff < tolerance
        }
        _ => false,
    }
}

/// Calculate total distance of a path
fn calculate_path_distance(graph: &RailwayGraph, path: &[EdgeIndex]) -> f64 {
    path.iter()
        .filter_map(|&e| graph.graph.edge_weight(e))
        .filter_map(|seg| seg.distance)
        .sum()
}

/// Get existing edges between two stations using pathfinding
/// Returns None if no path exists (infrastructure should have been created in earlier phases)
fn get_or_create_edges(
    graph: &RailwayGraph,
    edge_map: &mut HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>>,
    from: NodeIndex,
    to: NodeIndex,
    leg_distance: f64,
) -> Option<Vec<EdgeIndex>> {
    // Check cache first
    if let Some(edges) = edge_map.get(&(from, to)) {
        return Some(edges.clone());
    }

    // Try pathfinding - infrastructure should already exist from segment analysis
    if let Some(path) = graph.find_path_between_nodes(from, to) {
        let path_distance = calculate_path_distance(graph, &path);

        // Use this path if distance matches (within tolerance) or if we don't have distance info
        if leg_distance <= 0.0 || distance_matches(Some(path_distance), leg_distance) {
            edge_map.insert((from, to), path.clone());
            return Some(path);
        }

        // Distance doesn't match - check if there's a direct edge that matches better
        let direct_edge = graph.graph.find_edge(from, to).or_else(|| graph.graph.find_edge(to, from));
        if let Some(edge) = direct_edge {
            let direct_matches = graph.graph.edge_weight(edge)
                .is_some_and(|seg| distance_matches(seg.distance, leg_distance));
            if direct_matches {
                let result = vec![edge];
                edge_map.insert((from, to), result.clone());
                return Some(result);
            }
        }

        // No better match found - use the path we found anyway (express lines share local tracks)
        edge_map.insert((from, to), path.clone());
        return Some(path);
    }

    // No path found - check for direct edge
    let direct_edge = graph.graph.find_edge(from, to).or_else(|| graph.graph.find_edge(to, from));
    if let Some(edge) = direct_edge {
        let result = vec![edge];
        edge_map.insert((from, to), result.clone());
        return Some(result);
    }

    // No path and no direct edge - infrastructure should have been created
    None
}

/// Detect if a line returns to its starting station (turnaround)
fn detect_turnaround(stops: &[NimbyStop]) -> Option<usize> {
    if stops.len() < 3 {
        return None;
    }

    let first_station = &stops[0].station_id;
    if first_station == "0x0" {
        return None;
    }

    // Find where the line returns to the starting station
    stops.iter().enumerate().skip(2).find_map(|(idx, stop)| {
        if &stop.station_id == first_station {
            Some(idx)
        } else {
            None
        }
    })
}

/// Build return route from turnaround point back to start
fn build_return_route(
    stops: &[NimbyStop],
    turnaround_idx: usize,
    data: &NimbyImportData,
    config: &NimbyImportConfig,
    graph: &mut RailwayGraph,
    edge_map: &mut HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>>,
) -> Result<Vec<RouteSegment>, String> {
    let mut return_route = Vec::new();
    let mut prev_station: Option<(NodeIndex, &NimbyStop)> = None;
    let mut accumulated_distance: f64 = 0.0;

    // Process stops from turnaround to end
    for stop in stops.iter().skip(turnaround_idx) {
        // Accumulate distance from waypoints
        if stop.station_id == "0x0" {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        // Total leg distance includes any skipped waypoints
        let total_leg_distance = accumulated_distance + stop.leg_distance;
        accumulated_distance = 0.0;

        // Build connection context if we have a previous station
        let connection = prev_station.map(|(prev_idx, _)| ConnectionContext {
            from_node: prev_idx,
            leg_distance: total_leg_distance,
        });

        let station_idx = find_or_create_station(
            graph,
            &stop.station_id,
            data,
            config.create_infrastructure,
            connection.as_ref(),
        )?;

        if let Some((prev_idx, prev_stop)) = prev_station {
            let Some(edges) = get_or_create_edges(
                graph,
                edge_map,
                prev_idx,
                station_idx,
                total_leg_distance,
            ) else {
                return Err(format!(
                    "No path from {} to {} in return route",
                    graph.graph[prev_idx].display_name(),
                    graph.graph[station_idx].display_name()
                ));
            };

            // Calculate total travel time for this leg
            let total_travel_secs = if stop.arrival > prev_stop.departure {
                stop.arrival - prev_stop.departure
            } else {
                60
            };

            let wait_duration = Duration::seconds((stop.departure - stop.arrival).max(0));

            // Distribute travel time proportionally across edges based on distance
            let total_distance: f64 = edges.iter()
                .filter_map(|&e| graph.graph.edge_weight(e))
                .filter_map(|seg| seg.distance)
                .sum();

            for (i, &edge_idx) in edges.iter().enumerate() {
                let is_last = i == edges.len() - 1;

                // Calculate proportional travel time
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
                let edge_duration = if total_distance > 0.0 {
                    let edge_dist = graph.graph.edge_weight(edge_idx)
                        .and_then(|s| s.distance)
                        .unwrap_or(1.0);
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = (total_travel_secs as f64 * edge_dist / total_distance) as i64;
                    Duration::seconds(secs)
                } else {
                    #[allow(clippy::cast_possible_truncation)]
                    Duration::seconds(total_travel_secs / edges.len() as i64)
                };

                return_route.push(RouteSegment {
                    edge_index: edge_idx.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(edge_duration),
                    wait_time: if is_last { wait_duration } else { Duration::zero() },
                });
            }
        }

        prev_station = Some((station_idx, stop));
    }

    Ok(return_route)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_nimby_color() {
        // Format: 0xAABBGGRR → #RRGGBB
        assert_eq!(parse_nimby_color("0xff1a35fc"), "#fc351a"); // ABGR fc351a → RGB fc351a (red)
        assert_eq!(parse_nimby_color("0xfffc351a"), "#1a35fc"); // ABGR 1a35fc → RGB 1a35fc (blue)
        assert_eq!(parse_nimby_color("0xff18f0ff"), "#fff018"); // ABGR fff018 → RGB fff018 (yellow)
        assert_eq!(parse_nimby_color("invalid"), "#808080");
    }

    #[test]
    fn test_parse_basic_json() {
        let json = r#"[
            {"class":"ExportMeta", "company_name":"Test Co", "model_version":1},
            {"class":"Station", "id":"0x1", "name":"Station A", "lonlat":[10.0, 59.0]},
            {"class":"Station", "id":"0x2", "name":"Station B", "lonlat":[11.0, 60.0]}
        ]"#;

        let data = parse_nimby_json(json).unwrap();
        assert_eq!(data.company_name, "Test Co");
        assert_eq!(data.stations.len(), 2);
        assert_eq!(data.stations.get("0x1").unwrap().name, "Station A");
    }

    #[test]
    fn test_parse_line_with_stops() {
        let json = r#"[
            {"class":"Station", "id":"0x1", "name":"A", "lonlat":[10.0, 59.0]},
            {"class":"Station", "id":"0x2", "name":"B", "lonlat":[11.0, 60.0]},
            {"class":"Line", "id":"0x100", "name":"Test Line", "code":"T1", "color":"0xff112233", "stops":[
                {"class":"Stop", "idx":0, "leg_distance":0, "station_id":"0x1", "arrival":0, "departure":60},
                {"class":"Stop", "idx":1, "leg_distance":1000, "station_id":"0x2", "arrival":120, "departure":180}
            ]}
        ]"#;

        let data = parse_nimby_json(json).unwrap();
        assert_eq!(data.lines.len(), 1);
        assert_eq!(data.lines[0].name, "Test Line");
        assert_eq!(data.lines[0].stops.len(), 2);
        assert_eq!(data.lines[0].stops[0].station_id, "0x1");
    }

    #[test]
    fn test_line_summary() {
        let json = r#"[
            {"class":"Station", "id":"0x1", "name":"A", "lonlat":[10.0, 59.0]},
            {"class":"Line", "id":"0x100", "name":"Test", "code":"T1", "color":"0xff112233", "stops":[
                {"class":"Stop", "idx":0, "leg_distance":0, "station_id":"0x1", "arrival":0, "departure":60},
                {"class":"Stop", "idx":1, "leg_distance":1000, "station_id":"0x0", "arrival":120, "departure":120}
            ]}
        ]"#;

        let data = parse_nimby_json(json).unwrap();
        let summaries = data.get_line_summaries();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].stop_count, 2);
        assert_eq!(summaries[0].station_count, 1); // 0x0 is not a station
        assert_eq!(summaries[0].color, "#332211"); // ABGR 0xff112233 -> RGB #332211
    }
}
