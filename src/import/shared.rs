use crate::models::{RailwayGraph, Track, TrackDirection, Platform};
use petgraph::stable_graph::NodeIndex;

/// Create tracks based on count, using consistent direction assignment
///
/// Direction assignment logic:
/// - 1 track: Bidirectional
/// - 2 tracks: Forward, Backward
/// - 3 tracks: Forward, Bidirectional, Backward
/// - 4 tracks: Forward, Forward, Backward, Backward
/// - 5 tracks: Forward, Forward, Bidirectional, Backward, Backward
///
/// Pattern: outer tracks are directional, middle track(s) bidirectional for odd counts
#[must_use]
pub fn create_tracks_with_count(track_count: usize) -> Vec<Track> {
    (0..track_count)
        .map(|i| {
            let direction = if track_count == 1 {
                TrackDirection::Bidirectional
            } else if track_count % 2 == 1 && i == track_count / 2 {
                // Middle track in odd count is bidirectional
                TrackDirection::Bidirectional
            } else if i < track_count / 2 {
                // First half: Forward
                TrackDirection::Forward
            } else {
                // Second half: Backward
                TrackDirection::Backward
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
