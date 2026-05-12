use crate::models::{RailwayGraph, Track, TrackDirection, Platform, TrackHandedness};
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use chrono::NaiveTime;

/// Create tracks based on count, using consistent direction assignment based on handedness
///
/// Direction assignment logic:
/// - 1 track: Bidirectional
/// - 2+ tracks with right-hand traffic: Left tracks Forward, right tracks Backward
/// - 2+ tracks with left-hand traffic: Left tracks Backward, right tracks Forward
/// - Odd counts: Middle track is always Bidirectional
///
/// Examples for right-hand traffic:
/// - 2 tracks: Forward, Backward
/// - 3 tracks: Forward, Bidirectional, Backward
/// - 4 tracks: Forward, Forward, Backward, Backward
///
/// Examples for left-hand traffic:
/// - 2 tracks: Backward, Forward
/// - 3 tracks: Backward, Bidirectional, Forward
/// - 4 tracks: Backward, Backward, Forward, Forward
#[must_use]
pub fn create_tracks_with_count(track_count: usize, handedness: TrackHandedness) -> Vec<Track> {
    (0..track_count)
        .map(|i| {
            let direction = if track_count == 1 {
                TrackDirection::Bidirectional
            } else if track_count % 2 == 1 && i == track_count / 2 {
                // Middle track in odd count is bidirectional
                TrackDirection::Bidirectional
            } else {
                match handedness {
                    TrackHandedness::RightHand => {
                        if i < track_count / 2 {
                            TrackDirection::Forward  // Left tracks go forward
                        } else {
                            TrackDirection::Backward  // Right tracks go backward
                        }
                    }
                    TrackHandedness::LeftHand => {
                        if i < track_count / 2 {
                            TrackDirection::Backward  // Left tracks go backward
                        } else {
                            TrackDirection::Forward  // Right tracks go forward
                        }
                    }
                }
            };
            Track { direction }
        })
        .collect()
}

/// Ensure station has at least N platforms (numbered 1, 2, 3, ..., N)
/// Returns the index of the requested platform (0-indexed)
pub fn ensure_platforms_up_to(graph: &mut RailwayGraph, station_idx: NodeIndex, platform_number: usize) -> usize {
    if platform_number == 0 {
        return 0; // Invalid platform number, default to 0
    }

    let Some(station_node) = graph.graph.node_weight_mut(station_idx)
        .and_then(|node| node.as_station_mut()) else {
        return 0;
    };

    // Ensure we have at least platform_number platforms
    while station_node.platforms.len() < platform_number {
        let next_number = station_node.platforms.len() + 1;
        station_node.platforms.push(Platform {
            name: next_number.to_string(),
        });
    }

    // Return 0-indexed position (platform_number - 1)
    platform_number - 1
}

/// Get or add platform to station by name and return its index
/// If platform is a number, ensures all platforms up to that number exist
pub fn get_or_add_platform(graph: &mut RailwayGraph, station_idx: NodeIndex, platform_name: &str) -> usize {
    // Try to parse as a number
    if let Ok(platform_num) = platform_name.parse::<usize>() {
        return ensure_platforms_up_to(graph, station_idx, platform_num);
    }

    // Not a number, handle as named platform
    if let Some(station_node) = graph.graph.node_weight_mut(station_idx)
        .and_then(|node| node.as_station_mut()) {
        // Check if platform exists
        if let Some(idx) = station_node.platforms.iter().position(|p| p.name == platform_name) {
            return idx;
        }

        // Add new platform
        station_node.platforms.push(Platform {
            name: platform_name.to_string(),
        });
        station_node.platforms.len() - 1
    } else {
        0
    }
}

/// Calculate duration in seconds between two times, handling midnight wraparound
/// If arrival time < departure time, assumes midnight crossing
/// Returns duration in seconds
#[must_use]
pub fn calculate_duration_with_wraparound(from_seconds: i64, to_seconds: i64) -> i64 {
    if to_seconds >= from_seconds {
        to_seconds - from_seconds
    } else {
        // Crossed midnight
        86400 - from_seconds + to_seconds
    }
}

/// Ensure an edge has enough tracks for the given track number (1-indexed)
/// If `track_number` is Some(N), ensures at least N tracks exist
/// Recreates tracks using `create_tracks_with_count` if expansion is needed
pub fn ensure_track_count(graph: &mut RailwayGraph, edge_idx: EdgeIndex, track_number: Option<usize>, handedness: TrackHandedness) {
    let Some(track_num) = track_number else { return };
    if track_num == 0 { return; } // Invalid track number

    let required_track_count = track_num; // Track numbers are 1-indexed

    let Some(track_segment) = graph.graph.edge_weight_mut(edge_idx) else { return };
    if track_segment.tracks.len() < required_track_count {
        // Need to add more tracks - recreate with the new count
        track_segment.tracks = create_tracks_with_count(required_track_count, handedness);
    }
}

/// Parse time string in HH:MM or HH:MM:SS format to `NaiveTime`
/// Returns None if the string is empty or cannot be parsed
#[must_use]
pub fn parse_time(time_str: &str) -> Option<NaiveTime> {
    if time_str.is_empty() {
        return None;
    }

    let parts: Vec<&str> = time_str.split(':').collect();

    match parts.len() {
        2 => {
            // HH:MM format
            let hour = parts[0].parse::<u32>().ok()?;
            let minute = parts[1].parse::<u32>().ok()?;
            NaiveTime::from_hms_opt(hour, minute, 0)
        }
        3 => {
            // HH:MM:SS format
            let hour = parts[0].parse::<u32>().ok()?;
            let minute = parts[1].parse::<u32>().ok()?;
            let second = parts[2].parse::<u32>().ok()?;
            NaiveTime::from_hms_opt(hour, minute, second)
        }
        _ => None,
    }
}

/// Find platform index by name in a platform list
/// Returns None if platform name is empty or not found
#[must_use]
pub fn find_platform_by_name(platforms: &[Platform], platform_name: &str) -> Option<usize> {
    if platform_name.is_empty() {
        return None;
    }

    platforms.iter().position(|p| p.name == platform_name)
}

/// Find a junction with the given name connected to a station
/// Returns the `NodeIndex` of the junction if found
#[must_use]
pub fn find_connected_junction(
    graph: &RailwayGraph,
    station_idx: NodeIndex,
    junction_name: &str,
) -> Option<NodeIndex> {
    use crate::models::Junctions;
    use petgraph::visit::EdgeRef;
    use petgraph::Direction;

    graph.graph.edges(station_idx)
        .chain(graph.graph.edges_directed(station_idx, Direction::Incoming))
        .find_map(|edge| {
            let neighbor = if edge.source() == station_idx {
                edge.target()
            } else {
                edge.source()
            };

            graph.get_junction(neighbor)
                .and_then(|j| j.name.as_ref())
                .filter(|name| *name == junction_name)
                .map(|_| neighbor)
        })
}
