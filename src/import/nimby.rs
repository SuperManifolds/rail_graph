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

/// Track area within a stop (contains platform info)
#[derive(Debug, Clone, Deserialize)]
pub struct NimbyTrackArea {
    pub platform_name: String,
}

/// Raw NIMBY Rails stop within a line
#[derive(Debug, Clone, Deserialize)]
pub struct NimbyStop {
    pub idx: usize,
    pub leg_distance: f64,
    pub station_id: String,
    pub arrival: i64,
    pub departure: i64,
    #[serde(default)]
    pub areas: Vec<Vec<NimbyTrackArea>>,
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

/// A single run (trip) within a shift
#[derive(Debug, Clone, Deserialize)]
pub struct NimbyRun {
    pub line_id: String,
    pub enter_stop_idx: usize,
    pub exit_stop_idx: usize,
    /// Alternating arrival/departure times in seconds: [arr0, dep0, arr1, dep1, ...]
    pub arrival_departure: Vec<i64>,
}

/// A shift containing multiple runs (operated by one train)
#[derive(Debug, Clone, Deserialize)]
pub struct NimbyShift {
    pub id: String,
    pub name: String,
    pub runs: Vec<NimbyRun>,
}

/// A schedule containing shifts
#[derive(Debug, Clone, Deserialize)]
pub struct NimbySchedule {
    pub id: String,
    pub name: String,
    pub shifts: Vec<NimbyShift>,
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
    Schedule {
        id: String,
        name: String,
        shifts: Vec<NimbyShift>,
    },
    #[serde(other)]
    Other,
}

/// Parsed and categorized import data
#[derive(Debug, Default, Clone)]
pub struct NimbyImportData {
    pub company_name: String,
    pub stations: HashMap<String, NimbyStation>,
    pub lines: Vec<NimbyLine>,
    pub schedules: Vec<NimbySchedule>,
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
            NimbyRecord::Schedule { id, name, shifts } => {
                data.schedules.push(NimbySchedule { id, name, shifts });
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
    /// Update existing lines instead of creating new ones (match by line code)
    pub update_existing: bool,
}

impl Default for NimbyImportConfig {
    fn default() -> Self {
        Self {
            create_infrastructure: true,
            selected_line_ids: Vec::new(),
            handedness: TrackHandedness::RightHand,
            station_spacing: 2.0 * GRID_SIZE, // default 2 grid squares
            update_existing: false,
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

        // Get valid stops (non-null station IDs that exist in graph, excluding depots)
        let valid_stops: Vec<_> = nimby_line
            .stops
            .iter()
            .filter(|s| s.station_id != "0x0" && !is_depot(data, &s.station_id))
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
/// * `existing_lines` - Existing lines to update (when `config.update_existing` is true)
///
/// # Returns
/// Vector of created Line objects (empty if updating existing lines)
///
/// # Errors
/// Returns an error if import fails (e.g., station not found in pathfinding mode)
pub fn import_nimby_lines(
    data: &NimbyImportData,
    config: &NimbyImportConfig,
    graph: &mut RailwayGraph,
    existing_line_count: usize,
    mut existing_lines: Option<&mut Vec<Line>>,
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
        let segment_map = build_segment_map(&valid_lines, data);
        leptos::logging::log!("NIMBY import: analyzed {} segment pairs", segment_map.len());

        // Phase 2: Create all stations first (excluding depots)
        let mut station_id_to_node: HashMap<String, NodeIndex> = HashMap::new();
        for nimby_line in &valid_lines {
            for stop in &nimby_line.stops {
                if stop.station_id != "0x0"
                    && !is_depot(data, &stop.station_id)
                    && !station_id_to_node.contains_key(&stop.station_id)
                {
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
            data,
        );
        if consecutive_count > 0 {
            leptos::logging::log!(
                "NIMBY import: created {} additional consecutive edges",
                consecutive_count
            );
        }

        // Phase 4: Detect and create passing loops using segment map
        let passing_loop_count = create_passing_loops_from_segment_map(
            graph,
            &segment_map,
            &station_id_to_node,
            config.handedness,
        );
        if passing_loop_count > 0 {
            leptos::logging::log!(
                "NIMBY import: created {} passing loops",
                passing_loop_count
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
    let mut new_lines = Vec::new();
    let mut edge_map: HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>> = HashMap::new();

    for (idx, nimby_line) in valid_lines.iter().enumerate() {
        let nimby_code = if nimby_line.code.is_empty() {
            &nimby_line.name
        } else {
            &nimby_line.code
        };

        // If update mode is enabled, try to find and update an existing line
        if config.update_existing {
            if let Some(ref mut lines) = existing_lines.as_deref_mut() {
                if let Some(existing_line) = lines.iter_mut().find(|l| l.code == *nimby_code) {
                    leptos::logging::log!("Updating existing line: {}", nimby_code);
                    update_existing_line(existing_line, nimby_line, data, config, graph, &mut edge_map)?;
                    continue;
                }
            }
        }

        // Create new line if not updating or no match found
        let line = import_single_line(
            nimby_line,
            data,
            config,
            graph,
            &mut edge_map,
            existing_line_count + idx,
        )?;

        if let Some(l) = line {
            new_lines.push(l);
        }
    }

    Ok(new_lines)
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

            // Skip if edge already exists (in either direction, or through passing loops)
            if created_edges.contains(&(from_node, to_node))
                || created_edges.contains(&(to_node, from_node))
                || connection_exists(graph, from_node, to_node)
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

            // Look up distance from segment_map for this consecutive pair
            let pair_key = (seg_from.to_string(), seg_to.to_string());
            let reverse_key = (seg_to.to_string(), seg_from.to_string());
            let distance = segment_map
                .get(&pair_key)
                .or_else(|| segment_map.get(&reverse_key))
                .and_then(|paths| paths.first())
                .map(|p| p.total_distance / METERS_PER_KM);

            // Create the edge
            let tracks = super::shared::create_tracks_with_count(1, handedness);
            graph.add_track(from_node, to_node, tracks, distance);
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
    data: &NimbyImportData,
) -> usize {
    let mut created_edges: std::collections::HashSet<(NodeIndex, NodeIndex)> =
        std::collections::HashSet::new();

    for line in lines {
        // Get actual stations with their original indices (needed for distance calculation)
        // Exclude waypoints and depots
        let stations: Vec<(usize, &NimbyStop)> = line
            .stops
            .iter()
            .enumerate()
            .filter(|(_, s)| s.station_id != "0x0" && !is_depot(data, &s.station_id))
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

            // Skip if edge already exists (either direction, or through passing loops)
            if created_edges.contains(&(from_node, to_node))
                || created_edges.contains(&(to_node, from_node))
                || connection_exists(graph, from_node, to_node)
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
            let distance = if total_distance > 0.0 { Some(total_distance / METERS_PER_KM) } else { None };
            graph.add_track(from_node, to_node, tracks, distance);
            created_edges.insert((from_node, to_node));
        }
    }

    created_edges.len()
}

/// A passing loop candidate with its distance ratio normalized to canonical station ordering
struct NormalizedCandidate<'a> {
    candidate: &'a PassingLoopCandidate,
    /// Distance ratio normalized to canonical ordering (from first to second station alphabetically)
    normalized_ratio: f64,
}

/// Detect passing loops from segment map and create them in the graph.
///
/// A passing loop is detected when a segment A→B has waypoints at positions
/// that match (within tolerance) the normalized waypoint positions of segment B→A.
///
/// Returns the number of passing loops created.
#[allow(clippy::too_many_lines)]
fn create_passing_loops_from_segment_map(
    graph: &mut RailwayGraph,
    segment_map: &SegmentMap,
    station_id_to_node: &HashMap<String, NodeIndex>,
    handedness: TrackHandedness,
) -> usize {
    // Find passing loop candidates by comparing A→B with B→A segments
    let mut candidates: Vec<PassingLoopCandidate> = Vec::new();
    let mut processed: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();

    for ((from_id, to_id), forward_paths) in segment_map {
        // Skip if already processed this pair
        if processed.contains(&(from_id.clone(), to_id.clone()))
            || processed.contains(&(to_id.clone(), from_id.clone()))
        {
            continue;
        }

        // Look for reverse segment B→A
        let reverse_key = (to_id.clone(), from_id.clone());
        let Some(reverse_paths) = segment_map.get(&reverse_key) else {
            continue;
        };

        processed.insert((from_id.clone(), to_id.clone()));
        processed.insert((to_id.clone(), from_id.clone()));

        // Get the path with waypoints (consecutive pairs have waypoint data)
        let forward_path = forward_paths.iter().find(|p| !p.waypoint_distances.is_empty());
        let reverse_path = reverse_paths.iter().find(|p| !p.waypoint_distances.is_empty());

        let (Some(fwd), Some(rev)) = (forward_path, reverse_path) else {
            continue;
        };

        // Check distances match (same physical track)
        if !distances_match(fwd.total_distance, rev.total_distance) {
            continue;
        }

        let segment_distance = fwd.total_distance;

        // Filter waypoints that are too close to either station (likely station throat routing)
        let valid_fwd_waypoints: Vec<f64> = fwd
            .waypoint_distances
            .iter()
            .filter(|&&d| {
                d >= MIN_PASSING_LOOP_DISTANCE_FROM_STATION
                    && (fwd.total_distance - d) >= MIN_PASSING_LOOP_DISTANCE_FROM_STATION
            })
            .copied()
            .collect();

        let valid_rev_waypoints: Vec<f64> = rev
            .waypoint_distances
            .iter()
            .filter(|&&d| {
                d >= MIN_PASSING_LOOP_DISTANCE_FROM_STATION
                    && (rev.total_distance - d) >= MIN_PASSING_LOOP_DISTANCE_FROM_STATION
            })
            .copied()
            .collect();

        // Normalize reverse waypoint positions (B→A becomes distance from A)
        let normalized_reverse: Vec<f64> = valid_rev_waypoints
            .iter()
            .map(|&d| rev.total_distance - d)
            .collect();

        // Match forward waypoints with normalized reverse waypoints
        // Use a generous tolerance (700m) since waypoint positions can vary
        for &fwd_dist in &valid_fwd_waypoints {
            for &rev_normalized in &normalized_reverse {
                let diff = (fwd_dist - rev_normalized).abs();
                if diff < PASSING_LOOP_WAYPOINT_TOLERANCE {
                    let distance_ratio = fwd_dist / segment_distance;
                    candidates.push(PassingLoopCandidate {
                        between_stations: (from_id.clone(), to_id.clone()),
                        distance_ratio,
                        segment_distance,
                    });
                    break; // One match per forward waypoint
                }
            }
        }
    }

    if candidates.is_empty() {
        return 0;
    }

    // Deduplicate: same segment + similar normalized distance ratio = same passing loop
    let mut unique_loops: HashMap<(String, String, i32), PassingLoopCandidate> = HashMap::new();

    for candidate in candidates {
        // Use canonical ordering so A→B and B→A deduplicate to the same key
        let (a, b) = &candidate.between_stations;
        let (canonical_pair, needs_flip) = if a < b {
            ((a.clone(), b.clone()), false)
        } else {
            ((b.clone(), a.clone()), true)
        };
        // Normalize the ratio to canonical ordering for deduplication
        let normalized_ratio = if needs_flip {
            1.0 - candidate.distance_ratio
        } else {
            candidate.distance_ratio
        };
        #[allow(clippy::cast_possible_truncation)]
        let quantized_ratio = (normalized_ratio * 20.0).round() as i32;
        let key = (canonical_pair.0, canonical_pair.1, quantized_ratio);
        unique_loops.entry(key).or_insert(candidate);
    }

    leptos::logging::log!(
        "Passing loop detection: found {} unique candidates",
        unique_loops.len()
    );

    // Group loops by segment (canonical station pair) with normalized distance ratios
    let mut loops_by_segment: HashMap<(String, String), Vec<NormalizedCandidate>> = HashMap::new();
    for candidate in unique_loops.values() {
        let (a, b) = &candidate.between_stations;
        let (canonical, needs_flip) = if a < b {
            ((a.clone(), b.clone()), false)
        } else {
            ((b.clone(), a.clone()), true)
        };
        // If the candidate was stored as B→A but canonical is A→B, flip the ratio
        let normalized_ratio = if needs_flip {
            1.0 - candidate.distance_ratio
        } else {
            candidate.distance_ratio
        };
        loops_by_segment.entry(canonical).or_default().push(NormalizedCandidate {
            candidate,
            normalized_ratio,
        });
    }

    let mut loop_count = 0;

    // Process each segment's loops together, sorted by normalized distance ratio
    for ((station_a, station_b), mut segment_loops) in loops_by_segment {
        // Sort by normalized distance ratio so we process loops in order along the segment
        segment_loops.sort_by(|a, b| {
            a.normalized_ratio
                .partial_cmp(&b.normalized_ratio)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Get node indices for both stations
        let (Some(&from_node), Some(&to_node)) = (
            station_id_to_node.get(&station_a),
            station_id_to_node.get(&station_b),
        ) else {
            continue;
        };

        // Check for existing passing loops and filter out candidates that already exist
        let existing_loops = find_existing_passing_loops(graph, from_node, to_node);
        if !existing_loops.is_empty() {
            // Filter out candidates that match existing loops (within tolerance)
            segment_loops.retain(|candidate| {
                !existing_loops.iter().any(|(_, existing_ratio)| {
                    (candidate.normalized_ratio - existing_ratio).abs() < 0.05
                })
            });

            // If all candidates already exist, skip this segment
            if segment_loops.is_empty() {
                continue;
            }

            // There are new candidates to add, but we can't easily insert into existing chain
            // For now, log a warning and skip - this handles the common case of duplicate imports
            leptos::logging::log!(
                "Warning: Cannot add new passing loops to segment with existing loops: {} -> {}",
                station_a,
                station_b
            );
            continue;
        }

        // Find the existing edge between these stations
        let edge_to_split = graph
            .graph
            .find_edge(from_node, to_node)
            .or_else(|| graph.graph.find_edge(to_node, from_node));

        let Some(edge_idx) = edge_to_split else {
            continue;
        };

        // Get total segment distance from edge (already in km) or first candidate (in meters, convert to km)
        let edge_weight = graph.graph.edge_weight(edge_idx).cloned();
        let total_distance_km = edge_weight
            .as_ref()
            .and_then(|w| w.distance)
            .unwrap_or_else(|| segment_loops[0].candidate.segment_distance / METERS_PER_KM);

        // Check if edge direction matches our canonical from→to ordering
        let edge_endpoints = graph.graph.edge_endpoints(edge_idx);
        let edge_goes_from_to = edge_endpoints == Some((from_node, to_node));

        // Remove the original edge
        graph.graph.remove_edge(edge_idx);

        // Determine chain direction based on original edge direction
        // If edge was A→B, keep ratios as-is (from A)
        // If edge was B→A, flip ratios (measure from B instead) and reverse order
        let (chain_start, chain_end, flip_ratios) = if edge_goes_from_to {
            (from_node, to_node, false)
        } else {
            (to_node, from_node, true)
        };

        // If we need to flip, reverse the iteration order so loops are created in the right sequence
        // For edge A→B with ratios [0.3, 0.6]: A → Loop@0.3 → Loop@0.6 → B
        // For edge B→A with ratios [0.3, 0.6]: B → Loop@(1-0.6) → Loop@(1-0.3) → A
        //   which means iterate in reverse: [0.6, 0.3] → flip to [0.4, 0.7]
        let loop_iter: Box<dyn Iterator<Item = &NormalizedCandidate>> = if flip_ratios {
            Box::new(segment_loops.iter().rev())
        } else {
            Box::new(segment_loops.iter())
        };

        // Create all loops and edges for this segment (all distances in km)
        let mut prev_node = chain_start;
        let mut prev_distance_km = 0.0;

        for norm_candidate in loop_iter {
            // If we're going in the opposite direction, flip the ratio
            let effective_ratio = if flip_ratios {
                1.0 - norm_candidate.normalized_ratio
            } else {
                norm_candidate.normalized_ratio
            };
            let loop_distance_km = total_distance_km * effective_ratio;

            // Create the passing loop station
            let loop_station = StationNode {
                name: "Passing Loop".to_string(),
                external_id: None,
                position: None,
                passing_loop: true,
                platforms: vec![
                    crate::models::Platform { name: "1".to_string() },
                    crate::models::Platform { name: "2".to_string() },
                ],
                label_position: None,
            };
            let loop_node = graph.graph.add_node(Node::Station(loop_station));

            // Create edge from previous node to this loop (distances already in km)
            let edge_distance_km = loop_distance_km - prev_distance_km;
            let tracks = super::shared::create_tracks_with_count(1, handedness);
            graph.add_track(prev_node, loop_node, tracks, Some(edge_distance_km));

            prev_node = loop_node;
            prev_distance_km = loop_distance_km;
            loop_count += 1;
        }

        // Create final edge from last loop to destination station (distances already in km)
        let final_distance_km = total_distance_km - prev_distance_km;
        let tracks = super::shared::create_tracks_with_count(1, handedness);
        graph.add_track(prev_node, chain_end, tracks, Some(final_distance_km));
    }

    loop_count
}

/// Import a single line from NIMBY data
#[allow(clippy::too_many_lines)]
fn import_single_line(
    nimby_line: &NimbyLine,
    data: &NimbyImportData,
    config: &NimbyImportConfig,
    graph: &mut RailwayGraph,
    edge_map: &mut HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>>,
    color_seed: usize,
) -> Result<Option<Line>, String> {
    let mut forward_route: Vec<RouteSegment> = Vec::new();
    let mut prev_station: Option<(NodeIndex, &NimbyStop, usize)> = None; // (idx, stop, platform_idx)
    let mut first_stop_wait_time = Duration::zero();

    // Detect turnaround (furthest point before line starts returning)
    let turnaround_idx = detect_turnaround(&nimby_line.stops, data);

    // Track accumulated distance from skipped waypoints
    let mut accumulated_distance: f64 = 0.0;

    // Process stops to build route
    for (stop_idx, stop) in nimby_line.stops.iter().enumerate() {
        // Accumulate distance from waypoints
        if stop.station_id == "0x0" {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        // Skip depots, but accumulate their distance
        if is_depot(data, &stop.station_id) {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        // Total leg distance includes any skipped waypoints and depots
        let total_leg_distance = accumulated_distance + stop.leg_distance;
        accumulated_distance = 0.0; // Reset for next segment

        // Build connection context if we have a previous station
        let connection = prev_station.map(|(prev_idx, _, _)| ConnectionContext {
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

        // Add platform to station if present in stop data and get its index
        let platform_idx = get_platform_from_stop(stop)
            .map_or(0, |name| super::shared::get_or_add_platform(graph, station_idx, &name));

        // Capture first stop wait time
        if prev_station.is_none() {
            first_stop_wait_time = Duration::seconds((stop.departure - stop.arrival).max(0));
        }

        // Create edge(s) and route segment(s) if we have a previous station
        if let Some((prev_idx, prev_stop, prev_platform_idx)) = prev_station {
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
                    track_segment.distance = Some(total_leg_distance / METERS_PER_KM);
                }
            }

            // Track remaining time for multi-edge paths to avoid truncation loss
            let mut remaining_secs = total_travel_secs;

            for (i, &edge_idx) in edges.iter().enumerate() {
                let is_last = i == edges.len() - 1;

                // Calculate travel time for this edge
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
                let edge_duration = if edges.len() == 1 {
                    // Single edge: use exact NIMBY timing (no distribution needed)
                    Duration::seconds(total_travel_secs)
                } else if is_last {
                    // Last edge gets remaining time (avoids truncation loss)
                    Duration::seconds(remaining_secs)
                } else if total_distance > 0.0 {
                    // Distribute proportionally by distance
                    let edge_dist = graph.graph.edge_weight(edge_idx)
                        .and_then(|s| s.distance)
                        .unwrap_or(1.0);
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = (total_travel_secs as f64 * edge_dist / total_distance) as i64;
                    remaining_secs -= secs;
                    Duration::seconds(secs)
                } else {
                    // Equal distribution if no distance info
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = total_travel_secs / edges.len() as i64;
                    remaining_secs -= secs;
                    Duration::seconds(secs)
                };

                // Determine if we're traveling backward relative to the edge orientation
                // (i.e., from target to source instead of source to target)
                let (edge_source, edge_target) = graph.graph.edge_endpoints(edge_idx)
                    .expect("edge should exist");
                let traveling_backward = prev_idx == edge_target && station_idx == edge_source;
                let track_index = graph.select_track_for_direction(edge_idx, traveling_backward);

                forward_route.push(RouteSegment {
                    edge_index: edge_idx.index(),
                    track_index,
                    origin_platform: prev_platform_idx,
                    destination_platform: platform_idx,
                    duration: Some(edge_duration),
                    wait_time: if is_last { wait_duration } else { Duration::zero() },
                });
            }
        }

        prev_station = Some((station_idx, stop, platform_idx));

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
    let (return_route, return_route_first_wait) = if let Some(turnaround) = turnaround_idx {
        build_return_route(&nimby_line.stops, turnaround, data, config, graph, edge_map)?
    } else {
        (Vec::new(), Duration::zero())
    };

    let mut line = Line {
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
        first_stop_wait_time,
        return_first_stop_wait_time: return_route_first_wait,
        #[allow(clippy::cast_precision_loss)]
        sort_index: Some(color_seed as f64),
        sync_departure_offsets: false,
        folder_id: None,
        style: crate::models::LineStyle::default(),
        forward_turnaround: turnaround_idx.is_some(),
        return_turnaround: turnaround_idx.is_some(),
    };

    // Import schedule data from NIMBY schedules
    let max_stop_idx = nimby_line.stops.len().saturating_sub(1);
    import_schedule_for_line(
        &mut line,
        nimby_line,
        turnaround_idx,
        max_stop_idx,
        data,
        graph,
    );

    Ok(Some(line))
}

/// Update an existing line's routes with new stops and timing from NIMBY data,
/// while preserving the existing wait times at stations.
#[allow(clippy::too_many_lines)]
fn update_existing_line(
    existing_line: &mut Line,
    nimby_line: &NimbyLine,
    data: &NimbyImportData,
    config: &NimbyImportConfig,
    graph: &mut RailwayGraph,
    edge_map: &mut HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>>,
) -> Result<(), String> {
    // Build a map of station name -> wait_time from existing routes.
    // We use station names instead of node indices because edges may have been
    // modified (e.g., passing loops added) which invalidates old edge indices.
    //
    // We use two approaches:
    // 1. Look up edge endpoints if the edge still exists
    // 2. Use NIMBY stop order to match segments to station names (for removed edges)
    let mut wait_time_map: HashMap<String, Duration> = HashMap::new();

    // First, try to get station names by matching segment index to NIMBY stop order.
    // This works even if edges have been removed.
    let nimby_stops: Vec<_> = nimby_line.stops.iter()
        .filter(|s| s.station_id != "0x0" && !is_depot(data, &s.station_id))
        .collect();

    // Each segment ends at a stop (segments map 1:1 with stops after the first)
    for (seg_idx, segment) in existing_line.forward_route.iter().enumerate() {
        // seg_idx 0 -> stop 1, seg_idx 1 -> stop 2, etc.
        let stop_idx = seg_idx + 1;
        if stop_idx < nimby_stops.len() {
            if let Some(station) = data.stations.get(&nimby_stops[stop_idx].station_id) {
                wait_time_map.insert(station.name.clone(), segment.wait_time);
            }
        }
    }

    // Do the same for return route if it exists
    let turnaround_idx_for_map = detect_turnaround(&nimby_line.stops, data);
    if let Some(turnaround) = turnaround_idx_for_map {
        let return_stops: Vec<_> = nimby_line.stops.iter()
            .skip(turnaround)
            .filter(|s| s.station_id != "0x0" && !is_depot(data, &s.station_id))
            .collect();

        for (seg_idx, segment) in existing_line.return_route.iter().enumerate() {
            let stop_idx = seg_idx + 1;
            if stop_idx < return_stops.len() {
                if let Some(station) = data.stations.get(&return_stops[stop_idx].station_id) {
                    wait_time_map.insert(station.name.clone(), segment.wait_time);
                }
            }
        }
    }

    // Build new routes using the same logic as import_single_line
    let mut forward_route: Vec<RouteSegment> = Vec::new();
    let mut prev_station: Option<(NodeIndex, &NimbyStop, usize)> = None;
    let mut first_stop_wait_time = Duration::zero();

    let turnaround_idx = detect_turnaround(&nimby_line.stops, data);
    let mut accumulated_distance: f64 = 0.0;

    for (stop_idx, stop) in nimby_line.stops.iter().enumerate() {
        if stop.station_id == "0x0" {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        if is_depot(data, &stop.station_id) {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        let total_leg_distance = accumulated_distance + stop.leg_distance;
        accumulated_distance = 0.0;

        let connection = prev_station.map(|(prev_idx, _, _)| ConnectionContext {
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

        let platform_idx = get_platform_from_stop(stop)
            .map_or(0, |name| super::shared::get_or_add_platform(graph, station_idx, &name));

        if prev_station.is_none() {
            first_stop_wait_time = Duration::seconds((stop.departure - stop.arrival).max(0));
        }

        if let Some((prev_idx, prev_stop, prev_platform_idx)) = prev_station {
            let Some(edges) = get_or_create_edges(
                graph,
                edge_map,
                prev_idx,
                station_idx,
                total_leg_distance,
            ) else {
                return Err(format!(
                    "No path from {:?} to {:?} for line '{}'",
                    graph.graph[prev_idx].display_name(),
                    graph.graph[station_idx].display_name(),
                    nimby_line.name
                ));
            };

            let total_travel_secs = if stop.arrival > prev_stop.departure {
                stop.arrival - prev_stop.departure
            } else {
                60
            };

            // Use preserved wait time if available, otherwise use NIMBY timing
            let nimby_wait = Duration::seconds((stop.departure - stop.arrival).max(0));
            let station_name = graph.graph[station_idx].display_name();
            let wait_duration = wait_time_map.get(&station_name).copied().unwrap_or(nimby_wait);

            let total_distance: f64 = edges.iter()
                .filter_map(|&e| graph.graph.edge_weight(e))
                .filter_map(|seg| seg.distance)
                .sum();

            let mut remaining_secs = total_travel_secs;

            for (i, &edge_idx) in edges.iter().enumerate() {
                let is_last = i == edges.len() - 1;

                #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
                let edge_duration = if edges.len() == 1 {
                    Duration::seconds(total_travel_secs)
                } else if is_last {
                    Duration::seconds(remaining_secs)
                } else if total_distance > 0.0 {
                    let edge_dist = graph.graph.edge_weight(edge_idx)
                        .and_then(|s| s.distance)
                        .unwrap_or(1.0);
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = (total_travel_secs as f64 * edge_dist / total_distance) as i64;
                    remaining_secs -= secs;
                    Duration::seconds(secs)
                } else {
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = total_travel_secs / edges.len() as i64;
                    remaining_secs -= secs;
                    Duration::seconds(secs)
                };

                let (edge_source, edge_target) = graph.graph.edge_endpoints(edge_idx)
                    .expect("edge should exist");
                let traveling_backward = prev_idx == edge_target && station_idx == edge_source;
                let track_index = graph.select_track_for_direction(edge_idx, traveling_backward);

                forward_route.push(RouteSegment {
                    edge_index: edge_idx.index(),
                    track_index,
                    origin_platform: prev_platform_idx,
                    destination_platform: platform_idx,
                    duration: Some(edge_duration),
                    wait_time: if is_last { wait_duration } else { Duration::zero() },
                });
            }
        }

        prev_station = Some((station_idx, stop, platform_idx));

        if turnaround_idx == Some(stop_idx) {
            break;
        }
    }

    if forward_route.is_empty() {
        return Err(format!("No valid route built for line '{}'", nimby_line.name));
    }

    // Build return route if there's a turnaround
    let (return_route, return_route_first_wait) = if let Some(turnaround) = turnaround_idx {
        let (mut route, first_wait) = build_return_route(&nimby_line.stops, turnaround, data, config, graph, edge_map)?;

        // Apply preserved wait times to return route
        for segment in &mut route {
            if let Some((_, to)) = graph.graph.edge_endpoints(EdgeIndex::new(segment.edge_index)) {
                let station_name = graph.graph[to].display_name();
                if let Some(&preserved_wait) = wait_time_map.get(&station_name) {
                    segment.wait_time = preserved_wait;
                }
            }
        }

        (route, first_wait)
    } else {
        (Vec::new(), Duration::zero())
    };

    // Update the existing line with new routes while preserving other properties
    existing_line.forward_route = forward_route;
    existing_line.return_route = return_route;
    existing_line.first_stop_wait_time = first_stop_wait_time;
    existing_line.return_first_stop_wait_time = return_route_first_wait;
    existing_line.forward_turnaround = turnaround_idx.is_some();
    existing_line.return_turnaround = turnaround_idx.is_some();

    Ok(())
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

    // Create new station with empty platforms (will be populated as lines are processed)
    let station = StationNode {
        name: nimby_station.name.clone(),
        external_id: Some(nimby_id.to_string()),
        position: None,
        passing_loop: false,
        platforms: Vec::new(),
        label_position: None,
    };
    let idx = graph.graph.add_node(Node::Station(station));
    Ok(idx)
}

/// Check if a station is a depot (should be excluded from infrastructure)
fn is_depot(data: &NimbyImportData, station_id: &str) -> bool {
    data.stations
        .get(station_id)
        .is_some_and(|s| s.name.contains("[DEP]"))
}

/// Strip cardinal direction suffix (N/E/S/W) from platform names like "4S" -> "4"
fn normalize_platform_name(name: &str) -> String {
    if name.len() > 1 {
        if let Some(last_char) = name.chars().last() {
            if matches!(last_char, 'N' | 'E' | 'S' | 'W') {
                let prefix = &name[..name.len() - 1];
                if prefix.chars().all(|c| c.is_ascii_digit()) {
                    return prefix.to_string();
                }
            }
        }
    }
    name.to_string()
}

/// Extract the first platform name from a stop's areas
fn get_platform_from_stop(stop: &NimbyStop) -> Option<String> {
    stop.areas.iter()
        .flatten()
        .next()
        .map(|area| normalize_platform_name(&area.platform_name))
}

/// Check if a connection exists between two stations, either directly or through passing loops.
fn connection_exists(graph: &RailwayGraph, from: NodeIndex, to: NodeIndex) -> bool {
    // Direct edge
    if graph.graph.find_edge(from, to).is_some() || graph.graph.find_edge(to, from).is_some() {
        return true;
    }

    // Check for path through passing loops
    !find_existing_passing_loops(graph, from, to).is_empty()
}

/// Find existing passing loops between two stations and their distance ratios.
/// Returns a list of `(NodeIndex, f64)` tuples for all passing loops on the path.
/// Uses BFS to find a path through passing loops.
fn find_existing_passing_loops(
    graph: &RailwayGraph,
    from_node: NodeIndex,
    to_node: NodeIndex,
) -> Vec<(NodeIndex, f64)> {
    use petgraph::Direction;

    // If there's a direct edge, no passing loops exist yet
    if graph.graph.find_edge(from_node, to_node).is_some()
        || graph.graph.find_edge(to_node, from_node).is_some()
    {
        return Vec::new();
    }

    // BFS to find a path through passing loops
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut parent: std::collections::HashMap<NodeIndex, NodeIndex> = std::collections::HashMap::new();

    visited.insert(from_node);
    queue.push_back(from_node);

    while let Some(current) = queue.pop_front() {
        for neighbor in graph
            .graph
            .neighbors_directed(current, Direction::Outgoing)
            .chain(graph.graph.neighbors_directed(current, Direction::Incoming))
        {
            if visited.contains(&neighbor) {
                continue;
            }

            // We can traverse through passing loops or reach the destination
            if neighbor == to_node {
                // Found path! Reconstruct it
                parent.insert(neighbor, current);
                return reconstruct_loop_path(graph, from_node, to_node, &parent);
            }

            // Only traverse through passing loops
            if graph.graph[neighbor]
                .as_station()
                .is_some_and(|s| s.passing_loop)
            {
                visited.insert(neighbor);
                parent.insert(neighbor, current);
                queue.push_back(neighbor);
            }
        }
    }

    // No path found through passing loops
    Vec::new()
}

/// Reconstruct the path and calculate loop ratios.
fn reconstruct_loop_path(
    graph: &RailwayGraph,
    from_node: NodeIndex,
    to_node: NodeIndex,
    parent: &std::collections::HashMap<NodeIndex, NodeIndex>,
) -> Vec<(NodeIndex, f64)> {
    // Reconstruct path from to_node back to from_node
    let mut path = Vec::new();
    let mut current = to_node;
    while current != from_node {
        path.push(current);
        current = match parent.get(&current) {
            Some(&p) => p,
            None => return Vec::new(), // Shouldn't happen
        };
    }
    path.push(from_node);
    path.reverse();

    // Calculate cumulative distances and collect loops
    let mut loops_with_distances = Vec::new();
    let mut cumulative_distance = 0.0;

    for window in path.windows(2) {
        let from = window[0];
        let to = window[1];
        cumulative_distance += get_edge_distance(graph, from, to);

        if graph.graph[to]
            .as_station()
            .is_some_and(|s| s.passing_loop)
        {
            loops_with_distances.push((to, cumulative_distance));
        }
    }

    convert_to_ratios(loops_with_distances, cumulative_distance)
}

/// Get the distance of the edge between two nodes (in either direction).
fn get_edge_distance(graph: &RailwayGraph, a: NodeIndex, b: NodeIndex) -> f64 {
    let edge = graph
        .graph
        .find_edge(a, b)
        .or_else(|| graph.graph.find_edge(b, a));
    edge.and_then(|e| graph.graph.edge_weight(e))
        .and_then(|w| w.distance)
        .unwrap_or(0.0)
}

/// Convert absolute distances to ratios of total distance.
fn convert_to_ratios(
    loops_with_distances: Vec<(NodeIndex, f64)>,
    total_distance: f64,
) -> Vec<(NodeIndex, f64)> {
    if total_distance <= 0.0 {
        return Vec::new();
    }
    loops_with_distances
        .into_iter()
        .map(|(node, dist)| (node, dist / total_distance))
        .collect()
}

/// Distance matching tolerance (10%)
const DISTANCE_TOLERANCE_PERCENT: f64 = 0.10;
/// Minimum distance tolerance in meters
const DISTANCE_TOLERANCE_MIN_METERS: f64 = 100.0;
/// Minimum distance from station for passing loop waypoints (200m)
/// Waypoints closer than this are likely station throat routing, not passing loops
const MIN_PASSING_LOOP_DISTANCE_FROM_STATION: f64 = 200.0;
/// Maximum distance mismatch for passing loop waypoint matching (700m)
/// Forward/return waypoints within this distance are considered the same passing loop
const PASSING_LOOP_WAYPOINT_TOLERANCE: f64 = 700.0;

/// Check if an edge exists between two nodes with distance matching the expected value
/// Note: existing distance is in km, `expected_distance` is in meters (from NIMBY)
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
                // Convert existing km to meters for comparison
                return distances_match(distance * METERS_PER_KM, expected_distance);
            }
        }
    }
    false
}

/// Grid size in pixels for snapping
const GRID_SIZE: f64 = 30.0;

/// Conversion factor: NIMBY uses meters, graph stores km
const METERS_PER_KM: f64 = 1000.0;

/// A path between two stations with intermediate stops and distance info
#[derive(Debug, Clone)]
struct SegmentPath {
    /// Intermediate station IDs between start and end (not including start/end)
    intermediates: Vec<String>,
    /// Total distance for this segment (sum of `leg_distance` values)
    total_distance: f64,
    /// Waypoint positions as cumulative distances from segment start
    /// Each f64 is the distance from the first station to that waypoint
    waypoint_distances: Vec<f64>,
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
///
/// Also captures waypoint positions for passing loop detection.
fn build_segment_map(lines: &[&NimbyLine], data: &NimbyImportData) -> SegmentMap {
    let mut map = SegmentMap::new();

    for line in lines {
        // Get all actual station stops (exclude waypoints and depots)
        let stations: Vec<(usize, &NimbyStop)> = line.stops.iter()
            .enumerate()
            .filter(|(_, s)| s.station_id != "0x0" && !is_depot(data, &s.station_id))
            .collect();

        // For each CONSECUTIVE pair of stations, record the path with waypoints
        // (We only care about consecutive pairs for passing loop detection)
        for window in stations.windows(2) {
            let (from_idx, from_stop) = window[0];
            let (to_idx, _to_stop) = window[1];
            let from_id = &from_stop.station_id;
            let to_id = &window[1].1.station_id;

            // Calculate total distance and collect waypoint positions
            let mut cumulative_dist = 0.0;
            let mut waypoint_distances = Vec::new();

            for stop in &line.stops[from_idx + 1..=to_idx] {
                cumulative_dist += stop.leg_distance;
                if stop.station_id == "0x0" {
                    waypoint_distances.push(cumulative_dist);
                }
            }

            let path = SegmentPath {
                intermediates: Vec::new(), // No intermediates for consecutive pairs
                total_distance: cumulative_dist,
                waypoint_distances,
            };
            let key = (from_id.clone(), to_id.clone());
            let paths = map.entry(key).or_default();
            add_path_to_segment_map(paths, path);
        }

        // Also record non-consecutive pairs for express route analysis (without waypoints)
        for i in 0..stations.len() {
            for j in (i + 2)..stations.len() {
                // Skip consecutive pairs (already handled above)
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

                let path = SegmentPath {
                    intermediates,
                    total_distance,
                    waypoint_distances: Vec::new(), // No waypoint tracking for express paths
                };
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

/// Check if existing graph distance matches NIMBY distance within tolerance
/// Note: existing is in km (from graph), `nimby_distance` is in meters
fn distance_matches(existing_km: Option<f64>, nimby_distance: f64) -> bool {
    match existing_km {
        Some(d) if nimby_distance > 0.0 => {
            // Convert existing km to meters for comparison
            let existing_meters = d * METERS_PER_KM;
            let diff = (existing_meters - nimby_distance).abs();
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

/// Detect if a line has a turnaround (out-and-back pattern)
/// Returns the index of the turnaround station (furthest point before returning)
fn detect_turnaround(stops: &[NimbyStop], data: &NimbyImportData) -> Option<usize> {
    if stops.len() < 3 {
        return None;
    }

    // Collect station indices (excluding waypoints and depots)
    let station_stops: Vec<(usize, &str)> = stops
        .iter()
        .enumerate()
        .filter(|(_, s)| s.station_id != "0x0" && !is_depot(data, &s.station_id))
        .map(|(idx, s)| (idx, s.station_id.as_str()))
        .collect();

    if station_stops.len() < 3 {
        return None;
    }

    // Find the first station that appears more than once
    // (same pattern as build_segment_map turnaround detection)
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for (i, (_, station_id)) in station_stops.iter().enumerate() {
        if !seen.insert(station_id) {
            // This station was already seen - turnaround is the previous station
            if i > 0 {
                return Some(station_stops[i - 1].0);
            }
            return None;
        }
    }

    // No repeated stations - no turnaround
    None
}

/// A detected passing loop location between two stations
#[derive(Debug, Clone)]
struct PassingLoopCandidate {
    /// Station IDs this loop is between (ordered: from, to on forward journey)
    between_stations: (String, String),
    /// Distance ratio from first station (0.0 to 1.0)
    distance_ratio: f64,
    /// Total distance of the segment between the two stations
    segment_distance: f64,
}

/// Build return route from turnaround point back to start
fn build_return_route(
    stops: &[NimbyStop],
    turnaround_idx: usize,
    data: &NimbyImportData,
    config: &NimbyImportConfig,
    graph: &mut RailwayGraph,
    edge_map: &mut HashMap<(NodeIndex, NodeIndex), Vec<EdgeIndex>>,
) -> Result<(Vec<RouteSegment>, Duration), String> {
    let mut return_route = Vec::new();
    let mut prev_station: Option<(NodeIndex, &NimbyStop, usize)> = None; // (idx, stop, platform_idx)
    let mut accumulated_distance: f64 = 0.0;
    let mut first_stop_wait_time = Duration::zero();

    // Get the turnaround station ID - we need to find the LAST occurrence before
    // the actual return route starts (after depot area)
    let turnaround_station_id = stops.get(turnaround_idx)
        .map(|s| s.station_id.as_str());

    // Find the last occurrence of the turnaround station in the return portion
    // This handles cases like: Drammen → [Depot] → Drammen → Oslo → ...
    // where we want to start from the second Drammen
    let return_start_idx = if let Some(turnaround_id) = turnaround_station_id {
        // Look for the last occurrence of turnaround station before a different station appears
        let mut last_turnaround_idx = turnaround_idx;
        for (i, stop) in stops.iter().enumerate().skip(turnaround_idx) {
            // Skip waypoints and depots
            if stop.station_id == "0x0" || is_depot(data, &stop.station_id) {
                continue;
            }
            if stop.station_id == turnaround_id {
                last_turnaround_idx = i;
            } else {
                // Found a different station, stop looking
                break;
            }
        }
        last_turnaround_idx
    } else {
        turnaround_idx
    };

    // Process stops from the actual return start point
    for stop in stops.iter().skip(return_start_idx) {
        // Accumulate distance from waypoints
        if stop.station_id == "0x0" {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        // Skip depots, but accumulate their distance
        if is_depot(data, &stop.station_id) {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        // Total leg distance includes any skipped waypoints and depots
        let total_leg_distance = accumulated_distance + stop.leg_distance;
        accumulated_distance = 0.0;

        // Capture first stop wait time
        if prev_station.is_none() {
            first_stop_wait_time = Duration::seconds((stop.departure - stop.arrival).max(0));
        }

        // Build connection context if we have a previous station
        let connection = prev_station.map(|(prev_idx, _, _)| ConnectionContext {
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

        // Add platform to station if present in stop data and get its index
        let platform_idx = get_platform_from_stop(stop)
            .map_or(0, |name| super::shared::get_or_add_platform(graph, station_idx, &name));

        if let Some((prev_idx, prev_stop, prev_platform_idx)) = prev_station {
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

            // Track remaining time for multi-edge paths to avoid truncation loss
            let mut remaining_secs = total_travel_secs;

            for (i, &edge_idx) in edges.iter().enumerate() {
                let is_last = i == edges.len() - 1;

                // Calculate travel time for this edge
                #[allow(clippy::cast_precision_loss, clippy::cast_possible_wrap)]
                let edge_duration = if edges.len() == 1 {
                    // Single edge: use exact NIMBY timing (no distribution needed)
                    Duration::seconds(total_travel_secs)
                } else if is_last {
                    // Last edge gets remaining time (avoids truncation loss)
                    Duration::seconds(remaining_secs)
                } else if total_distance > 0.0 {
                    // Distribute proportionally by distance
                    let edge_dist = graph.graph.edge_weight(edge_idx)
                        .and_then(|s| s.distance)
                        .unwrap_or(1.0);
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = (total_travel_secs as f64 * edge_dist / total_distance) as i64;
                    remaining_secs -= secs;
                    Duration::seconds(secs)
                } else {
                    // Equal distribution if no distance info
                    #[allow(clippy::cast_possible_truncation)]
                    let secs = total_travel_secs / edges.len() as i64;
                    remaining_secs -= secs;
                    Duration::seconds(secs)
                };

                // Determine if we're traveling backward relative to the edge orientation
                // (i.e., from target to source instead of source to target)
                let (edge_source, edge_target) = graph.graph.edge_endpoints(edge_idx)
                    .expect("edge should exist");
                let traveling_backward = prev_idx == edge_target && station_idx == edge_source;
                let track_index = graph.select_track_for_direction(edge_idx, traveling_backward);

                return_route.push(RouteSegment {
                    edge_index: edge_idx.index(),
                    track_index,
                    origin_platform: prev_platform_idx,
                    destination_platform: platform_idx,
                    duration: Some(edge_duration),
                    wait_time: if is_last { wait_duration } else { Duration::zero() },
                });
            }
        }

        prev_station = Some((station_idx, stop, platform_idx));
    }

    Ok((return_route, first_stop_wait_time))
}

// ============================================================================
// Schedule Import Functions
// ============================================================================

/// Classification of a run based on its stop range relative to the line's turnaround point
#[derive(Debug, Clone, Copy, PartialEq)]
enum RunType {
    /// Covers the full loop (enter=0, exit=max)
    FullLoop,
    /// Forward journey only (enter=0, `exit=turnaround_idx`)
    ForwardOnly,
    /// Return journey only (`enter=turnaround_idx`, exit=max)
    ReturnOnly,
    /// Partial route (short-turn, express, etc.)
    Partial,
}

/// Classify a run based on its stop range
/// Uses a tolerance of ±2 stops for turnaround matching to handle depot areas
fn classify_run(run: &NimbyRun, turnaround_idx: Option<usize>, max_stop_idx: usize) -> RunType {
    let Some(turnaround) = turnaround_idx else {
        // No turnaround: only FullLoop or Partial
        if run.enter_stop_idx == 0 && run.exit_stop_idx == max_stop_idx {
            return RunType::FullLoop;
        }
        return RunType::Partial;
    };

    // Check if the exit/enter is near the turnaround (within ±2 stops for depot areas)
    let is_near_turnaround_exit = run.exit_stop_idx >= turnaround.saturating_sub(2)
        && run.exit_stop_idx <= turnaround + 2;
    let is_near_turnaround_enter = run.enter_stop_idx >= turnaround.saturating_sub(2)
        && run.enter_stop_idx <= turnaround + 2;

    if run.enter_stop_idx == 0 && run.exit_stop_idx == max_stop_idx {
        RunType::FullLoop
    } else if run.enter_stop_idx == 0 && is_near_turnaround_exit {
        RunType::ForwardOnly
    } else if is_near_turnaround_enter && run.exit_stop_idx == max_stop_idx {
        RunType::ReturnOnly
    } else {
        RunType::Partial
    }
}

/// Detect the most common interval between departures (in seconds)
/// Returns None if there are fewer than 2 departures
fn detect_frequency(departures: &[i64]) -> Option<Duration> {
    if departures.len() < 2 {
        return None;
    }

    let mut sorted = departures.to_vec();
    sorted.sort_unstable();

    // Calculate all gaps
    let gaps: Vec<i64> = sorted.windows(2).map(|w| w[1] - w[0]).collect();

    if gaps.is_empty() {
        return None;
    }

    // Round gaps to nearest minute for grouping
    let rounded_gaps: Vec<i64> = gaps.iter().map(|&g| (g / 60) * 60).collect();

    // Count occurrences of each gap
    let mut gap_counts: HashMap<i64, usize> = HashMap::new();
    for gap in &rounded_gaps {
        *gap_counts.entry(*gap).or_insert(0) += 1;
    }

    // Find the most common gap
    let most_common = gap_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(gap, _)| gap)?;

    // Don't return very short frequencies (less than 5 minutes)
    if most_common < 300 {
        return None;
    }

    Some(Duration::seconds(most_common))
}

/// Check if a departure time fits the pattern first + N*frequency (within 2 minutes tolerance)
fn fits_frequency_pattern(departure: i64, first: i64, frequency_secs: i64) -> bool {
    if frequency_secs <= 0 {
        return false;
    }
    let offset = departure - first;
    // Use rem_euclid to get positive remainder (handles negative offsets correctly)
    let remainder = offset.rem_euclid(frequency_secs);
    // Allow 2 minutes tolerance
    remainder < 120 || remainder > (frequency_secs - 120)
}

/// Find the departure time that, when used as the pattern base, matches the most departures.
/// This handles cases where early departures (e.g., positioning runs from a turnaround)
/// are on a different timing offset than the main schedule pattern.
fn find_best_first_departure(departures: &[i64], frequency_secs: i64) -> Option<i64> {
    if departures.is_empty() {
        return None;
    }
    if departures.len() == 1 {
        return Some(departures[0]);
    }

    // For each departure, count how many others match its pattern
    let mut best_candidate = departures[0];
    let mut best_match_count = 0;

    for &candidate in departures {
        let match_count = departures
            .iter()
            .filter(|&&dep| fits_frequency_pattern(dep, candidate, frequency_secs))
            .count();

        if match_count > best_match_count {
            best_match_count = match_count;
            best_candidate = candidate;
        }
    }

    Some(best_candidate)
}

/// Collected schedule data for a single direction
struct DirectionSchedule {
    /// Raw departure times (absolute seconds including day offset)
    raw_departures: Vec<i64>,
    /// Normalized departure times (mod 86400), populated after `finalize()`
    departures: Vec<i64>,
    /// Map from normalized departure time to the days it runs on
    departure_days: std::collections::HashMap<i64, crate::models::DaysOfWeek>,
    first_departure: Option<i64>,
    last_departure: Option<i64>,
    frequency: Option<Duration>,
    /// Days of week that the regular schedule operates on
    days_of_week: crate::models::DaysOfWeek,
}

impl DirectionSchedule {
    fn new() -> Self {
        Self {
            raw_departures: Vec::new(),
            departures: Vec::new(),
            departure_days: std::collections::HashMap::new(),
            first_departure: None,
            last_departure: None,
            frequency: None,
            days_of_week: crate::models::DaysOfWeek::empty(),
        }
    }

    fn add_departure(&mut self, time: i64) {
        self.raw_departures.push(time);
    }

    fn finalize(&mut self) {
        use crate::models::DaysOfWeek;

        if self.raw_departures.is_empty() {
            return;
        }

        // Build map from normalized time to days it appears on
        for &raw_time in &self.raw_departures {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let day_idx = (raw_time / 86400) as usize;
            let time_of_day = raw_time % 86400;

            if let Some(day_flag) = DaysOfWeek::from_index(day_idx) {
                self.departure_days
                    .entry(time_of_day)
                    .or_insert(DaysOfWeek::empty())
                    .insert(day_flag);
            }
        }

        // Get unique normalized times
        let mut normalized: Vec<i64> = self.departure_days.keys().copied().collect();
        normalized.sort_unstable();

        self.frequency = detect_frequency(&normalized);

        // Find the best first_departure - the one that maximizes matches with the frequency pattern
        // This handles cases where early departures are on a different offset than the main schedule
        self.first_departure = if let Some(freq) = self.frequency {
            find_best_first_departure(&normalized, freq.num_seconds())
        } else {
            normalized.first().copied()
        };
        self.last_departure = normalized.last().copied();

        // Calculate overall days_of_week as the union of all departures' days
        // This represents the days the schedule operates on
        self.days_of_week = self.departure_days.values().fold(
            DaysOfWeek::empty(),
            |acc, days| acc | *days,
        );

        self.departures = normalized;
    }

    /// Get the days of week for a specific departure time
    fn get_days_for_departure(&self, time_of_day: i64) -> crate::models::DaysOfWeek {
        self.departure_days
            .get(&time_of_day)
            .copied()
            .unwrap_or_default()
    }
}

/// Check if a node is a passing loop
fn is_passing_loop(graph: &RailwayGraph, node: NodeIndex) -> bool {
    graph.graph[node]
        .as_station()
        .is_some_and(|s| s.passing_loop)
}

/// Get the origin and destination stations for the forward direction
/// Uses the station path to get correct termini regardless of edge direction
/// Skips passing loops to find actual terminal stations
fn get_forward_route_endpoints(
    line: &Line,
    graph: &RailwayGraph,
) -> Option<(NodeIndex, NodeIndex)> {
    let station_path = line.get_station_path(graph);
    // Find first non-passing-loop station
    let origin = station_path
        .iter()
        .find(|&&n| !is_passing_loop(graph, n))
        .copied()?;
    // Find last non-passing-loop station
    let destination = station_path
        .iter()
        .rev()
        .find(|&&n| !is_passing_loop(graph, n))
        .copied()?;
    Some((origin, destination))
}

/// Get the origin and destination stations for the return direction
/// Uses the station path - return goes from last station back to first
/// Skips passing loops to find actual terminal stations
fn get_return_route_endpoints(
    line: &Line,
    graph: &RailwayGraph,
) -> Option<(NodeIndex, NodeIndex)> {
    let station_path = line.get_station_path(graph);
    // Return is the reverse of forward
    // Find last non-passing-loop station (origin of return)
    let origin = station_path
        .iter()
        .rev()
        .find(|&&n| !is_passing_loop(graph, n))
        .copied()?;
    // Find first non-passing-loop station (destination of return)
    let destination = station_path
        .iter()
        .find(|&&n| !is_passing_loop(graph, n))
        .copied()?;
    Some((origin, destination))
}

/// Create a manual departure
fn create_manual_departure(
    time_secs: i64,
    from_station: NodeIndex,
    to_station: NodeIndex,
    train_number: Option<String>,
    days_of_week: crate::models::DaysOfWeek,
) -> crate::models::ManualDeparture {
    crate::models::ManualDeparture {
        id: uuid::Uuid::new_v4(),
        time: BASE_MIDNIGHT + Duration::seconds(time_secs),
        from_station,
        to_station,
        days_of_week,
        train_number,
        repeat_interval: None,
        repeat_until: None,
    }
}

/// Get the return departure time from a full loop run at the turnaround point
/// The turnaround station is both the last forward stop (arrival) and first return stop (departure)
fn get_return_departure_from_full_loop(
    run: &NimbyRun,
    turnaround_idx: Option<usize>,
    nimby_line: &NimbyLine,
    data: &NimbyImportData,
) -> Option<i64> {
    let turnaround = turnaround_idx?;

    // First try the turnaround station itself - this is where the return journey starts
    let turnaround_stop = nimby_line.stops.get(turnaround)?;
    if turnaround_stop.station_id != "0x0" && !is_depot(data, &turnaround_stop.station_id) {
        // The turnaround station is a real station - use its departure time
        let dep_idx = turnaround * 2 + 1;
        if let Some(&dep) = run.arrival_departure.get(dep_idx) {
            return Some(dep);
        }
    }

    // If turnaround is a waypoint/depot, find the first real station after it
    for (offset, stop) in nimby_line.stops.iter().skip(turnaround + 1).enumerate() {
        if stop.station_id == "0x0" {
            continue;
        }
        if is_depot(data, &stop.station_id) {
            continue;
        }

        let stop_idx = turnaround + 1 + offset;
        let dep_idx = stop_idx * 2 + 1;
        if let Some(&dep) = run.arrival_departure.get(dep_idx) {
            return Some(dep);
        }
    }
    None
}

/// Get the departure time from a return-only run
/// Uses `enter_stop_idx` to find the first real station
fn get_return_departure_from_return_only(
    run: &NimbyRun,
    nimby_line: &NimbyLine,
    data: &NimbyImportData,
) -> Option<i64> {
    // Find first real station starting from enter_stop_idx
    // Track position within the run's arrival_departure array
    let mut run_stop_count = 0;
    for stop in nimby_line.stops.iter().skip(run.enter_stop_idx) {
        // Skip waypoints
        if stop.station_id == "0x0" {
            run_stop_count += 1;
            continue;
        }
        // Skip depots
        if is_depot(data, &stop.station_id) {
            run_stop_count += 1;
            continue;
        }

        // For return-only runs, arrival_departure is indexed relative to the run's start
        // So the first stop (enter_stop_idx) is at index 0 in arrival_departure
        let dep_idx = run_stop_count * 2 + 1;
        return run.arrival_departure.get(dep_idx).copied();
    }
    None
}

/// Context for processing runs
struct RunProcessingContext<'a> {
    turnaround_idx: Option<usize>,
    max_stop_idx: usize,
    nimby_line: &'a NimbyLine,
    data: &'a NimbyImportData,
}

/// Process a single run and add to appropriate schedule
fn process_run<'a>(
    run: &'a NimbyRun,
    shift: &'a NimbyShift,
    ctx: &RunProcessingContext<'_>,
    forward_schedule: &mut DirectionSchedule,
    return_schedule: &mut DirectionSchedule,
    partial_runs: &mut Vec<(&'a NimbyRun, &'a NimbyShift)>,
) {
    let departure_time = run.arrival_departure.get(1).copied().unwrap_or(0);
    let run_type = classify_run(run, ctx.turnaround_idx, ctx.max_stop_idx);

    match run_type {
        RunType::FullLoop => {
            forward_schedule.add_departure(departure_time);
            if let Some(return_dep) = get_return_departure_from_full_loop(
                run, ctx.turnaround_idx, ctx.nimby_line, ctx.data
            ) {
                return_schedule.add_departure(return_dep);
            }
        }
        RunType::ForwardOnly => forward_schedule.add_departure(departure_time),
        RunType::ReturnOnly => {
            if let Some(dep) = get_return_departure_from_return_only(run, ctx.nimby_line, ctx.data) {
                return_schedule.add_departure(dep);
            }
        }
        RunType::Partial => partial_runs.push((run, shift)),
    }
}

/// Collect runs from all schedules for a specific line
fn collect_runs_for_line<'a>(
    data: &'a NimbyImportData,
    nimby_line: &'a NimbyLine,
    turnaround_idx: Option<usize>,
    max_stop_idx: usize,
) -> (DirectionSchedule, DirectionSchedule, Vec<(&'a NimbyRun, &'a NimbyShift)>) {
    let mut forward_schedule = DirectionSchedule::new();
    let mut return_schedule = DirectionSchedule::new();
    let mut partial_runs = Vec::new();

    let ctx = RunProcessingContext {
        turnaround_idx,
        max_stop_idx,
        nimby_line,
        data,
    };

    let matching_runs = data.schedules.iter()
        .flat_map(|s| s.shifts.iter())
        .flat_map(|shift| shift.runs.iter().map(move |run| (run, shift)))
        .filter(|(run, _)| run.line_id == nimby_line.id);

    for (run, shift) in matching_runs {
        process_run(
            run, shift, &ctx,
            &mut forward_schedule, &mut return_schedule, &mut partial_runs,
        );
    }

    (forward_schedule, return_schedule, partial_runs)
}

/// Add irregular departures as manual departures for a direction
fn add_irregular_departures(
    line: &mut Line,
    schedule: &DirectionSchedule,
    first_departure: i64,
    freq_secs: i64,
    origin: NodeIndex,
    destination: NodeIndex,
) {
    for &dep in &schedule.departures {
        if !fits_frequency_pattern(dep, first_departure, freq_secs) {
            let days = schedule.get_days_for_departure(dep);
            line.manual_departures.push(create_manual_departure(
                dep, origin, destination, None, days,
            ));
        }
    }
}

/// Add all departures as manual departures for a direction
fn add_all_as_manual(
    line: &mut Line,
    schedule: &DirectionSchedule,
    origin: NodeIndex,
    destination: NodeIndex,
) {
    for &dep in &schedule.departures {
        let days = schedule.get_days_for_departure(dep);
        line.manual_departures.push(create_manual_departure(
            dep, origin, destination, None, days,
        ));
    }
}

/// Import schedule data for a line from NIMBY schedules
#[allow(clippy::too_many_lines)]
fn import_schedule_for_line(
    line: &mut Line,
    nimby_line: &NimbyLine,
    turnaround_idx: Option<usize>,
    max_stop_idx: usize,
    data: &NimbyImportData,
    graph: &RailwayGraph,
) {
    // Collect and classify all runs
    let (mut forward_schedule, mut return_schedule, partial_runs) =
        collect_runs_for_line(data, nimby_line, turnaround_idx, max_stop_idx);

    forward_schedule.finalize();
    return_schedule.finalize();

    let has_forward = !forward_schedule.departures.is_empty();
    let has_return = !return_schedule.departures.is_empty();

    if !has_forward && !has_return && partial_runs.is_empty() {
        line.schedule_mode = ScheduleMode::Manual;
        return;
    }

    // Get route endpoints once
    let forward_endpoints = get_forward_route_endpoints(line, graph);
    let return_endpoints = get_return_route_endpoints(line, graph);

    // Determine frequency
    let base_frequency = forward_schedule.frequency.or(return_schedule.frequency);

    if let Some(frequency) = base_frequency {
        // Set up auto schedule
        line.schedule_mode = ScheduleMode::Auto;
        line.frequency = frequency;

        // Set days_of_week as the union of forward and return schedule days
        line.days_of_week = forward_schedule.days_of_week | return_schedule.days_of_week;

        if let Some(first) = forward_schedule.first_departure {
            line.first_departure = BASE_MIDNIGHT + Duration::seconds(first);
        }
        if let Some(last) = forward_schedule.last_departure {
            line.last_departure = BASE_MIDNIGHT + Duration::seconds(last);
        }
        if let Some(first) = return_schedule.first_departure {
            line.return_first_departure = BASE_MIDNIGHT + Duration::seconds(first);
        }
        if let Some(last) = return_schedule.last_departure {
            line.return_last_departure = BASE_MIDNIGHT + Duration::seconds(last);
        }

        let freq_secs = frequency.num_seconds();

        // Add irregular forward departures
        if let (Some(first_fwd), Some((origin, dest))) =
            (forward_schedule.first_departure, forward_endpoints)
        {
            add_irregular_departures(
                line,
                &forward_schedule,
                first_fwd,
                freq_secs,
                origin,
                dest,
            );
        }

        // Add irregular return departures
        if let (Some(first_ret), Some((origin, dest))) =
            (return_schedule.first_departure, return_endpoints)
        {
            add_irregular_departures(
                line,
                &return_schedule,
                first_ret,
                freq_secs,
                origin,
                dest,
            );
        }
    } else {
        // No consistent frequency - all manual
        line.schedule_mode = ScheduleMode::Manual;

        // Set days_of_week as the union of forward and return schedule days
        line.days_of_week = forward_schedule.days_of_week | return_schedule.days_of_week;

        if let Some((origin, dest)) = forward_endpoints {
            add_all_as_manual(line, &forward_schedule, origin, dest);
        }
        if let Some((origin, dest)) = return_endpoints {
            add_all_as_manual(line, &return_schedule, origin, dest);
        }
    }

    // Add partial runs as manual departures
    // Group by normalized time to track days
    add_partial_runs_as_manual(line, &partial_runs, graph);
}

/// Data for a partial run grouped by departure time
struct PartialRunData {
    days: crate::models::DaysOfWeek,
    from_station: Option<NodeIndex>,
    to_station: Option<NodeIndex>,
    train_number: Option<String>,
}

/// Add partial runs as manual departures, grouping by time to track days
fn add_partial_runs_as_manual(
    line: &mut Line,
    partial_runs: &[(&NimbyRun, &NimbyShift)],
    graph: &RailwayGraph,
) {
    let mut partial_by_time: std::collections::HashMap<i64, PartialRunData> =
        std::collections::HashMap::new();

    for (run, shift) in partial_runs {
        let departure_time = run.arrival_departure.get(1).copied().unwrap_or(0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let day_idx = (departure_time / 86400) as usize;
        let normalized = departure_time % 86400;

        let from_station = line.forward_route.get(run.enter_stop_idx)
            .and_then(|seg| graph.graph.edge_endpoints(EdgeIndex::new(seg.edge_index)))
            .map(|(from, _)| from);

        let to_station = line.forward_route.get(run.exit_stop_idx.saturating_sub(1))
            .and_then(|seg| graph.graph.edge_endpoints(EdgeIndex::new(seg.edge_index)))
            .map(|(_, to)| to);

        let day_flag = crate::models::DaysOfWeek::from_index(day_idx)
            .unwrap_or(crate::models::DaysOfWeek::ALL_DAYS);

        partial_by_time
            .entry(normalized)
            .and_modify(|data| data.days.insert(day_flag))
            .or_insert(PartialRunData {
                days: day_flag,
                from_station,
                to_station,
                train_number: Some(shift.name.clone()),
            });
    }

    for (time, data) in partial_by_time {
        if let (Some(from_station), Some(to_station)) = (data.from_station, data.to_station) {
            line.manual_departures.push(create_manual_departure(
                time, from_station, to_station, data.train_number, data.days,
            ));
        }
    }
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

    #[test]
    fn test_r14_schedule_import() {
        // Load the actual timetable.json file
        let json = std::fs::read_to_string("timetable.json")
            .expect("Could not read timetable.json");

        let data = parse_nimby_json(&json).unwrap();

        // Find R14 line
        let r14 = data.lines.iter()
            .find(|l| l.name == "R14 Kongsvinger - Drammen")
            .expect("R14 not found");

        println!("R14 line id: {}", r14.id);
        println!("R14 stop count: {}", r14.stops.len());

        // First import infrastructure for all lines to build the graph
        let mut graph = RailwayGraph::default();
        let infra_config = NimbyImportConfig {
            create_infrastructure: true,
            selected_line_ids: Vec::new(), // Import all for infrastructure
            ..Default::default()
        };
        let _ = import_nimby_lines(&data, &infra_config, &mut graph, 0, None).unwrap();

        println!("Graph has {} nodes and {} edges", graph.graph.node_count(), graph.graph.edge_count());

        // Now import just R14 as a line (using existing infrastructure)
        let config = NimbyImportConfig {
            create_infrastructure: false,
            selected_line_ids: vec![r14.id.clone()],
            ..Default::default()
        };
        let lines = import_nimby_lines(&data, &config, &mut graph, 0, None).unwrap();

        assert_eq!(lines.len(), 1);
        let line = &lines[0];

        println!("\n=== R14 Schedule Import Results ===");
        println!("Name: {}", line.name);
        println!("Forward route length: {}", line.forward_route.len());
        println!("Return route length: {}", line.return_route.len());
        println!("Schedule Mode: {:?}", line.schedule_mode);
        println!("Days of Week: {}", line.days_of_week.to_display_string());
        println!("Frequency: {} seconds ({} min)", line.frequency.num_seconds(), line.frequency.num_minutes());
        println!("First Departure: {}", line.first_departure.format("%H:%M"));
        println!("Last Departure: {}", line.last_departure.format("%H:%M"));
        println!("Return First Departure: {}", line.return_first_departure.format("%H:%M"));
        println!("Return Last Departure: {}", line.return_last_departure.format("%H:%M"));
        println!("Manual Departures: {}", line.manual_departures.len());

        // Show all manual departures with station names
        for (i, dep) in line.manual_departures.iter().enumerate() {
            let from_name = graph.get_station_name(dep.from_station).unwrap_or("?");
            let to_name = graph.get_station_name(dep.to_station).unwrap_or("?");
            println!("  Manual {}: {} days={} from={} to={} train={:?}",
                i, dep.time.format("%H:%M"), dep.days_of_week.to_display_string(),
                from_name, to_name, dep.train_number);
        }

        // Basic sanity checks
        assert!(line.frequency.num_seconds() > 0, "Should have detected a frequency");
    }

    #[test]
    fn test_update_existing_line_with_passing_loop() {
        use crate::models::{Line, RouteSegment, Node, Track, TrackDirection, StationNode, Platform};
        use chrono::Duration;

        // Create a simple graph: A -- B -- C
        let mut graph = RailwayGraph::default();

        let station_a = graph.graph.add_node(Node::Station(StationNode {
            name: "Station A".to_string(),
            external_id: Some("0x1".to_string()),
            position: Some((0.0, 0.0)),
            passing_loop: false,
            platforms: vec![],
            label_position: None,
        }));

        let station_b = graph.graph.add_node(Node::Station(StationNode {
            name: "Station B".to_string(),
            external_id: Some("0x2".to_string()),
            position: Some((100.0, 0.0)),
            passing_loop: false,
            platforms: vec![],
            label_position: None,
        }));

        let station_c = graph.graph.add_node(Node::Station(StationNode {
            name: "Station C".to_string(),
            external_id: Some("0x3".to_string()),
            position: Some((200.0, 0.0)),
            passing_loop: false,
            platforms: vec![],
            label_position: None,
        }));

        // Create edges A-B and B-C
        let tracks = vec![Track { direction: TrackDirection::Bidirectional }];
        let edge_ab = graph.add_track(station_a, station_b, tracks.clone(), Some(10.0));
        let edge_bc = graph.add_track(station_b, station_c, tracks.clone(), Some(10.0));

        // Create an existing line A -> B -> C with custom wait times
        let mut existing_line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test Line".to_string(),
            code: "T1".to_string(),
            color: "#FF0000".to_string(),
            frequency: Duration::minutes(30),
            thickness: 2.0,
            first_departure: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(6, 0, 0).unwrap(),
            return_first_departure: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(6, 0, 0).unwrap(),
            last_departure: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(22, 0, 0).unwrap(),
            return_last_departure: chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap().and_hms_opt(22, 0, 0).unwrap(),
            visible: true,
            schedule_mode: crate::models::ScheduleMode::Auto,
            days_of_week: crate::models::DaysOfWeek::default(),
            manual_departures: Vec::new(),
            forward_route: vec![
                RouteSegment {
                    edge_index: edge_ab.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(5)),
                    wait_time: Duration::minutes(2), // Custom wait time at B
                },
                RouteSegment {
                    edge_index: edge_bc.index(),
                    track_index: 0,
                    origin_platform: 0,
                    destination_platform: 0,
                    duration: Some(Duration::minutes(5)),
                    wait_time: Duration::minutes(3), // Custom wait time at C
                },
            ],
            return_route: Vec::new(),
            sync_routes: false,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            default_wait_time: Duration::seconds(30),
            first_stop_wait_time: Duration::zero(),
            return_first_stop_wait_time: Duration::zero(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
            style: crate::models::LineStyle::default(),
            forward_turnaround: false,
            return_turnaround: false,
        };

        // Verify initial state
        assert_eq!(existing_line.forward_route.len(), 2);
        assert_eq!(existing_line.forward_route[0].wait_time.num_minutes(), 2);
        assert_eq!(existing_line.forward_route[1].wait_time.num_minutes(), 3);

        // Now add a passing loop between A and B (split edge_ab into two edges)
        let passing_loop = graph.graph.add_node(Node::Station(StationNode {
            name: "Passing Loop".to_string(),
            external_id: None,
            position: Some((50.0, 0.0)),
            passing_loop: true,
            platforms: vec![
                Platform { name: "1".to_string() },
                Platform { name: "2".to_string() },
            ],
            label_position: None,
        }));

        // Remove old edge and create two new edges
        graph.graph.remove_edge(edge_ab);
        let _edge_a_loop = graph.add_track(station_a, passing_loop, tracks.clone(), Some(5.0));
        let _edge_loop_b = graph.add_track(passing_loop, station_b, tracks.clone(), Some(5.0));

        // Now the graph is: A -- PassingLoop -- B -- C
        // The line's route still references the old edge_ab which is now invalid

        // Create minimal NIMBY data for update
        let nimby_data = NimbyImportData {
            company_name: "Test".to_string(),
            stations: [
                ("0x1".to_string(), NimbyStation { id: "0x1".to_string(), name: "Station A".to_string(), lonlat: (0.0, 0.0) }),
                ("0x2".to_string(), NimbyStation { id: "0x2".to_string(), name: "Station B".to_string(), lonlat: (0.0, 0.0) }),
                ("0x3".to_string(), NimbyStation { id: "0x3".to_string(), name: "Station C".to_string(), lonlat: (0.0, 0.0) }),
            ].into_iter().collect(),
            schedules: Vec::new(),
            lines: vec![NimbyLine {
                id: "0x100".to_string(),
                name: "Test Line".to_string(),
                code: "T1".to_string(),
                color: "0xffff0000".to_string(),
                stops: vec![
                    NimbyStop { idx: 0, leg_distance: 0.0, station_id: "0x1".to_string(), arrival: 0, departure: 60, areas: Vec::new() },
                    NimbyStop { idx: 1, leg_distance: 10000.0, station_id: "0x2".to_string(), arrival: 360, departure: 420, areas: Vec::new() },
                    NimbyStop { idx: 2, leg_distance: 10000.0, station_id: "0x3".to_string(), arrival: 720, departure: 780, areas: Vec::new() },
                ],
            }],
        };

        let config = NimbyImportConfig {
            create_infrastructure: false,
            update_existing: true,
            ..Default::default()
        };

        let mut edge_map = HashMap::new();

        // Update the existing line
        let result = update_existing_line(
            &mut existing_line,
            &nimby_data.lines[0],
            &nimby_data,
            &config,
            &mut graph,
            &mut edge_map,
        );

        assert!(result.is_ok(), "Update should succeed: {result:?}");

        // Verify the route now goes through the passing loop (3 segments: A->Loop, Loop->B, B->C)
        println!("Forward route after update: {} segments", existing_line.forward_route.len());
        for (i, seg) in existing_line.forward_route.iter().enumerate() {
            let endpoints = graph.graph.edge_endpoints(EdgeIndex::new(seg.edge_index));
            let (from, to) = endpoints.unwrap();
            let from_name = graph.graph[from].display_name();
            let to_name = graph.graph[to].display_name();
            println!("  Segment {}: {} -> {}, wait_time: {}s", i, from_name, to_name, seg.wait_time.num_seconds());
        }

        assert_eq!(existing_line.forward_route.len(), 3, "Should have 3 segments (A->Loop, Loop->B, B->C)");

        // Verify wait times are preserved for B and C
        // The segment ending at B should have preserved wait time of 2 minutes
        let seg_to_b = &existing_line.forward_route[1]; // Loop -> B
        let (_, to_b) = graph.graph.edge_endpoints(EdgeIndex::new(seg_to_b.edge_index)).unwrap();
        assert_eq!(graph.graph[to_b].display_name(), "Station B");
        assert_eq!(seg_to_b.wait_time.num_minutes(), 2, "Wait time at B should be preserved");

        // The segment ending at C should have preserved wait time of 3 minutes
        let seg_to_c = &existing_line.forward_route[2]; // B -> C
        let (_, to_c) = graph.graph.edge_endpoints(EdgeIndex::new(seg_to_c.edge_index)).unwrap();
        assert_eq!(graph.graph[to_c].display_name(), "Station C");
        assert_eq!(seg_to_c.wait_time.num_minutes(), 3, "Wait time at C should be preserved");

        // The new passing loop segment should have NIMBY timing (not preserved, since it's new)
        let seg_to_loop = &existing_line.forward_route[0]; // A -> Loop
        let (_, to_loop) = graph.graph.edge_endpoints(EdgeIndex::new(seg_to_loop.edge_index)).unwrap();
        assert_eq!(graph.graph[to_loop].display_name(), "Passing Loop");
        // Passing loop wait time comes from NIMBY or default (0 since it's intermediate)
        println!("Passing loop wait time: {}s", seg_to_loop.wait_time.num_seconds());
    }
}
