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
                .map(|p| p.total_distance);

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
            let distance = if total_distance > 0.0 { Some(total_distance) } else { None };
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

        // Get total segment distance from edge or first candidate
        let edge_weight = graph.graph.edge_weight(edge_idx).cloned();
        let total_distance = edge_weight
            .as_ref()
            .and_then(|w| w.distance)
            .unwrap_or(segment_loops[0].candidate.segment_distance);

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

        // Create all loops and edges for this segment
        let mut prev_node = chain_start;
        let mut prev_distance = 0.0;

        for norm_candidate in loop_iter {
            // If we're going in the opposite direction, flip the ratio
            let effective_ratio = if flip_ratios {
                1.0 - norm_candidate.normalized_ratio
            } else {
                norm_candidate.normalized_ratio
            };
            let loop_distance = total_distance * effective_ratio;

            // Create the passing loop station
            let loop_station = StationNode {
                name: "Passing Loop".to_string(),
                external_id: None,
                position: None,
                passing_loop: true,
                platforms: vec![crate::models::Platform { name: "1".to_string() }],
                label_position: None,
            };
            let loop_node = graph.graph.add_node(Node::Station(loop_station));

            // Create edge from previous node to this loop
            let edge_distance = loop_distance - prev_distance;
            let tracks = super::shared::create_tracks_with_count(1, handedness);
            graph.add_track(prev_node, loop_node, tracks, Some(edge_distance));

            prev_node = loop_node;
            prev_distance = loop_distance;
            loop_count += 1;
        }

        // Create final edge from last loop to destination station
        let final_distance = total_distance - prev_distance;
        let tracks = super::shared::create_tracks_with_count(1, handedness);
        graph.add_track(prev_node, chain_end, tracks, Some(final_distance));
    }

    loop_count
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

        // Skip depots, but accumulate their distance
        if is_depot(data, &stop.station_id) {
            accumulated_distance += stop.leg_distance;
            continue;
        }

        // Total leg distance includes any skipped waypoints and depots
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

/// Check if a station is a depot (should be excluded from infrastructure)
fn is_depot(data: &NimbyImportData, station_id: &str) -> bool {
    data.stations
        .get(station_id)
        .is_some_and(|s| s.name.contains("[DEP]"))
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
/// Minimum distance from station for passing loop waypoints (1km)
/// Waypoints closer than this are likely station throat routing, not passing loops
const MIN_PASSING_LOOP_DISTANCE_FROM_STATION: f64 = 1000.0;
/// Maximum distance mismatch for passing loop waypoint matching (700m)
/// Forward/return waypoints within this distance are considered the same passing loop
const PASSING_LOOP_WAYPOINT_TOLERANCE: f64 = 700.0;

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
