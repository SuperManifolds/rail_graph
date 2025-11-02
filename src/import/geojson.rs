use crate::models::{Node, RailwayGraph, Stations, Track, TrackDirection, TrackHandedness, Tracks};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use super::{Import, ImportMode, ImportResult};

// Infrastructure view grid constants - match infrastructure_view.rs
const GRID_SIZE: f64 = 30.0;
const BASE_STATION_SPACING: f64 = 120.0;
const TARGET_SIZE: f64 = BASE_STATION_SPACING * 20.0; // Reasonable default map size
const MAX_STATION_DISTANCE: f64 = GRID_SIZE * 5.0; // 150px tolerance for matching track endpoints to stations
const SAMPLE_DISTANCE: f64 = 50.0; // Check for stations every 50px along the line

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
        leptos::logging::log!("🚀 GeoJSON import starting (v2 with detailed logging)");

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
        leptos::logging::log!("Extracting positions for transform...");
        let positions: Vec<(f64, f64)> = station_data.values().map(|(_, coords)| *coords).collect();
        leptos::logging::log!("Positions extracted: {}", positions.len());

        // Calculate transform
        leptos::logging::log!("Calculating coordinate transform...");
        let transform = if positions.is_empty() {
            CoordinateTransform::default()
        } else {
            calculate_coordinate_transform(&positions)
        };
        leptos::logging::log!("Transform calculated");

        // Build list of stations with their snapped positions for nearest-neighbor search
        leptos::logging::log!("Allocating station list...");
        let mut station_list: Vec<(petgraph::stable_graph::NodeIndex, (f64, f64))> =
            Vec::with_capacity(station_data.len());
        leptos::logging::log!("Station list allocated");

        // Pass 2: Add unique stations and build lookup list
        leptos::logging::log!("Adding stations to graph...");
        for (i, (id, (name, coords))) in station_data.iter().enumerate() {
            if i % 10 == 0 {
                leptos::logging::log!("  Adding station {}/{}", i, station_data.len());
            }

            // Use ID as internal unique key, name is for display only
            let idx = graph.add_or_get_station(id.clone());

            // Update the station name to show the display name
            if let Some(Node::Station(ref mut station)) = graph.graph.node_weight_mut(idx) {
                station.name.clone_from(name);
            }

            let normalized = transform.apply(*coords);

            // Snap to grid for clean alignment
            let grid_key = coord_to_grid(normalized);
            let snapped_position = grid_to_coord(grid_key);
            graph.set_station_position(idx, snapped_position);

            // Store in list for nearest-neighbor search
            station_list.push((idx, snapped_position));
            stations_added += 1;
        }
        leptos::logging::log!("Stations added to graph: {}", stations_added);

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

        Ok(ImportResult {
            lines: Vec::new(),
            stations_added,
            edges_added,
        })
    }
}

fn is_station_feature(feature: &Value) -> bool {
    feature["properties"]["preview_type"].as_str() == Some("station")
}

fn is_tracks_feature(feature: &Value) -> bool {
    feature["properties"]["preview_type"].as_str() == Some("tracks")
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

fn import_track_feature(
    feature: &Value,
    graph: &mut RailwayGraph,
    transform: &CoordinateTransform,
    station_list: &[(petgraph::stable_graph::NodeIndex, (f64, f64))],
) -> Result<usize, String> {
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

        for &coords in linestring {
            let transformed_coords = transform.apply(coords);

            // Check first point, or when we've traveled SAMPLE_DISTANCE
            let should_check = if let Some(last_pos) = last_checked_pos {
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
                true // First point - always check
            };

            last_checked_pos = Some(transformed_coords);

            if !should_check {
                continue;
            }

            // Check if this point is near any station
            if let Some(station_idx) = find_nearest_station(transformed_coords, station_list, MAX_STATION_DISTANCE) {
                // Only add if it's not the same as the last waypoint (avoid consecutive duplicates)
                if waypoints.is_empty() || waypoints.last() != Some(&station_idx) {
                    waypoints.push(station_idx);
                }
            }
        }

        // Need at least 2 waypoints to create edges
        if waypoints.len() < 2 {
            continue;
        }

        // Create edges between consecutive waypoints
        for i in 0..waypoints.len() - 1 {
            let start_idx = waypoints[i];
            let end_idx = waypoints[i + 1];

            if start_idx == end_idx {
                continue; // Skip self-loops
            }

            // Check if edge already exists
            if let Some(edge_idx) = graph.graph.find_edge(start_idx, end_idx) {
                // Edge exists - add another track to it (parallel track)
                if let Some(edge_weight) = graph.graph.edge_weight_mut(edge_idx) {
                    let track = Track {
                        direction: TrackDirection::Bidirectional,
                    };
                    edge_weight.tracks.push(track);
                }
            } else {
                // Create new edge with one track
                let track = Track {
                    direction: TrackDirection::Bidirectional,
                };
                graph.add_track(start_idx, end_idx, vec![track]);
                edges_added += 1;
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
#[derive(Serialize, Deserialize)]
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
