use crate::models::{Junction, Junctions, Node, RailwayGraph, Stations, Track, TrackDirection, TrackHandedness, Tracks};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::{Import, ImportMode, ImportResult};

// Infrastructure view grid constants - match infrastructure_view.rs
const GRID_SIZE: f64 = 30.0;
const BASE_STATION_SPACING: f64 = 120.0;
const TARGET_SIZE: f64 = BASE_STATION_SPACING * 20.0; // Reasonable default map size
const MAX_STATION_DISTANCE: f64 = 15.0; // 15px tolerance - half a grid cell
const SAMPLE_DISTANCE: f64 = 10.0; // Check for stations every 10px along the line

pub struct GeoJsonImport;

#[derive(Debug, Clone)]
pub struct StationInfo {
    pub name: String,
    pub lat: f64,
    pub lng: f64,
}

impl GeoJsonImport {
    /// Extract station information from `GeoJSON` for preview/selection
    ///
    /// # Errors
    /// Returns error if `GeoJSON` is invalid or missing required fields
    pub fn extract_stations(parsed: &Value) -> Result<Vec<StationInfo>, String> {
        let features = parsed["features"]
            .as_array()
            .ok_or("Invalid GeoJSON: missing 'features' array")?;

        let mut stations = Vec::new();

        for feature in features {
            if is_station_feature(feature) {
                let name = extract_station_name(feature)?;
                let coords = extract_point_coords(feature)?; // (lng, lat)

                stations.push(StationInfo {
                    name,
                    lng: coords.0,
                    lat: coords.1,
                });
            }
        }

        Ok(stations)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GeoJsonConfig {
    pub create_infrastructure: bool,
    pub bounds: Option<(f64, f64, f64, f64)>, // (min_lat, min_lng, max_lat, max_lng)
}

impl Default for GeoJsonConfig {
    fn default() -> Self {
        Self {
            create_infrastructure: true,
            bounds: None,
        }
    }
}

impl Import for GeoJsonImport {
    type Config = GeoJsonConfig;
    type Parsed = Value;
    type ParseError = serde_json::Error;

    fn parse(content: &str) -> Result<Self::Parsed, Self::ParseError> {
        serde_json::from_str(content)
    }

    fn analyze(_content: &str, _filename: Option<String>) -> Option<Self::Config> {
        Some(GeoJsonConfig::default())
    }

    fn import(
        parsed: &Self::Parsed,
        config: &Self::Config,
        mode: ImportMode,
        graph: &mut RailwayGraph,
        _existing_line_count: usize,
        _existing_line_ids: &[String],
        _handedness: TrackHandedness,
    ) -> Result<ImportResult, String> {
        if !config.create_infrastructure {
            return Err("GeoJSON import requires infrastructure creation".to_string());
        }

        if !matches!(mode, ImportMode::CreateInfrastructure) {
            return Err(
                "GeoJSON import only supports CreateInfrastructure mode".to_string()
            );
        }

        leptos::logging::log!("Parsing features array...");
        let features = parsed["features"]
            .as_array()
            .ok_or("Invalid GeoJSON: missing 'features' array")?;
        leptos::logging::log!("Features array parsed: {} features", features.len());

        let mut stations_added = 0;
        let mut edges_added = 0;

        // Use HashMap to deduplicate by ID
        let mut station_data: HashMap<String, (String, (f64, f64))> = HashMap::new();

        // Pass 1: Collect unique stations by ID (with bounds filtering if specified)
        for feature in features {
            if !is_station_feature(feature) {
                continue;
            }

            let id = extract_station_id(feature)?;
            let name = extract_station_name(feature)?;
            let coords = extract_point_coords(feature)?; // (lng, lat)

            // Filter by bounds if specified
            if let Some((min_lat, min_lng, max_lat, max_lng)) = config.bounds {
                let lat = coords.1;
                let lng = coords.0;
                if lat < min_lat || lat > max_lat || lng < min_lng || lng > max_lng {
                    continue; // Skip stations outside bounds
                }
            }

            // Store by ID - duplicates are automatically handled
            station_data.insert(id, (name, coords));
        }

        // Check station count limit
        if station_data.len() > 1000 {
            return Err(format!(
                "Too many stations in selection: {}. Maximum is 1000. Please select a smaller region.",
                station_data.len()
            ));
        }

        leptos::logging::log!("Station collection complete: {} unique stations", station_data.len());

        // Extract positions for transform calculation
        let positions: Vec<(f64, f64)> = station_data.values().map(|(_, coords)| *coords).collect();

        // Calculate transform
        let transform = if positions.is_empty() {
            CoordinateTransform::default()
        } else {
            calculate_coordinate_transform(&positions)
        };

        // Build list of stations with their snapped positions for nearest-neighbor search
        let mut station_list: Vec<(petgraph::stable_graph::NodeIndex, (f64, f64))> =
            Vec::with_capacity(station_data.len());

        // Pass 2: Add unique stations and build lookup list
        for (id, (name, coords)) in &station_data {
            let idx = graph.add_or_get_station(id.clone());

            if let Some(Node::Station(ref mut station)) = graph.graph.node_weight_mut(idx) {
                station.name.clone_from(name);
            }

            // Use raw transformed coordinates for nearest-neighbor search
            // Don't snap to grid yet - that happens during layout
            let transformed = transform.apply(*coords);

            // Store in list for nearest-neighbor search (but don't set position yet)
            station_list.push((idx, transformed));
            stations_added += 1;
        }

        // Pass 2.5: Detect linestring junctions and create junction nodes
        leptos::logging::log!("Detecting linestring junctions...");
        let junction_count = detect_and_create_junctions(features, graph, &transform, &mut station_list)?;
        leptos::logging::log!("Created {} junction nodes", junction_count);
        stations_added += junction_count;

        // Pass 3: Import tracks using nearest-neighbor matching
        leptos::logging::log!("Importing tracks from GeoJSON, {} stations loaded", station_list.len());
        let mut track_features = 0;
        for feature in features {
            if is_tracks_feature(feature) {
                track_features += 1;
                leptos::logging::log!("Processing track feature {}", track_features);
                edges_added += import_track_feature(feature, graph, &transform, &station_list)?;
                leptos::logging::log!("Track feature {} done, {} total edges so far", track_features, edges_added);
            }
        }
        leptos::logging::log!("Track import complete: {} edges added from {} track features", edges_added, track_features);

        // Pass 3.5: Clean up depot through-routes
        remove_depot_through_routes(graph);

        // Pass 4: Layout stations based on track topology (transit diagram style)
        layout_stations_topological(graph, &station_list);

        Ok(ImportResult {
            lines: Vec::new(),
            stations_added,
            edges_added,
        })
    }
}

const JUNCTION_DISTANCE: f64 = 20.0;
const MAX_DUPLICATE_DISTANCE: f64 = 5.0; // Only skip if within 5px of existing junction/station

/// Helper to check if an endpoint is close enough to another linestring to form a junction
/// A junction is created when:
/// - A linestring from station A has an endpoint
/// - That endpoint is close to the middle of a linestring between stations B and C
/// - Where A is not B and A is not C
#[allow(clippy::too_many_arguments)]
fn check_and_create_junction(
    line_idx: usize,
    endpoint_idx: usize,
    endpoint: (f64, f64),
    other_line_idx: usize,
    other_line: &[(f64, f64)],
    line_endpoint_stations: &[(Option<petgraph::stable_graph::NodeIndex>, Option<petgraph::stable_graph::NodeIndex>)],
    junctions_created: &mut usize,
    processed_pairs: &mut std::collections::HashSet<(usize, usize, usize, usize)>,
    station_list: &mut Vec<(petgraph::stable_graph::NodeIndex, (f64, f64))>,
    graph: &mut RailwayGraph,
) {

    // Find minimum distance to other_line
    let mut min_dist = f64::MAX;
    let mut closest_idx = 0;

    for (idx, &point) in other_line.iter().enumerate() {
        let dx = endpoint.0 - point.0;
        let dy = endpoint.1 - point.1;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < min_dist {
            min_dist = dist;
            closest_idx = idx;
        }
    }

    // If not close enough, not a junction
    if min_dist >= JUNCTION_DISTANCE {
        return;
    }

    // Don't create junctions at endpoints of the other line
    // Only create junctions when an endpoint connects to the MIDDLE of another line
    let is_at_other_endpoint = closest_idx == 0 || closest_idx == other_line.len() - 1;
    if is_at_other_endpoint {
        return;
    }

    // Get the stations at the endpoints of both linestrings
    let my_stations = &line_endpoint_stations[line_idx];
    let other_stations = &line_endpoint_stations[other_line_idx];

    // Get the station at this endpoint (start or end)
    let my_endpoint_station = if endpoint_idx == 0 {
        my_stations.0
    } else {
        my_stations.1
    };

    // Check if my endpoint station matches either endpoint station of the other line
    // If they match, these are parallel tracks or tracks sharing a station, not a junction
    if let Some(my_station) = my_endpoint_station {
        if other_stations.0 == Some(my_station) || other_stations.1 == Some(my_station) {
            return;
        }
    }

    // Check if we already processed this junction
    let key = (line_idx, endpoint_idx, other_line_idx, closest_idx);
    if processed_pairs.contains(&key) {
        return;
    }
    processed_pairs.insert(key);

    // Junction point is midpoint between the two closest points
    let junction_point = (
        (endpoint.0 + other_line[closest_idx].0) / 2.0,
        (endpoint.1 + other_line[closest_idx].1) / 2.0,
    );

    // Check if there's already a station near this junction
    let nearest_station_dist = station_list.iter()
        .map(|(_, station_pos)| {
            let dx = junction_point.0 - station_pos.0;
            let dy = junction_point.1 - station_pos.1;
            (dx * dx + dy * dy).sqrt()
        })
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(f64::MAX);

    // Only skip junction creation if there's a station VERY close (right on top of the junction)
    if nearest_station_dist < MAX_DUPLICATE_DISTANCE {
        return;
    }

    // Create new junction node
    let junction = Junction {
        name: Some(format!("Junction {}", *junctions_created + 1)),
        position: Some(junction_point),
        routing_rules: vec![],
    };
    let node_idx = graph.add_junction(junction);

    station_list.push((node_idx, junction_point));

    *junctions_created += 1;
    leptos::logging::log!("  Created junction at ({:.1}, {:.1}) between linestrings {} and {} (distance: {:.1}px)",
        junction_point.0, junction_point.1, line_idx, other_line_idx, min_dist);
}

/// Detect where linestrings come very close to each other (junctions) and create junction nodes
fn detect_and_create_junctions(
    features: &[Value],
    graph: &mut RailwayGraph,
    transform: &CoordinateTransform,
    station_list: &mut Vec<(petgraph::stable_graph::NodeIndex, (f64, f64))>,
) -> Result<usize, String> {
    // Collect all track linestrings
    let mut linestrings: Vec<Vec<(f64, f64)>> = Vec::new();

    for feature in features {
        if !is_tracks_feature(feature) {
            continue;
        }

        let coords = extract_multilinestring_coords(feature)?;
        for linestring in coords {
            if linestring.len() < 2 {
                continue;
            }

            // Transform all coords
            let transformed: Vec<(f64, f64)> = linestring
                .iter()
                .map(|&c| transform.apply(c))
                .collect();

            linestrings.push(transformed);
        }
    }

    leptos::logging::log!("  Checking {} linestrings for junctions", linestrings.len());

    // For each linestring, find the stations at its endpoints
    let mut line_endpoint_stations: Vec<(Option<petgraph::stable_graph::NodeIndex>, Option<petgraph::stable_graph::NodeIndex>)> = Vec::new();

    for linestring in &linestrings {
        let start = linestring.first().copied().unwrap_or((0.0, 0.0));
        let end = linestring.last().copied().unwrap_or((0.0, 0.0));

        // Find the closest station to the start and end points
        let start_station = station_list.iter()
            .filter(|(node_idx, _)| {
                // Only consider actual stations, not junctions (junctions haven't been created yet)
                graph.graph.node_weight(*node_idx).is_some_and(Node::is_station)
            })
            .min_by(|(_, a_pos), (_, b_pos)| {
                let dist_a = ((start.0 - a_pos.0).powi(2) + (start.1 - a_pos.1).powi(2)).sqrt();
                let dist_b = ((start.0 - b_pos.0).powi(2) + (start.1 - b_pos.1).powi(2)).sqrt();
                dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|(node_idx, pos)| {
                let dist = ((start.0 - pos.0).powi(2) + (start.1 - pos.1).powi(2)).sqrt();
                if dist < MAX_STATION_DISTANCE { Some(*node_idx) } else { None }
            });

        let end_station = station_list.iter()
            .filter(|(node_idx, _)| {
                graph.graph.node_weight(*node_idx).is_some_and(Node::is_station)
            })
            .min_by(|(_, a_pos), (_, b_pos)| {
                let dist_a = ((end.0 - a_pos.0).powi(2) + (end.1 - a_pos.1).powi(2)).sqrt();
                let dist_b = ((end.0 - b_pos.0).powi(2) + (end.1 - b_pos.1).powi(2)).sqrt();
                dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|(node_idx, pos)| {
                let dist = ((end.0 - pos.0).powi(2) + (end.1 - pos.1).powi(2)).sqrt();
                if dist < MAX_STATION_DISTANCE { Some(*node_idx) } else { None }
            });

        line_endpoint_stations.push((start_station, end_station));
    }

    let mut junctions_created = 0;
    let mut processed_pairs: std::collections::HashSet<(usize, usize, usize, usize)> = std::collections::HashSet::new();

    // For each pair of linestrings, check if they come close together
    for i in 0..linestrings.len() {
        for j in (i + 1)..linestrings.len() {
            let line_a = &linestrings[i];
            let line_b = &linestrings[j];

            // Check distance between endpoints of both lines and all points on the other line
            // Check line_a endpoints against line_b points
            for (endpoint_idx, &endpoint) in [line_a.first(), line_a.last()].iter().enumerate() {
                let Some(&endpoint) = endpoint else { continue };

                check_and_create_junction(
                    i, endpoint_idx, endpoint,
                    j, line_b,
                    &line_endpoint_stations,
                    &mut junctions_created,
                    &mut processed_pairs,
                    station_list,
                    graph,
                );
            }

            // Check line_b endpoints against line_a points
            for (endpoint_idx, &endpoint) in [line_b.first(), line_b.last()].iter().enumerate() {
                let Some(&endpoint) = endpoint else { continue };

                check_and_create_junction(
                    j, endpoint_idx, endpoint,
                    i, line_a,
                    &line_endpoint_stations,
                    &mut junctions_created,
                    &mut processed_pairs,
                    station_list,
                    graph,
                );
            }
        }
    }

    Ok(junctions_created)
}

/// Layout stations in transit diagram style:
/// - Uniform spacing between connected stations
/// - Lines snap to 8 cardinal/diagonal directions based on geography
/// - Grid-aligned
fn layout_stations_topological(
    graph: &mut RailwayGraph,
    station_list: &[(petgraph::stable_graph::NodeIndex, (f64, f64))],
) {
    use petgraph::visit::{EdgeRef, IntoEdgeReferences};
    use std::collections::{HashMap, HashSet, VecDeque};

    if station_list.is_empty() {
        return;
    }

    // Build map of station -> geographic position for direction calculation
    let geo_positions: HashMap<_, _> = station_list.iter().copied().collect();

    // Track which stations have been positioned
    let mut positioned: HashMap<petgraph::stable_graph::NodeIndex, (f64, f64)> = HashMap::new();
    let mut occupied_grids: HashSet<(i32, i32)> = HashSet::new();

    // Get all node indices
    let all_nodes: Vec<_> = graph.graph.node_indices().collect();

    // Start from first node
    if let Some(&start_node) = all_nodes.first() {
        // Position start node at origin
        let start_pos = (0.0, 0.0);
        let start_grid = coord_to_grid(start_pos);
        positioned.insert(start_node, start_pos);
        occupied_grids.insert(start_grid);

        // BFS to position all connected stations
        let mut queue = VecDeque::new();
        queue.push_back(start_node);

        while let Some(current_node) = queue.pop_front() {
            let current_pos = positioned[&current_node];
            let current_geo = geo_positions.get(&current_node).copied().unwrap_or((0.0, 0.0));

            // Get all neighbors (treat graph as undirected)
            let mut neighbors = Vec::new();

            // Outgoing edges
            for edge in graph.graph.edges(current_node) {
                neighbors.push(edge.target());
            }

            // Incoming edges (to treat as undirected)
            for edge_ref in graph.graph.edge_references() {
                if edge_ref.target() == current_node {
                    neighbors.push(edge_ref.source());
                }
            }

            for neighbor in neighbors {
                // Skip if already positioned
                if positioned.contains_key(&neighbor) {
                    continue;
                }

                let neighbor_geo = geo_positions.get(&neighbor).copied().unwrap_or((0.0, 0.0));

                // Calculate geographic direction
                let dx = neighbor_geo.0 - current_geo.0;
                let dy = neighbor_geo.1 - current_geo.1;

                // Snap to one of 8 directions
                let direction = snap_to_8_directions(dx, dy);

                // Calculate new position at uniform distance
                let new_pos = (
                    current_pos.0 + direction.0 * BASE_STATION_SPACING,
                    current_pos.1 + direction.1 * BASE_STATION_SPACING,
                );

                // Snap to grid and handle collisions
                let ideal_grid = coord_to_grid(new_pos);
                let final_grid = find_free_grid_cell(ideal_grid, &occupied_grids);
                let final_pos = grid_to_coord(final_grid);

                positioned.insert(neighbor, final_pos);
                occupied_grids.insert(final_grid);
                queue.push_back(neighbor);
            }
        }
    }

    // Apply positions to graph
    for (node_idx, pos) in positioned {
        graph.set_station_position(node_idx, pos);
    }
}

/// Snap a direction vector to one of 8 cardinal/diagonal directions
fn snap_to_8_directions(dx: f64, dy: f64) -> (f64, f64) {
    if dx.abs() < 0.0001 && dy.abs() < 0.0001 {
        return (1.0, 0.0); // Default to east
    }

    let angle = dy.atan2(dx);
    let snapped_angle = (angle / (std::f64::consts::PI / 4.0)).round() * (std::f64::consts::PI / 4.0);
    (snapped_angle.cos(), snapped_angle.sin())
}

/// Find a free grid cell, starting from ideal position and spiraling outward
fn find_free_grid_cell(ideal: (i32, i32), occupied: &std::collections::HashSet<(i32, i32)>) -> (i32, i32) {
    if !occupied.contains(&ideal) {
        return ideal;
    }

    // Try small offsets first - prefer cardinal directions over diagonal
    let nearby_offsets = [
        (1, 0), (-1, 0), (0, 1), (0, -1),  // Cardinal neighbors
        (1, 1), (1, -1), (-1, 1), (-1, -1), // Diagonal neighbors
        (2, 0), (-2, 0), (0, 2), (0, -2),  // 2 cells away cardinal
    ];

    for (dx, dy) in nearby_offsets {
        let test_grid = (ideal.0 + dx, ideal.1 + dy);
        if !occupied.contains(&test_grid) {
            return test_grid;
        }
    }

    // If nearby cells occupied, try expanding search radius
    for radius in 3_i32..=5 {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                if dx.abs().max(dy.abs()) != radius {
                    continue; // Only check perimeter at this radius
                }
                let test_grid = (ideal.0 + dx, ideal.1 + dy);
                if !occupied.contains(&test_grid) {
                    return test_grid;
                }
            }
        }
    }

    // If all cells within radius 5 are occupied, just return ideal
    ideal
}

fn is_station_feature(feature: &Value) -> bool {
    feature["properties"]["preview_type"].as_str() == Some("station")
}

fn is_tracks_feature(feature: &Value) -> bool {
    feature["properties"]["preview_type"].as_str() == Some("tracks")
}

/// Remove depot stations that are incorrectly being used as through stations
#[allow(dead_code, clippy::cast_precision_loss)]
fn remove_depot_through_routes(graph: &mut RailwayGraph) {
    use petgraph::visit::{EdgeRef, IntoEdgeReferences};
    use std::collections::{HashMap, HashSet};

    const MIN_TRACKS_FOR_DEPOT: usize = 10;

    // Find stations with high track count edges
    let mut station_max_tracks: HashMap<petgraph::stable_graph::NodeIndex, usize> = HashMap::new();

    for edge in graph.graph.edge_references() {
        let track_count = edge.weight().tracks.len();
        let source_max = station_max_tracks.entry(edge.source()).or_insert(0);
        *source_max = (*source_max).max(track_count);
        let target_max = station_max_tracks.entry(edge.target()).or_insert(0);
        *target_max = (*target_max).max(track_count);
    }

    // Find depots being used as through stations (have high track count AND >2 neighbors)
    let mut depots_to_remove = Vec::new();

    for (node, &max_tracks) in &station_max_tracks {
        if max_tracks < MIN_TRACKS_FOR_DEPOT {
            continue;
        }

        // Count neighbors
        let mut neighbors = HashSet::new();
        for edge in graph.graph.edges(*node) {
            neighbors.insert(edge.target());
        }
        for edge in graph.graph.edge_references() {
            if edge.target() == *node {
                neighbors.insert(edge.source());
            }
        }

        // Count total tracks
        let mut total_tracks = 0;
        for edge in graph.graph.edges(*node) {
            total_tracks += edge.weight().tracks.len();
        }
        for edge in graph.graph.edge_references() {
            if edge.target() == *node {
                total_tracks += edge.weight().tracks.len();
            }
        }

        if let Some(Node::Station(station)) = graph.graph.node_weight(*node) {
            let ratio = if total_tracks > 0 {
                max_tracks as f64 / total_tracks as f64
            } else {
                0.0
            };

            leptos::logging::log!("Checking depot candidate: {} ({} neighbors, {} of {} tracks = {:.1}%)",
                station.name, neighbors.len(), max_tracks, total_tracks, ratio * 100.0);

            // A depot has most tracks (>75%) going to one neighbor AND has another connection
            // This means it's a through-route via the depot
            if neighbors.len() >= 2 && ratio > 0.75 {
                leptos::logging::log!("Removing depot through-route: {} ({} neighbors, {:.1}% to one)",
                    station.name, neighbors.len(), ratio * 100.0);
                depots_to_remove.push(*node);
            }
        }
    }

    // Remove depot nodes
    for depot_node in depots_to_remove {
        graph.graph.remove_node(depot_node);
    }
}

fn find_nearest_station(
    pos: (f64, f64),
    stations: &[(petgraph::stable_graph::NodeIndex, (f64, f64))],
    max_distance: f64,
) -> Option<petgraph::stable_graph::NodeIndex> {
    let mut nearest_idx = None;
    let mut min_distance = max_distance;

    for &(idx, station_pos) in stations {
        let dx = pos.0 - station_pos.0;
        let dy = pos.1 - station_pos.1;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance < min_distance {
            min_distance = distance;
            nearest_idx = Some(idx);
        }
    }

    nearest_idx
}

/// Finalize a station if it has enough consecutive matches
fn finalize_station_if_valid(
    waypoints: &mut Vec<petgraph::stable_graph::NodeIndex>,
    station_idx: petgraph::stable_graph::NodeIndex,
    consecutive_count: usize,
    min_required: usize,
) {
    if consecutive_count >= min_required
        && (waypoints.is_empty() || waypoints.last() != Some(&station_idx))
    {
        waypoints.push(station_idx);
    }
}

/// Add a waypoint if it's not a duplicate of the last one
fn add_waypoint_if_unique(
    waypoints: &mut Vec<petgraph::stable_graph::NodeIndex>,
    station_idx: petgraph::stable_graph::NodeIndex,
) {
    if waypoints.is_empty() || waypoints.last() != Some(&station_idx) {
        waypoints.push(station_idx);
    }
}

fn import_track_feature(
    feature: &Value,
    graph: &mut RailwayGraph,
    transform: &CoordinateTransform,
    station_list: &[(petgraph::stable_graph::NodeIndex, (f64, f64))],
) -> Result<usize, String> {
    const MIN_CONSECUTIVE_MATCHES: usize = 2;

    let linestrings = extract_multilinestring_coords(feature)?;
    leptos::logging::log!("  Processing {} linestrings", linestrings.len());
    let mut edges_added = 0;

    for (idx, linestring) in linestrings.iter().enumerate() {
        if linestring.len() < 2 {
            continue;
        }

        if idx % 10 == 0 {
            leptos::logging::log!("    Linestring {}/{}", idx, linestrings.len());
        }

        // Find all waypoint stations along this linestring
        let mut waypoints = Vec::new();
        let mut distance_since_last_check = 0.0;
        let mut last_checked_pos: Option<(f64, f64)> = None;
        let mut last_matched_station: Option<petgraph::stable_graph::NodeIndex> = None;
        let mut consecutive_match_count = 0_usize;

        for (point_idx, &coords) in linestring.iter().enumerate() {
            let transformed_coords = transform.apply(coords);
            let is_first = point_idx == 0;
            let is_last = point_idx == linestring.len() - 1;

            // Always check first and last points, or when we've traveled SAMPLE_DISTANCE
            let should_check = if is_first || is_last {
                true
            } else if let Some(last_pos) = last_checked_pos {
                let dx = transformed_coords.0 - last_pos.0;
                let dy = transformed_coords.1 - last_pos.1;
                let dist = (dx * dx + dy * dy).sqrt();
                distance_since_last_check += dist;

                if distance_since_last_check >= SAMPLE_DISTANCE {
                    distance_since_last_check = 0.0;
                    true
                } else {
                    false
                }
            } else {
                true
            };

            last_checked_pos = Some(transformed_coords);

            if !should_check {
                continue;
            }

            // Check if this point is near any station
            let station_match = find_nearest_station(transformed_coords, station_list, MAX_STATION_DISTANCE);

            let Some(station_idx) = station_match else {
                // No match - finalize previous station if it had enough consecutive matches
                if let Some(prev_station) = last_matched_station {
                    finalize_station_if_valid(&mut waypoints, prev_station, consecutive_match_count, MIN_CONSECUTIVE_MATCHES);
                }
                last_matched_station = None;
                consecutive_match_count = 0;
                continue;
            };

            // Track consecutive matches to same station
            if last_matched_station == Some(station_idx) {
                consecutive_match_count += 1;
            } else {
                // Different station - finalize previous if it had enough consecutive matches
                if let Some(prev_station) = last_matched_station {
                    finalize_station_if_valid(&mut waypoints, prev_station, consecutive_match_count, MIN_CONSECUTIVE_MATCHES);
                }

                // Start tracking new station
                last_matched_station = Some(station_idx);
                consecutive_match_count = 1;
            }

            // Always add endpoints (first/last) immediately regardless of consecutive count
            if is_first || is_last {
                add_waypoint_if_unique(&mut waypoints, station_idx);
            }
        }

        // Finalize last matched station at end of linestring
        if let Some(station_idx) = last_matched_station {
            finalize_station_if_valid(&mut waypoints, station_idx, consecutive_match_count, MIN_CONSECUTIVE_MATCHES);
        }

        // Need at least 2 waypoints to create edges
        if waypoints.len() < 2 {
            continue;
        }

        // Log waypoints for this linestring
        let waypoint_names: Vec<String> = waypoints.iter()
            .map(|&idx| {
                graph.graph.node_weight(idx)
                    .map_or_else(|| "Unknown".to_string(), Node::display_name)
            })
            .collect();
        leptos::logging::log!("    Linestring waypoints: {}", waypoint_names.join(" -> "));

        // Create edges between consecutive waypoints
        // If edge already exists, add a parallel track
        for i in 0..waypoints.len() - 1 {
            let start_idx = waypoints[i];
            let end_idx = waypoints[i + 1];

            if start_idx == end_idx {
                continue; // Skip self-loops
            }

            // Get station names for logging
            let start_name = graph.graph.node_weight(start_idx)
                .and_then(|n| if let crate::models::Node::Station(s) = n { Some(s.name.clone()) } else { None })
                .unwrap_or_else(|| "Unknown".to_string());
            let end_name = graph.graph.node_weight(end_idx)
                .and_then(|n| if let crate::models::Node::Station(s) = n { Some(s.name.clone()) } else { None })
                .unwrap_or_else(|| "Unknown".to_string());

            // Check if edge already exists in either direction (since tracks are bidirectional)
            let edge_idx = graph.graph.find_edge(start_idx, end_idx)
                .or_else(|| graph.graph.find_edge(end_idx, start_idx));

            if let Some(edge_idx) = edge_idx {
                // Edge exists - add another track to it (parallel track)
                if let Some(edge_weight) = graph.graph.edge_weight_mut(edge_idx) {
                    let track = Track {
                        direction: TrackDirection::Bidirectional,
                    };
                    edge_weight.tracks.push(track);
                    let track_count = edge_weight.tracks.len();
                    leptos::logging::log!("      Added parallel track {} to {} - {} (now {} tracks)",
                        track_count, start_name, end_name, track_count);
                }
            } else {
                // Create new edge with one track
                let track = Track {
                    direction: TrackDirection::Bidirectional,
                };
                graph.add_track(start_idx, end_idx, vec![track]);
                edges_added += 1;
                leptos::logging::log!("      Created new edge {} - {} (1 track)", start_name, end_name);
            }
        }
    }

    Ok(edges_added)
}

fn extract_station_name(feature: &Value) -> Result<String, String> {
    feature["properties"]["name"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| "Station feature missing 'name' property".to_string())
}

fn extract_station_id(feature: &Value) -> Result<String, String> {
    feature["properties"]["id"]
        .as_str()
        .map(String::from)
        .or_else(|| feature["properties"]["id"].as_i64().map(|i| i.to_string()))
        .or_else(|| feature["properties"]["id"].as_u64().map(|u| u.to_string()))
        .ok_or_else(|| "Station feature missing 'id' property".to_string())
}

fn extract_point_coords(feature: &Value) -> Result<(f64, f64), String> {
    let coords = feature["geometry"]["coordinates"]
        .as_array()
        .ok_or("Invalid Point geometry: missing coordinates")?;

    if coords.len() < 2 {
        return Err("Invalid Point geometry: insufficient coordinates".to_string());
    }

    let lon = coords[0]
        .as_f64()
        .ok_or("Invalid longitude coordinate")?;
    let lat = coords[1]
        .as_f64()
        .ok_or("Invalid latitude coordinate")?;

    Ok((lon, lat))
}

fn extract_multilinestring_coords(feature: &Value) -> Result<Vec<Vec<(f64, f64)>>, String> {
    let coordinates = feature["geometry"]["coordinates"]
        .as_array()
        .ok_or("Invalid MultiLineString geometry: missing coordinates")?;

    let mut linestrings = Vec::new();

    for linestring in coordinates {
        let points = linestring
            .as_array()
            .ok_or("Invalid LineString in MultiLineString")?;

        let mut coords = Vec::new();
        for point in points {
            let point_arr = point
                .as_array()
                .ok_or("Invalid point in LineString")?;

            if point_arr.len() < 2 {
                continue;
            }

            let lon = point_arr[0]
                .as_f64()
                .ok_or("Invalid longitude in LineString")?;
            let lat = point_arr[1]
                .as_f64()
                .ok_or("Invalid latitude in LineString")?;

            coords.push((lon, lat));
        }

        if !coords.is_empty() {
            linestrings.push(coords);
        }
    }

    Ok(linestrings)
}

#[allow(clippy::cast_possible_truncation)]
fn coord_to_grid(coords: (f64, f64)) -> (i32, i32) {
    // Round to grid cells (GRID_SIZE = 30px) for spatial lookup
    // This ensures stations and track endpoints use the same rounding
    ((coords.0 / GRID_SIZE).round() as i32, (coords.1 / GRID_SIZE).round() as i32)
}

fn grid_to_coord(grid: (i32, i32)) -> (f64, f64) {
    // Convert grid cell back to coordinate (center of cell)
    // Snap to GRID_SIZE (30px) for proper alignment with infrastructure view
    (f64::from(grid.0) * GRID_SIZE, f64::from(grid.1) * GRID_SIZE)
}

struct CoordinateTransform {
    offset_x: f64,
    offset_y: f64,
    scale: f64,
}

impl Default for CoordinateTransform {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            scale: 1.0,
        }
    }
}

impl CoordinateTransform {
    fn apply(&self, coords: (f64, f64)) -> (f64, f64) {
        let x = (coords.0 + self.offset_x) * self.scale;
        let y = (coords.1 + self.offset_y) * self.scale;
        // Invert Y so north is up (canvas Y increases downward)
        (x, -y)
    }
}

fn calculate_coordinate_transform(positions: &[(f64, f64)]) -> CoordinateTransform {
    if positions.is_empty() {
        return CoordinateTransform::default();
    }

    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;

    for &(x, y) in positions {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;

    let width = max_x - min_x;
    let height = max_y - min_y;
    let max_dimension = width.max(height);

    let scale = if max_dimension > 0.0 {
        TARGET_SIZE / max_dimension
    } else {
        1.0
    };

    CoordinateTransform {
        offset_x: -center_x,
        offset_y: -center_y,
        scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_geojson() {
        let content = r#"{"type": "FeatureCollection", "features": []}"#;
        let result = GeoJsonImport::parse(content);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_json() {
        let content = r#"{"type": "FeatureCollection", "features": []"#;
        let result = GeoJsonImport::parse(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_coordinate_transform() {
        let positions = vec![(0.0, 0.0), (10.0, 10.0)];
        let transform = calculate_coordinate_transform(&positions);

        assert!((transform.offset_x - (-5.0)).abs() < 0.001);
        assert!((transform.offset_y - (-5.0)).abs() < 0.001);
        assert!(transform.scale > 0.0);
    }

    #[test]
    fn test_bergen_import() {
        use petgraph::visit::{IntoEdgeReferences, EdgeRef};
        use crate::models::RailwayGraph;

        let json = std::fs::read_to_string("test-data/bergen.json")
            .expect("Failed to read bergen.json");

        // Parse JSON first
        let parsed = GeoJsonImport::parse(&json).expect("Failed to parse JSON");

        let config = GeoJsonConfig {
            create_infrastructure: true,
            bounds: None,
        };

        let mut graph = RailwayGraph::default();
        let result = GeoJsonImport::import(
            &parsed,
            &config,
            ImportMode::CreateInfrastructure,
            &mut graph,
            0,
            &[],
            crate::models::TrackHandedness::RightHand,
        );

        assert!(result.is_ok(), "Import failed: {:?}", result.err());
        let import_result = result.expect("result should be Ok");

        println!("Stations added: {}", import_result.stations_added);
        println!("Edges added: {}", import_result.edges_added);

        // Check specific edges
        for edge in graph.graph.edge_references() {
            let source = edge.source();
            let target = edge.target();
            let source_name = graph.graph.node_weight(source)
                .and_then(|n| if let crate::models::Node::Station(s) = n { Some(s.name.as_str()) } else { None })
                .unwrap_or("Unknown");
            let target_name = graph.graph.node_weight(target)
                .and_then(|n| if let crate::models::Node::Station(s) = n { Some(s.name.as_str()) } else { None })
                .unwrap_or("Unknown");

            let track_count = edge.weight().tracks.len();

            // Log edges involving Bergen stations or Møllendal
            if source_name.contains("Bergen") || target_name.contains("Bergen")
                || source_name.contains("Møllendal") || target_name.contains("Møllendal")
                || source_name.contains("Nygård") || target_name.contains("Nygård")
            {
                println!("{source_name} -> {target_name}: {track_count} tracks");
            }
        }
    }

    #[test]
    fn test_is_station_feature() {
        let feature: Value = serde_json::json!({
            "properties": {"preview_type": "station", "name": "Test"}
        });
        assert!(is_station_feature(&feature));
    }

    #[test]
    fn test_is_tracks_feature() {
        let feature: Value = serde_json::json!({
            "properties": {"preview_type": "tracks"}
        });
        assert!(is_tracks_feature(&feature));
    }

    #[test]
    fn test_import_bergen() {
        // Load the bergen.json test file
        let content = std::fs::read_to_string("test-data/bergen.json")
            .expect("Failed to read bergen.json");

        // Parse the GeoJSON
        let parsed = GeoJsonImport::parse(&content)
            .expect("Failed to parse bergen.json");

        // Extract stations to verify file structure
        let stations = GeoJsonImport::extract_stations(&parsed)
            .expect("Failed to extract stations");
        println!("Extracted {} stations from bergen.json", stations.len());
        assert_eq!(stations.len(), 61, "Should have 61 stations");

        // Create a test graph
        let mut graph = RailwayGraph::new();

        // Create import config
        let config = GeoJsonConfig {
            create_infrastructure: true,
            bounds: None,
        };

        // Run the import
        let result = GeoJsonImport::import(
            &parsed,
            &config,
            ImportMode::CreateInfrastructure,
            &mut graph,
            0,
            &[],
            TrackHandedness::RightHand,
        ).expect("Import failed");

        println!("Import result:");
        println!("  Stations added: {}", result.stations_added);
        println!("  Edges added: {}", result.edges_added);
        println!("  Graph now has {} stations", graph.graph.node_count());
        println!("  Graph now has {} edges", graph.graph.edge_count());

        // Verify stations were imported
        // Bergen.json has 61 unique station IDs (even though "Nygård" name appears twice)
        assert_eq!(result.stations_added, 61, "Should process 61 station features");
        assert_eq!(graph.graph.node_count(), 61, "Graph should have 61 unique stations by ID");

        // Verify tracks were imported
        // Bergen has 111 LineStrings but many don't connect to stations within tolerance
        assert!(result.edges_added > 0, "Should import at least some tracks");
        assert!(graph.graph.edge_count() > 0, "Graph should have at least some edges");
        println!("✓ Successfully imported {} tracks", result.edges_added);
    }
}

// Worker message types for offloading import to web worker

/// Represents a graph operation that can be applied by the main thread
#[derive(Clone, Serialize, Deserialize)]
pub enum GraphUpdate {
    AddStation {
        id: String,
        name: String,
        position: (f64, f64),
    },
    AddTrack {
        start_id: String,
        end_id: String,
        bidirectional: bool,
    },
    AddParallelTrack {
        start_id: String,
        end_id: String,
        bidirectional: bool,
    },
}

/// Request sent to the `GeoJSON` import worker
#[derive(Clone, Serialize, Deserialize)]
pub struct GeoJsonImportRequest {
    pub geojson_string: String,
    pub config: GeoJsonConfig,
}

/// Response from the `GeoJSON` import worker
#[derive(Serialize, Deserialize)]
pub struct GeoJsonImportResponse {
    pub result: Result<(), String>,
    pub updates: Vec<GraphUpdate>,
    pub stations_added: usize,
    pub edges_added: usize,
}

/// Core import logic - independent of worker vs sync execution
/// Can be called from worker thread or main thread
#[must_use]
pub fn import_geojson_to_updates(request: &GeoJsonImportRequest) -> GeoJsonImportResponse {
    use crate::models::{Node, Stations, TrackHandedness};
    use petgraph::visit::{EdgeRef, IntoEdgeReferences};

    // Parse GeoJSON
    let parsed = match GeoJsonImport::parse(&request.geojson_string) {
        Ok(p) => p,
        Err(e) => {
            return GeoJsonImportResponse {
                result: Err(format!("Failed to parse GeoJSON: {e}")),
                updates: vec![],
                stations_added: 0,
                edges_added: 0,
            };
        }
    };

    // Create temporary graph for import
    let mut temp_graph = RailwayGraph::new();

    // Perform import
    let import_result = match GeoJsonImport::import(
        &parsed,
        &request.config,
        ImportMode::CreateInfrastructure,
        &mut temp_graph,
        0,
        &[],
        TrackHandedness::RightHand,
    ) {
        Ok(r) => r,
        Err(e) => {
            return GeoJsonImportResponse {
                result: Err(e),
                updates: vec![],
                stations_added: 0,
                edges_added: 0,
            };
        }
    };

    // Extract GraphUpdate operations from result
    let mut updates = Vec::new();

    // Extract stations with positions
    for node_idx in temp_graph.graph.node_indices() {
        if let Some(Node::Station(station)) = temp_graph.graph.node_weight(node_idx) {
            let position = temp_graph.get_station_position(node_idx).unwrap_or((0.0, 0.0));

            let station_id = temp_graph
                .station_name_to_index
                .iter()
                .find(|(_, &idx)| idx == node_idx)
                .map_or_else(|| station.name.clone(), |(id, _)| id.clone());

            updates.push(GraphUpdate::AddStation {
                id: station_id,
                name: station.name.clone(),
                position,
            });
        }
    }

    // Extract edges
    for edge_ref in temp_graph.graph.edge_references() {
        let start_idx = edge_ref.source();
        let end_idx = edge_ref.target();
        let edge_weight = edge_ref.weight();

        let start_id = temp_graph
            .station_name_to_index
            .iter()
            .find(|(_, &idx)| idx == start_idx)
            .map(|(id, _)| id.clone())
            .unwrap_or_default();

        let end_id = temp_graph
            .station_name_to_index
            .iter()
            .find(|(_, &idx)| idx == end_idx)
            .map(|(id, _)| id.clone())
            .unwrap_or_default();

        if let Some(first_track) = edge_weight.tracks.first() {
            let bidirectional = matches!(
                first_track.direction,
                crate::models::TrackDirection::Bidirectional
            );

            updates.push(GraphUpdate::AddTrack {
                start_id: start_id.clone(),
                end_id: end_id.clone(),
                bidirectional,
            });

            for track in edge_weight.tracks.iter().skip(1) {
                let bidirectional = matches!(
                    track.direction,
                    crate::models::TrackDirection::Bidirectional
                );

                updates.push(GraphUpdate::AddParallelTrack {
                    start_id: start_id.clone(),
                    end_id: end_id.clone(),
                    bidirectional,
                });
            }
        }
    }

    GeoJsonImportResponse {
        result: Ok(()),
        updates,
        stations_added: import_result.stations_added,
        edges_added: import_result.edges_added,
    }
}
