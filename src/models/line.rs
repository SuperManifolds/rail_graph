use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use crate::constants::{BASE_DATE, BASE_MIDNIGHT};
use petgraph::stable_graph::NodeIndex;
use super::{RailwayGraph, TrackSegment, TrackDirection, Tracks, DaysOfWeek, RouteDirection, TrackHandedness, Stations, Routes, StationPosition};

#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
pub fn generate_random_color(seed: usize) -> String {
    // Use a simple hash-based color generator for deterministic but varied colors
    let hue = f64::from(((seed * 137) % 360) as i32);
    let saturation = 65.0 + f64::from(((seed * 97) % 20) as i32); // 65-85%
    let lightness = 55.0 + f64::from(((seed * 53) % 15) as i32);  // 55-70%

    // Convert HSL to RGB
    let chroma = (1.0 - (2.0 * lightness / 100.0 - 1.0).abs()) * saturation / 100.0;
    let second_component = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let lightness_match = lightness / 100.0 - chroma / 2.0;

    let (red, green, blue) = match hue as u32 {
        0..=59 => (chroma, second_component, 0.0),
        60..=119 => (second_component, chroma, 0.0),
        120..=179 => (0.0, chroma, second_component),
        180..=239 => (0.0, second_component, chroma),
        240..=299 => (second_component, 0.0, chroma),
        _ => (chroma, 0.0, second_component),
    };

    format!("#{:02X}{:02X}{:02X}",
        ((red + lightness_match) * 255.0) as u8,
        ((green + lightness_match) * 255.0) as u8,
        ((blue + lightness_match) * 255.0) as u8
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteSegment {
    pub edge_index: usize,
    #[serde(default)]
    pub track_index: usize,
    #[serde(default)]
    pub origin_platform: usize,
    #[serde(default)]
    pub destination_platform: usize,
    #[serde(with = "option_duration_serde", default)]
    pub duration: Option<Duration>,
    #[serde(with = "duration_serde", default = "default_wait_time")]
    pub wait_time: Duration,
}

fn default_wait_time() -> Duration {
    Duration::seconds(30)
}

fn default_first_stop_wait_time() -> Duration {
    Duration::zero()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[derive(Default)]
pub enum ScheduleMode {
    #[default]
    Auto,
    Manual,
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManualDeparture {
    #[serde(default = "uuid::Uuid::new_v4")]
    pub id: uuid::Uuid,
    #[serde(with = "naive_datetime_serde")]
    pub time: NaiveDateTime,
    #[serde(with = "node_index_serde")]
    pub from_station: NodeIndex,
    #[serde(with = "node_index_serde")]
    pub to_station: NodeIndex,
    #[serde(default)]
    pub days_of_week: DaysOfWeek,
    #[serde(default)]
    pub train_number: Option<String>,
    #[serde(with = "option_duration_serde", default)]
    pub repeat_interval: Option<Duration>,
    #[serde(with = "option_naive_datetime_serde", default)]
    pub repeat_until: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    #[serde(default = "uuid::Uuid::new_v4")]
    pub id: uuid::Uuid,
    pub name: String,
    #[serde(with = "duration_serde")]
    pub frequency: Duration,
    pub color: String,
    #[serde(default = "default_thickness")]
    pub thickness: f64,
    #[serde(with = "naive_datetime_serde")]
    pub first_departure: NaiveDateTime,
    #[serde(with = "naive_datetime_serde")]
    pub return_first_departure: NaiveDateTime,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default)]
    pub schedule_mode: ScheduleMode,
    #[serde(default)]
    pub days_of_week: DaysOfWeek,
    #[serde(default)]
    pub manual_departures: Vec<ManualDeparture>,
    #[serde(default)]
    pub forward_route: Vec<RouteSegment>,
    #[serde(default)]
    pub return_route: Vec<RouteSegment>,
    #[serde(default = "default_sync_routes")]
    pub sync_routes: bool,
    #[serde(default = "default_train_number_format")]
    pub auto_train_number_format: String,
    #[serde(with = "naive_datetime_serde")]
    pub last_departure: NaiveDateTime,
    #[serde(with = "naive_datetime_serde", default = "default_return_last_departure")]
    pub return_last_departure: NaiveDateTime,
    #[serde(with = "duration_serde", default = "default_wait_time")]
    pub default_wait_time: Duration,
    #[serde(with = "duration_serde", default = "default_first_stop_wait_time")]
    pub first_stop_wait_time: Duration,
    #[serde(with = "duration_serde", default = "default_first_stop_wait_time")]
    pub return_first_stop_wait_time: Duration,
    #[serde(default)]
    pub sort_index: Option<f64>,
    #[serde(default = "default_sync_departure_offsets")]
    pub sync_departure_offsets: bool,
    #[serde(default)]
    pub folder_id: Option<uuid::Uuid>,
}

fn default_visible() -> bool {
    true
}

fn default_thickness() -> f64 {
    2.0
}

fn default_sync_routes() -> bool {
    true
}

fn default_sync_departure_offsets() -> bool {
    false
}

fn default_train_number_format() -> String {
    "{line} {seq:04}".to_string()
}

fn default_return_last_departure() -> NaiveDateTime {
    BASE_DATE.and_hms_opt(22, 0, 0).unwrap_or(BASE_MIDNIGHT)
}

impl RouteSegment {
    /// Validate that a route segment with no duration is valid
    /// Segments without duration are only valid for passing stations (must have zero wait time)
    #[must_use]
    pub fn is_valid_for_passing_station(&self) -> bool {
        self.duration.is_none() && self.wait_time == Duration::zero()
    }
}

impl Line {
    /// Create lines from names with default settings
    /// `color_offset` is added to the color seed to avoid duplicate colors when adding lines to existing project
    #[must_use]
    pub fn create_from_ids(line_names: &[String], color_offset: usize) -> Vec<Line> {
        line_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let offset_minutes = u32::try_from(i).unwrap_or(0).saturating_mul(15);
                Line {
                    id: uuid::Uuid::new_v4(),
                    name: name.clone(),
                    frequency: Duration::hours(1), // Default, configurable by user
                    color: generate_random_color(i + color_offset),
                    thickness: 2.0,
                    first_departure: BASE_DATE.and_hms_opt(5, offset_minutes, 0).unwrap_or(BASE_MIDNIGHT),
                    return_first_departure: BASE_DATE.and_hms_opt(6, offset_minutes, 0).unwrap_or(BASE_MIDNIGHT),
                    visible: true,
                    schedule_mode: ScheduleMode::Auto,
                    days_of_week: DaysOfWeek::ALL_DAYS,
                    manual_departures: Vec::new(),
                    forward_route: Vec::new(),
                    return_route: Vec::new(),
                    sync_routes: true,
                    auto_train_number_format: default_train_number_format(),
                    last_departure: BASE_DATE.and_hms_opt(22, 0, 0).unwrap_or(BASE_MIDNIGHT),
                    return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).unwrap_or(BASE_MIDNIGHT),
                    default_wait_time: default_wait_time(),
                    first_stop_wait_time: default_first_stop_wait_time(),
                    return_first_stop_wait_time: default_first_stop_wait_time(),
                    sort_index: None,
                    sync_departure_offsets: false,
                    folder_id: None,
                }
            })
            .collect()
    }

    /// Update route after station deletion with bypass edges
    /// `removed_edges`: edges that were removed
    /// `bypass_mapping`: maps (`old_edge1`, `old_edge2`) -> `new_bypass_edge`
    pub fn update_route_after_deletion(
        &mut self,
        removed_edges: &[usize],
        bypass_mapping: &std::collections::HashMap<(usize, usize), usize>,
    ) {
        self.forward_route = Self::update_single_route(&self.forward_route, removed_edges, bypass_mapping);
        self.return_route = Self::update_single_route(&self.return_route, removed_edges, bypass_mapping);
    }

    fn update_single_route(
        route: &[RouteSegment],
        removed_edges: &[usize],
        bypass_mapping: &std::collections::HashMap<(usize, usize), usize>,
    ) -> Vec<RouteSegment> {
        let mut new_route = Vec::new();
        let mut i = 0;

        while i < route.len() {
            let segment = &route[i];

            // Keep segments that don't use removed edges
            if !removed_edges.contains(&segment.edge_index) {
                new_route.push(segment.clone());
                i += 1;
                continue;
            }

            // Segment uses a removed edge - try to create a bypass
            let Some(next_segment) = route.get(i + 1) else {
                i += 1;
                continue;
            };

            // Check if we have a bypass edge for this pair
            let Some(&bypass_edge_idx) = bypass_mapping.get(&(segment.edge_index, next_segment.edge_index)) else {
                i += 1;
                continue;
            };

            // Combine durations (travel time + wait time at deleted station + next travel time)
            let combined_duration = segment.duration.and_then(|d1| next_segment.duration.map(|d2| d1 + segment.wait_time + d2));

            // Preserve platforms from the original segments
            new_route.push(RouteSegment {
                edge_index: bypass_edge_idx,
                track_index: 0,
                origin_platform: segment.origin_platform,
                destination_platform: next_segment.destination_platform,
                duration: combined_duration,
                wait_time: next_segment.wait_time,
            });

            i += 2; // Skip both segments
        }

        new_route
    }

    /// Fix track indices after track changes on an edge
    /// Reassigns tracks that are out of bounds or have incompatible directions
    pub fn fix_track_indices_after_change(&mut self, edge_index: usize, new_track_count: usize, graph: &RailwayGraph) {
        use petgraph::stable_graph::EdgeIndex;

        let max_track_index = new_track_count.saturating_sub(1);
        let edge_idx = EdgeIndex::new(edge_index);

        // Get track segment to check directions
        let track_segment = graph.get_track(edge_idx);

        // Fix forward route
        for segment in &mut self.forward_route {
            if segment.edge_index != edge_index {
                continue;
            }

            // Check if track index is out of bounds
            if segment.track_index > max_track_index {
                segment.track_index = Self::find_compatible_track(track_segment, true, max_track_index);
                continue;
            }

            // Check if track direction is compatible with forward route
            if Self::is_track_incompatible(track_segment, segment.track_index, true) {
                segment.track_index = Self::find_compatible_track(track_segment, true, max_track_index);
            }
        }

        // Fix return route
        for segment in &mut self.return_route {
            if segment.edge_index != edge_index {
                continue;
            }

            // Check if track index is out of bounds
            if segment.track_index > max_track_index {
                segment.track_index = Self::find_compatible_track(track_segment, false, max_track_index);
                continue;
            }

            // Check if track direction is compatible with return route
            if Self::is_track_incompatible(track_segment, segment.track_index, false) {
                segment.track_index = Self::find_compatible_track(track_segment, false, max_track_index);
            }
        }
    }

    /// Validate and fix all track indices in both forward and return routes
    /// Only corrects segments where the track index is out of bounds or incompatible with route direction
    /// Returns the number of segments that were corrected
    pub fn validate_and_fix_track_indices(&mut self, graph: &RailwayGraph) -> usize {
        use petgraph::stable_graph::EdgeIndex;

        let mut fixed_count = 0;

        eprintln!("\n=== Fixing tracks for line '{}' ===", self.name);

        // Fix forward route - track current node as we traverse
        // First pass: determine initial direction by looking at first two segments
        let mut current_node: Option<petgraph::stable_graph::NodeIndex> = None;
        if let (Some(first), Some(second)) = (self.forward_route.first(), self.forward_route.get(1)) {
            let first_edge = EdgeIndex::new(first.edge_index);
            let second_edge = EdgeIndex::new(second.edge_index);
            if let (Some((source, target)), Some((next_source, next_target))) =
                (graph.graph.edge_endpoints(first_edge), graph.graph.edge_endpoints(second_edge)) {
                // If target connects to next edge, we start at source
                if target == next_source || target == next_target {
                    current_node = Some(source);
                } else {
                    current_node = Some(target);
                }
            } else {
                current_node = Some(graph.graph.edge_endpoints(first_edge).map(|(s, _)| s)).flatten();
            }
        }

        for segment in &mut self.forward_route {
            let edge_idx = EdgeIndex::new(segment.edge_index);

            // Get edge endpoints to determine travel direction
            let Some((source, target)) = graph.graph.edge_endpoints(edge_idx) else {
                continue;
            };

            // If we still don't have current_node, default to source
            if current_node.is_none() {
                current_node = Some(source);
            }

            let is_forward = current_node == Some(source);
            let next_node = if is_forward { target } else { source };

            eprintln!("Forward route edge {}: track {}, traveling {} on edge",
                segment.edge_index,
                segment.track_index,
                if is_forward { "forward" } else { "backward" }
            );

            // Check if current track is incompatible with actual travel direction
            let track_segment = graph.get_track(edge_idx);
            if Self::is_track_incompatible(track_segment, segment.track_index, is_forward) {
                let correct_track = graph.select_track_for_direction(edge_idx, !is_forward);
                eprintln!("  -> Fixing to track {correct_track}");
                segment.track_index = correct_track;
                fixed_count += 1;
            }

            current_node = Some(next_node);
        }

        // Fix return route - track current node as we traverse
        // First pass: determine initial direction by looking at first two segments
        let mut current_node: Option<petgraph::stable_graph::NodeIndex> = None;
        if let (Some(first), Some(second)) = (self.return_route.first(), self.return_route.get(1)) {
            let first_edge = EdgeIndex::new(first.edge_index);
            let second_edge = EdgeIndex::new(second.edge_index);
            if let (Some((source, target)), Some((next_source, next_target))) =
                (graph.graph.edge_endpoints(first_edge), graph.graph.edge_endpoints(second_edge)) {
                // If target connects to next edge, we start at source
                if target == next_source || target == next_target {
                    current_node = Some(source);
                } else {
                    current_node = Some(target);
                }
            } else {
                current_node = Some(graph.graph.edge_endpoints(first_edge).map(|(s, _)| s)).flatten();
            }
        }

        for segment in &mut self.return_route {
            let edge_idx = EdgeIndex::new(segment.edge_index);

            // Get edge endpoints to determine travel direction
            let Some((source, target)) = graph.graph.edge_endpoints(edge_idx) else {
                continue;
            };

            // If we still don't have current_node, default to source
            if current_node.is_none() {
                current_node = Some(source);
            }

            let is_forward = current_node == Some(source);
            let next_node = if is_forward { target } else { source };

            eprintln!("Return route edge {}: track {}, traveling {} on edge",
                segment.edge_index,
                segment.track_index,
                if is_forward { "forward" } else { "backward" }
            );

            // Check if current track is incompatible with actual travel direction
            let track_segment = graph.get_track(edge_idx);
            if Self::is_track_incompatible(track_segment, segment.track_index, is_forward) {
                let correct_track = graph.select_track_for_direction(edge_idx, !is_forward);
                eprintln!("  -> Fixing to track {correct_track}");
                segment.track_index = correct_track;
                fixed_count += 1;
            }

            current_node = Some(next_node);
        }

        fixed_count
    }

    /// Check if a track is incompatible with the route direction
    /// Returns true if track doesn't exist or has incompatible direction
    fn is_track_incompatible(track_segment: Option<&TrackSegment>, track_index: usize, is_forward: bool) -> bool {
        let Some(ts) = track_segment else {
            return false;
        };

        let Some(track) = ts.tracks.get(track_index) else {
            // Track index is out of bounds - definitely incompatible
            return true;
        };

        if is_forward {
            !matches!(track.direction, TrackDirection::Forward | TrackDirection::Bidirectional)
        } else {
            !matches!(track.direction, TrackDirection::Backward | TrackDirection::Bidirectional)
        }
    }

    /// Find a compatible track for a given route direction
    #[must_use]
    pub fn find_compatible_track(track_segment: Option<&TrackSegment>, is_forward: bool, max_index: usize) -> usize {
        let Some(ts) = track_segment else {
            return 0;
        };

        // Find first track compatible with the route direction
        for (i, track) in ts.tracks.iter().enumerate().take(max_index + 1) {
            let compatible = if is_forward {
                matches!(track.direction, TrackDirection::Forward | TrackDirection::Bidirectional)
            } else {
                matches!(track.direction, TrackDirection::Backward | TrackDirection::Bidirectional)
            };

            if compatible {
                return i;
            }
        }

        // Fallback to track 0 if no compatible track found
        0
    }

    /// Check if this line uses a specific edge in either route
    #[must_use]
    pub fn uses_edge(&self, edge_index: usize) -> bool {
        self.forward_route.iter().any(|segment| segment.edge_index == edge_index) ||
        self.return_route.iter().any(|segment| segment.edge_index == edge_index)
    }

    /// Extract the ordered list of all nodes (stations and junctions) from this line's forward route
    #[must_use]
    pub fn get_station_path(&self, graph: &RailwayGraph) -> Vec<NodeIndex> {
        use std::collections::HashSet;

        let mut path = Vec::new();
        let mut seen = HashSet::new();

        // Helper to add any node if not seen
        let try_add_node = |node_idx: NodeIndex, seen: &mut HashSet<NodeIndex>, path: &mut Vec<NodeIndex>| {
            // Check if the node exists and we haven't seen it yet
            if graph.graph.node_weight(node_idx).is_some() && !seen.contains(&node_idx) {
                path.push(node_idx);
                seen.insert(node_idx);
            }
        };

        // Get all nodes from forward route edges
        for segment in &self.forward_route {
            if let Some((from, to)) = graph.graph.edge_endpoints(petgraph::graph::EdgeIndex::new(segment.edge_index)) {
                try_add_node(from, &mut seen, &mut path);
                try_add_node(to, &mut seen, &mut path);
            }
        }

        path
    }

    /// Check if this line uses any of the given edges in either route
    #[must_use]
    pub fn uses_any_edge(&self, edge_indices: &[usize]) -> bool {
        self.forward_route.iter().any(|segment| edge_indices.contains(&segment.edge_index)) ||
        self.return_route.iter().any(|segment| edge_indices.contains(&segment.edge_index))
    }

    /// Replace an edge that was split by a junction with two new edges
    /// This is used when inserting a junction in the middle of an existing edge
    pub fn replace_split_edge(&mut self, old_edge: usize, new_edge1: usize, new_edge2: usize, track_count: usize, graph: &RailwayGraph, middle_node_platform_count: usize, handedness: TrackHandedness) {
        Self::replace_split_edge_in_route(&mut self.forward_route, old_edge, new_edge1, new_edge2, track_count, graph, middle_node_platform_count, handedness);
        Self::replace_split_edge_in_route(&mut self.return_route, old_edge, new_edge2, new_edge1, track_count, graph, middle_node_platform_count, handedness);
    }

    fn replace_split_edge_in_route(route: &mut Vec<RouteSegment>, old_edge: usize, first_edge: usize, second_edge: usize, track_count: usize, graph: &RailwayGraph, middle_node_platform_count: usize, handedness: TrackHandedness) {
        use petgraph::prelude::*;

        let mut new_route = Vec::new();

        for segment in route.iter() {
            if segment.edge_index == old_edge {
                // Calculate platform for middle station using direction-based logic
                // The train arrives at middle station via first_edge and departs via second_edge
                let first_edge_idx = EdgeIndex::new(first_edge);
                let second_edge_idx = EdgeIndex::new(second_edge);

                let middle_platform_arriving = graph.get_default_platform_for_arrival(
                    first_edge_idx,
                    true,  // arriving at target of first_edge (the middle station)
                    middle_node_platform_count,
                    handedness
                );

                let middle_platform_departing = graph.get_default_platform_for_arrival(
                    second_edge_idx,
                    false,  // departing from source of second_edge (the middle station)
                    middle_node_platform_count,
                    handedness
                );

                // Split this segment into two through the junction
                new_route.push(RouteSegment {
                    edge_index: first_edge,
                    track_index: segment.track_index.min(track_count.saturating_sub(1)),
                    origin_platform: segment.origin_platform,
                    destination_platform: middle_platform_arriving,
                    duration: segment.duration.map(|d| d / 2),
                    wait_time: segment.wait_time,
                });
                new_route.push(RouteSegment {
                    edge_index: second_edge,
                    track_index: segment.track_index.min(track_count.saturating_sub(1)),
                    origin_platform: middle_platform_departing,
                    destination_platform: segment.destination_platform,
                    duration: segment.duration.map(|d| d / 2),
                    wait_time: Duration::zero(),
                });
            } else {
                new_route.push(segment.clone());
            }
        }

        *route = new_route;
    }

    /// Attempt to reroute segments that use a deleted edge
    /// `deleted_edge`: The edge index that was deleted
    /// `from_node`: The source node of the deleted edge
    /// `to_node`: The target node of the deleted edge
    /// Returns true if any rerouting was performed
    pub fn reroute_deleted_edge(&mut self, deleted_edge: usize, from_node: NodeIndex, to_node: NodeIndex, graph: &RailwayGraph) -> bool {
        let mut changed = false;

        // Check forward route
        changed |= Self::reroute_single_direction(&mut self.forward_route, deleted_edge, from_node, to_node, graph);

        // Check return route
        changed |= Self::reroute_single_direction(&mut self.return_route, deleted_edge, from_node, to_node, graph);

        changed
    }

    /// Get the effective display durations for return route when sync is enabled
    /// Returns the durations that will actually be used during journey generation,
    /// mirroring the forward route's inheritance pattern
    #[must_use]
    pub fn get_return_display_durations(&self) -> Vec<Option<chrono::Duration>> {
        if !self.sync_routes || self.return_route.is_empty() {
            return self.return_route.iter().map(|seg| seg.duration).collect();
        }

        let mut return_durations = vec![None; self.return_route.len()];

        // Walk forward route to find segments with durations and their spans
        let mut i = 0;
        while i < self.forward_route.len() {
            if let Some(duration) = self.forward_route[i].duration {
                // Count how many segments this duration covers in forward route
                let mut span_len = 1;
                let mut j = i + 1;
                while j < self.forward_route.len() && self.forward_route[j].duration.is_none() {
                    span_len += 1;
                    j += 1;
                }

                // Mirror this span to return route
                let return_start = self.return_route.len().saturating_sub(i + span_len);
                if return_start < return_durations.len() {
                    return_durations[return_start] = Some(duration);
                }

                i += span_len;
            } else {
                i += 1;
            }
        }

        return_durations
    }

    /// Sync return route from forward route if `sync_routes` is enabled
    /// Preserves user-configured track indices and platform assignments
    /// from existing return segments while syncing the route structure and wait times.
    /// Clears all return route durations - they will be calculated from forward route during journey generation
    pub fn apply_route_sync_if_enabled(&mut self) {
        use std::collections::HashMap;

        if !self.sync_routes {
            return;
        }

        // Build a map of edge_index -> (track_index, origin_platform, destination_platform, wait_time)
        // This preserves user-configured tracks, platforms, and wait times from the existing return route
        let existing_settings: HashMap<usize, (usize, usize, usize, Duration)> = self.return_route
            .iter()
            .map(|seg| (
                seg.edge_index,
                (seg.track_index, seg.origin_platform, seg.destination_platform, seg.wait_time)
            ))
            .collect();

        // Create new return route by reversing forward route
        let mut new_return_route = Vec::new();

        for (i, forward_seg) in self.forward_route.iter().rev().enumerate() {
            // If we have existing settings for this edge in return route, preserve tracks/platforms/wait_time
            if let Some((track_index, origin_platform, destination_platform, wait_time)) =
                existing_settings.get(&forward_seg.edge_index) {
                // Preserve user-configured tracks, platforms, and wait time, clear duration
                new_return_route.push(RouteSegment {
                    edge_index: forward_seg.edge_index,
                    track_index: *track_index,
                    origin_platform: *origin_platform,
                    destination_platform: *destination_platform,
                    duration: None,
                    wait_time: *wait_time,
                });
            } else {
                // This is a new edge not in the return route, use defaults from forward route
                // but swap platforms for the reverse direction and clear duration
                // For wait time: need to shift when reversing because they represent wait at destination
                // For return_route[i], we need the wait time from the previous stop in forward direction
                let wait_time = if i < self.forward_route.len() - 1 {
                    // Get wait time from forward_route[len - i - 2] (the next segment in forward direction)
                    self.forward_route[self.forward_route.len() - i - 2].wait_time
                } else {
                    // Last segment in return route corresponds to first stop
                    self.first_stop_wait_time
                };

                new_return_route.push(RouteSegment {
                    edge_index: forward_seg.edge_index,
                    track_index: forward_seg.track_index,
                    origin_platform: forward_seg.destination_platform,
                    destination_platform: forward_seg.origin_platform,
                    duration: None,
                    wait_time,
                });
            }
        }

        self.return_route = new_return_route;
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn reroute_single_direction(
        route: &mut Vec<RouteSegment>,
        deleted_edge: usize,
        from_node: NodeIndex,
        to_node: NodeIndex,
        graph: &RailwayGraph,
    ) -> bool {
        use super::Routes;

        // Find all segments using the deleted edge
        let positions: Vec<usize> = route.iter()
            .enumerate()
            .filter(|(_, seg)| seg.edge_index == deleted_edge)
            .map(|(i, _)| i)
            .collect();

        if positions.is_empty() {
            return false;
        }

        let mut changed = false;

        for &pos in positions.iter().rev() {
            let segment = &route[pos];

            // Try to find alternative path between the endpoints
            let Some(path) = graph.find_path_between_nodes(from_node, to_node) else {
                continue;
            };

            // Create new segments for the path
            let mut new_segments = Vec::new();
            for (i, &path_edge) in path.iter().enumerate() {
                let new_segment = RouteSegment {
                    edge_index: path_edge.index(),
                    track_index: segment.track_index.min(
                        graph.graph.edge_weight(path_edge)
                            .map_or(0, |seg| seg.tracks.len().saturating_sub(1))
                    ),
                    origin_platform: if i == 0 { segment.origin_platform } else { 0 },
                    destination_platform: if i == path.len() - 1 { segment.destination_platform } else { 0 },
                    duration: segment.duration.map(|d| d / path.len().max(1) as i32),
                    wait_time: if i == 0 { segment.wait_time } else { Duration::zero() },
                };
                new_segments.push(new_segment);
            }

            // Replace the deleted segment with the new path
            route.splice(pos..=pos, new_segments);
            changed = true;
        }

        changed
    }

    /// Generate a name for a duplicated line
    /// If the name ends with (N), increments N. Otherwise appends (1).
    #[must_use]
    pub fn generate_duplicate_name(original_name: &str) -> String {
        // Check if the name ends with " (N)" pattern
        if let Some(rest) = original_name.strip_suffix(')') {
            if let Some(open_paren_pos) = rest.rfind(" (") {
                let number_str = &rest[open_paren_pos + 2..];
                if let Ok(num) = number_str.parse::<i32>() {
                    let base = &rest[..open_paren_pos];
                    return format!("{base} ({})", num + 1);
                }
            }
        }
        format!("{original_name} (1)")
    }

    /// Create a duplicate of this line with a new ID and updated name
    #[must_use]
    pub fn duplicate(&self) -> Self {
        let mut duplicated = self.clone();
        duplicated.id = uuid::Uuid::new_v4();
        duplicated.name = Self::generate_duplicate_name(&self.name);
        duplicated
    }

    /// Creates a route between two stations and adds it to the specified direction.
    ///
    /// # Arguments
    /// * `first_name` - Name of the starting station
    /// * `second_name` - Name of the ending station
    /// * `graph` - Railway graph to use for pathfinding
    /// * `direction` - Which route to populate (Forward or Return)
    /// * `handedness` - Track handedness for platform selection
    ///
    /// # Returns
    /// `true` if the route was created successfully, `false` if no path exists
    pub fn create_route_between_stations(
        &mut self,
        first_name: &str,
        second_name: &str,
        graph: &RailwayGraph,
        direction: RouteDirection,
        handedness: TrackHandedness,
    ) -> bool {
        let Some(first_idx) = graph.get_station_index(first_name) else {
            return false;
        };
        let Some(second_idx) = graph.get_station_index(second_name) else {
            return false;
        };
        let Some(path) = graph.find_path_between_nodes(first_idx, second_idx) else {
            return false;
        };

        for edge in &path {
            let Some((source, target)) = graph.graph.edge_endpoints(*edge) else {
                continue;
            };

            let is_passing_loop = graph.graph.node_weight(source)
                .and_then(|node| node.as_station())
                .is_some_and(|s| s.passing_loop);
            let default_wait = if is_passing_loop {
                Duration::seconds(0)
            } else {
                self.default_wait_time
            };

            let source_platform_count = graph.graph.node_weight(source)
                .and_then(|n| n.as_station())
                .map_or(1, |s| s.platforms.len());

            let target_platform_count = graph.graph.node_weight(target)
                .and_then(|n| n.as_station())
                .map_or(1, |s| s.platforms.len());

            let origin_platform = graph.get_default_platform_for_arrival(*edge, false, source_platform_count, handedness);
            let destination_platform = graph.get_default_platform_for_arrival(*edge, true, target_platform_count, handedness);

            // Select track compatible with route direction
            let traveling_backward = matches!(direction, RouteDirection::Return);
            let track_index = graph.select_track_for_direction(*edge, traveling_backward);

            let segment = RouteSegment {
                edge_index: edge.index(),
                track_index,
                origin_platform,
                destination_platform,
                duration: None,
                wait_time: default_wait,
            };

            match direction {
                RouteDirection::Forward => {
                    self.forward_route.push(segment);
                }
                RouteDirection::Return => {
                    self.return_route.push(segment);
                }
            }
        }

        if matches!(direction, RouteDirection::Forward) {
            self.apply_route_sync_if_enabled();
        }

        true
    }

    /// Adds a station to a route at the specified position (start or end).
    ///
    /// # Arguments
    /// * `station_idx` - `NodeIndex` of the station to add
    /// * `graph` - Railway graph to use for pathfinding
    /// * `direction` - Which route to modify (Forward or Return)
    /// * `position` - Whether to add at start or end of route
    /// * `handedness` - Track handedness for platform selection
    ///
    /// # Returns
    /// `true` if the station was added successfully, `false` if no path exists
    pub fn add_station_to_route(
        &mut self,
        station_idx: NodeIndex,
        graph: &RailwayGraph,
        direction: RouteDirection,
        position: StationPosition,
        handedness: TrackHandedness,
    ) -> bool {
        // Get the current route
        let current_route = match direction {
            RouteDirection::Forward => &self.forward_route,
            RouteDirection::Return => &self.return_route,
        };

        // Handle empty route case - just return true (no segments to add yet)
        if current_route.is_empty() {
            return true;
        }

        // Get the existing endpoint based on position
        let existing_idx = match position {
            StationPosition::Start => {
                // Get the first node in the route
                let Some(first_edge) = current_route.first().map(|seg| seg.edge_index) else {
                    return false;
                };
                let first_edge_idx = petgraph::stable_graph::EdgeIndex::new(first_edge);
                let Some((source, target)) = graph.graph.edge_endpoints(first_edge_idx) else {
                    return false;
                };

                // Determine which endpoint is the start
                if current_route.len() > 1 {
                    let second_edge = current_route[1].edge_index;
                    let second_edge_idx = petgraph::stable_graph::EdgeIndex::new(second_edge);
                    let Some((second_source, second_target)) = graph.graph.edge_endpoints(second_edge_idx) else {
                        return false;
                    };

                    // If target connects to second edge, we started at source
                    if target == second_source || target == second_target {
                        source
                    } else {
                        target
                    }
                } else {
                    source
                }
            }
            StationPosition::End => {
                // Get the last node in the route
                let Some(last_edge) = current_route.last().map(|seg| seg.edge_index) else {
                    return false;
                };
                let last_edge_idx = petgraph::stable_graph::EdgeIndex::new(last_edge);
                let Some((source, target)) = graph.graph.edge_endpoints(last_edge_idx) else {
                    return false;
                };

                // Determine which endpoint is the end
                if current_route.len() > 1 {
                    let second_last_edge = current_route[current_route.len() - 2].edge_index;
                    let second_last_idx = petgraph::stable_graph::EdgeIndex::new(second_last_edge);
                    let Some((second_source, second_target)) = graph.graph.edge_endpoints(second_last_idx) else {
                        return false;
                    };

                    // If source connects to second-last edge, we ended at target
                    if source == second_source || source == second_target {
                        target
                    } else {
                        source
                    }
                } else {
                    target
                }
            }
        };

        // Find path based on position
        let Some(path) = (match position {
            StationPosition::Start => graph.find_path_between_nodes(station_idx, existing_idx),
            StationPosition::End => graph.find_path_between_nodes(existing_idx, station_idx),
        }) else {
            return false;
        };

        // Determine starting node for path traversal
        let mut current_node = match position {
            StationPosition::Start => station_idx,
            StationPosition::End => existing_idx,
        };

        // Convert path edges into route segments
        let mut new_segments = Vec::new();
        for edge in &path {
            let Some((source, target)) = graph.graph.edge_endpoints(*edge) else {
                continue;
            };

            // Determine direction
            let is_forward = current_node == source;
            let next_node = if is_forward { target } else { source };

            // Select track
            let traveling_backward = !is_forward;
            let track_index = graph.select_track_for_direction(*edge, traveling_backward);

            // Check for passing loop or junction
            let is_passing_loop_or_junction = graph.graph.node_weight(current_node)
                .is_some_and(|node| {
                    node.as_station().is_some_and(|s| s.passing_loop) ||
                    node.as_junction().is_some()
                });
            let default_wait = if is_passing_loop_or_junction {
                Duration::seconds(0)
            } else {
                self.default_wait_time
            };

            // Get platform counts
            let source_platform_count = graph.graph.node_weight(source)
                .and_then(|n| n.as_station())
                .map_or(1, |s| s.platforms.len());

            let target_platform_count = graph.graph.node_weight(target)
                .and_then(|n| n.as_station())
                .map_or(1, |s| s.platforms.len());

            let origin_platform = graph.get_default_platform_for_arrival(*edge, false, source_platform_count, handedness);
            let destination_platform = graph.get_default_platform_for_arrival(*edge, true, target_platform_count, handedness);

            new_segments.push(RouteSegment {
                edge_index: edge.index(),
                track_index,
                origin_platform,
                destination_platform,
                duration: None,
                wait_time: default_wait,
            });

            current_node = next_node;
        }

        // Insert segments
        let route = match direction {
            RouteDirection::Forward => &mut self.forward_route,
            RouteDirection::Return => &mut self.return_route,
        };

        match position {
            StationPosition::Start => {
                for (i, segment) in new_segments.into_iter().enumerate() {
                    route.insert(i, segment);
                }
            }
            StationPosition::End => {
                route.extend(new_segments);
            }
        }

        // Sync return route if needed
        if matches!(direction, RouteDirection::Forward) {
            self.apply_route_sync_if_enabled();
        }

        true
    }
}

pub mod duration_serde {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(duration.num_seconds())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = i64::deserialize(deserializer)?;
        Ok(Duration::seconds(seconds))
    }
}

mod option_duration_serde {
    use chrono::Duration;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::ref_option)]
    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => serializer.serialize_some(&d.num_seconds()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<i64>::deserialize(deserializer)
            .map(|opt| opt.map(Duration::seconds))
    }
}

mod naive_datetime_serde {
    use chrono::NaiveDateTime;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(datetime: &NaiveDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&datetime.format("%Y-%m-%d %H:%M:%S").to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
            .map_err(serde::de::Error::custom)
    }
}

mod option_naive_datetime_serde {
    use chrono::NaiveDateTime;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::ref_option)]
    pub fn serialize<S>(datetime: &Option<NaiveDateTime>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match datetime {
            Some(dt) => serializer.serialize_some(&dt.format("%Y-%m-%d %H:%M:%S").to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDateTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer)?
            .map(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S"))
            .transpose()
            .map_err(serde::de::Error::custom)
    }
}

mod node_index_serde {
    use petgraph::stable_graph::NodeIndex;
    use serde::{Deserialize, Deserializer, Serializer};

    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S>(node: &NodeIndex, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let index_u32 = u32::try_from(node.index()).unwrap_or(u32::MAX);
        serializer.serialize_u32(index_u32)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NodeIndex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let index = u32::deserialize(deserializer)?;
        Ok(NodeIndex::new(index as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{RailwayGraph, Stations, Tracks};
    use crate::models::track::{Track, TrackDirection};

    fn create_test_segment(edge_index: usize) -> RouteSegment {
        RouteSegment {
            edge_index,
            track_index: 0,
            origin_platform: 0,
            destination_platform: 0,
            duration: Some(Duration::minutes(5)),
            wait_time: Duration::seconds(30),
        }
    }

    #[test]
    fn test_default_wait_time() {
        assert_eq!(default_wait_time(), Duration::seconds(30));
    }

    #[test]
    fn test_schedule_mode_default() {
        let mode = ScheduleMode::default();
        assert_eq!(mode, ScheduleMode::Auto);
    }

    #[test]
    fn test_create_from_ids() {
        let names = vec!["Line 1".to_string(), "Line 2".to_string()];
        let lines = Line::create_from_ids(&names, 0);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].name, "Line 1");
        assert_eq!(lines[1].name, "Line 2");
        assert_eq!(lines[0].frequency, Duration::hours(1));
        assert!(lines[0].visible);
        assert_eq!(lines[0].schedule_mode, ScheduleMode::Auto);
    }

    #[test]
    fn test_uses_edge() {
        let line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![create_test_segment(1), create_test_segment(2)],
            return_route: vec![create_test_segment(3)],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        assert!(line.uses_edge(1));
        assert!(line.uses_edge(2));
        assert!(line.uses_edge(3));
        assert!(!line.uses_edge(4));
    }

    #[test]
    fn test_uses_any_edge() {
        let line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![create_test_segment(1), create_test_segment(2)],
            return_route: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        assert!(line.uses_any_edge(&[1, 5, 6]));
        assert!(line.uses_any_edge(&[2]));
        assert!(!line.uses_any_edge(&[3, 4, 5]));
    }

    #[test]
    fn test_update_route_after_deletion_with_bypass() {
        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![
                create_test_segment(1),
                create_test_segment(2),
                create_test_segment(3),
            ],
            return_route: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        // Simulate deleting a station that used edges 1 and 2, creating bypass edge 10
        let removed_edges = vec![1, 2];
        let mut bypass_mapping = std::collections::HashMap::new();
        bypass_mapping.insert((1, 2), 10);

        line.update_route_after_deletion(&removed_edges, &bypass_mapping);

        // Should have bypass edge 10 and edge 3
        assert_eq!(line.forward_route.len(), 2);
        assert_eq!(line.forward_route[0].edge_index, 10);
        assert_eq!(line.forward_route[1].edge_index, 3);

        // Check combined duration
        let expected_duration = Duration::minutes(5) + Duration::seconds(30) + Duration::minutes(5);
        assert_eq!(line.forward_route[0].duration, Some(expected_duration));
    }

    #[test]
    fn test_update_route_after_deletion_without_bypass() {
        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![
                create_test_segment(1),
                create_test_segment(2),
            ],
            return_route: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        // Remove edge 1 but no bypass mapping
        let removed_edges = vec![1];
        let bypass_mapping = std::collections::HashMap::new();

        line.update_route_after_deletion(&removed_edges, &bypass_mapping);

        // Edge 1 should be removed, edge 2 should remain
        assert_eq!(line.forward_route.len(), 1);
        assert_eq!(line.forward_route[0].edge_index, 2);
    }

    #[test]
    fn test_fix_track_indices_after_change() {
        let mut graph = RailwayGraph::new();
        let idx1 = graph.add_or_get_station("A".to_string());
        let idx2 = graph.add_or_get_station("B".to_string());

        // Create double track edge (Forward and Backward)
        let edge = graph.add_track(idx1, idx2, vec![
            Track { direction: TrackDirection::Forward },
            Track { direction: TrackDirection::Backward },
        ]);

        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![RouteSegment {
                edge_index: edge.index(),
                track_index: 5, // Out of bounds
                origin_platform: 0,
                destination_platform: 0,
                duration: Some(Duration::minutes(5)),
                wait_time: Duration::seconds(30),
            }],
            return_route: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        line.fix_track_indices_after_change(edge.index(), 2, &graph);

        // Track index should be fixed to 0 (Forward track)
        assert_eq!(line.forward_route[0].track_index, 0);
    }

    #[test]
    fn test_is_track_incompatible() {
        let segment = TrackSegment {
            tracks: vec![
                Track { direction: TrackDirection::Forward },
                Track { direction: TrackDirection::Backward },
            ],
            distance: None,
            default_platform_source: None,
            default_platform_target: None,
        };

        // Forward route should be compatible with Forward track (index 0)
        assert!(!Line::is_track_incompatible(Some(&segment), 0, true));

        // Forward route should be incompatible with Backward track (index 1)
        assert!(Line::is_track_incompatible(Some(&segment), 1, true));

        // Return route should be compatible with Backward track (index 1)
        assert!(!Line::is_track_incompatible(Some(&segment), 1, false));

        // Return route should be incompatible with Forward track (index 0)
        assert!(Line::is_track_incompatible(Some(&segment), 0, false));
    }

    #[test]
    fn test_find_compatible_track() {
        let segment = TrackSegment {
            tracks: vec![
                Track { direction: TrackDirection::Backward },
                Track { direction: TrackDirection::Forward },
                Track { direction: TrackDirection::Bidirectional },
            ],
            distance: None,
            default_platform_source: None,
            default_platform_target: None,
        };

        // For forward route, should find first compatible track (index 1 - Forward)
        assert_eq!(Line::find_compatible_track(Some(&segment), true, 2), 1);

        // For return route, should find first compatible track (index 0 - Backward)
        assert_eq!(Line::find_compatible_track(Some(&segment), false, 2), 0);
    }

    #[test]
    fn test_route_segment_equality() {
        let seg1 = create_test_segment(1);
        let seg2 = create_test_segment(1);
        let seg3 = create_test_segment(2);

        assert_eq!(seg1, seg2);
        assert_ne!(seg1, seg3);
    }

    #[test]
    fn test_replace_split_edge() {
        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![
                create_test_segment(5),
                create_test_segment(10),
                create_test_segment(15),
            ],
            return_route: vec![
                create_test_segment(15),
                create_test_segment(10),
                create_test_segment(5),
            ],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        // Create a minimal test graph for platform assignment
        let graph = RailwayGraph::new();
        let platform_count = 2; // Passing loop with 2 platforms
        let handedness = TrackHandedness::RightHand;

        // Split edge 10 into edges 20 and 21
        line.replace_split_edge(10, 20, 21, 1, &graph, platform_count, handedness);

        // Forward route should have: 5, 20, 21, 15
        assert_eq!(line.forward_route.len(), 4);
        assert_eq!(line.forward_route[0].edge_index, 5);
        assert_eq!(line.forward_route[1].edge_index, 20);
        assert_eq!(line.forward_route[2].edge_index, 21);
        assert_eq!(line.forward_route[3].edge_index, 15);

        // Return route should have: 15, 21, 20, 5 (reversed order for split edges)
        assert_eq!(line.return_route.len(), 4);
        assert_eq!(line.return_route[0].edge_index, 15);
        assert_eq!(line.return_route[1].edge_index, 21);
        assert_eq!(line.return_route[2].edge_index, 20);
        assert_eq!(line.return_route[3].edge_index, 5);

        // Check duration is split in half
        assert_eq!(line.forward_route[1].duration, Some(Duration::minutes(5) / 2));
        assert_eq!(line.forward_route[2].duration, Some(Duration::minutes(5) / 2));
    }

    #[test]
    fn test_reroute_deleted_edge() {
        use crate::models::{Junctions, Junction};

        let mut graph = RailwayGraph::new();

        // Create: A -> B -> C with a junction creating an alternative path
        let a = graph.add_or_get_station("A".to_string());
        let b = graph.add_or_get_station("B".to_string());
        let c = graph.add_or_get_station("C".to_string());
        let j = graph.add_junction(Junction {
            name: Some("Junction".to_string()),
            position: None,
            routing_rules: vec![],
            label_position: None,
        });

        // Direct path: A -> B -> C
        let e1 = graph.add_track(a, b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(b, c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Alternative path through junction: B -> J -> C
        let _e3 = graph.add_track(b, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let _e4 = graph.add_track(j, c, vec![Track { direction: TrackDirection::Bidirectional }]);

        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![
                create_test_segment(e1.index()),
                create_test_segment(e2.index()),
            ],
            return_route: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        // Delete the direct edge B -> C
        graph.graph.remove_edge(e2);

        // Try to reroute - pass the endpoints we know (b and c)
        let changed = line.reroute_deleted_edge(e2.index(), b, c, &graph);

        // Should have found the alternative path through the junction
        assert!(changed);
        assert_eq!(line.forward_route.len(), 3);
        assert_eq!(line.forward_route[0].edge_index, e1.index());
        // The last two segments should be the alternative path (b->j->c)
        // We don't check exact indices since they could be e3/e4 in any order
    }

    #[test]
    fn test_reroute_deleted_edge_no_alternative() {
        let mut graph = RailwayGraph::new();

        // Create: A -> B with no alternative path
        let a = graph.add_or_get_station("A".to_string());
        let b = graph.add_or_get_station("B".to_string());

        let e1 = graph.add_track(a, b, vec![Track { direction: TrackDirection::Bidirectional }]);

        let mut line = Line {
            id: uuid::Uuid::new_v4(),
            name: "Test".to_string(),
            frequency: Duration::hours(1),
            color: "#FF0000".to_string(),
            thickness: 2.0,
            first_departure: BASE_MIDNIGHT,
            return_first_departure: BASE_MIDNIGHT,
            visible: true,
            schedule_mode: ScheduleMode::Auto,
            days_of_week: DaysOfWeek::ALL_DAYS,
            manual_departures: vec![],
            forward_route: vec![create_test_segment(e1.index())],
            return_route: vec![],
            sync_routes: true,
            auto_train_number_format: "{line} {seq:04}".to_string(),
            last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            return_last_departure: BASE_DATE.and_hms_opt(22, 0, 0).expect("valid time"),
            default_wait_time: default_wait_time(),
            first_stop_wait_time: default_first_stop_wait_time(),
            return_first_stop_wait_time: default_first_stop_wait_time(),
            sort_index: None,
            sync_departure_offsets: false,
            folder_id: None,
        };

        // Delete the edge
        graph.graph.remove_edge(e1);

        // Try to reroute - should fail because no alternative exists
        let changed = line.reroute_deleted_edge(e1.index(), a, b, &graph);

        // Should not have changed (no alternative path found)
        assert!(!changed);
        assert_eq!(line.forward_route.len(), 1);
        assert_eq!(line.forward_route[0].edge_index, e1.index());
    }
}