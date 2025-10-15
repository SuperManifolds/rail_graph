use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use crate::constants::{BASE_DATE, BASE_MIDNIGHT};
use petgraph::stable_graph::NodeIndex;
use super::{RailwayGraph, TrackSegment, TrackDirection, Tracks, DaysOfWeek};

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
fn generate_random_color(seed: usize) -> String {
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
    #[serde(with = "duration_serde")]
    pub duration: Duration,
    #[serde(with = "duration_serde", default = "default_wait_time")]
    pub wait_time: Duration,
}

fn default_wait_time() -> Duration {
    Duration::seconds(30)
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    pub id: String,
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

fn default_train_number_format() -> String {
    "{line} {seq:04}".to_string()
}

impl Line {
    /// Create lines from IDs with default settings
    #[must_use]
    pub fn create_from_ids(line_ids: &[String]) -> Vec<Line> {
        line_ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let offset_minutes = u32::try_from(i).unwrap_or(0).saturating_mul(15);
                Line {
                    id: id.clone(),
                    frequency: Duration::hours(1), // Default, configurable by user
                    color: generate_random_color(i),
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
            let combined_duration = segment.duration + segment.wait_time + next_segment.duration;

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

    /// Check if a track is incompatible with the route direction
    fn is_track_incompatible(track_segment: Option<&TrackSegment>, track_index: usize, is_forward: bool) -> bool {
        let Some(ts) = track_segment else {
            return false;
        };

        let Some(track) = ts.tracks.get(track_index) else {
            return false;
        };

        if is_forward {
            !matches!(track.direction, TrackDirection::Forward | TrackDirection::Bidirectional)
        } else {
            !matches!(track.direction, TrackDirection::Backward | TrackDirection::Bidirectional)
        }
    }

    /// Find a compatible track for a given route direction
    fn find_compatible_track(track_segment: Option<&TrackSegment>, is_forward: bool, max_index: usize) -> usize {
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
    pub fn replace_split_edge(&mut self, old_edge: usize, new_edge1: usize, new_edge2: usize, track_count: usize) {
        Self::replace_split_edge_in_route(&mut self.forward_route, old_edge, new_edge1, new_edge2, track_count);
        Self::replace_split_edge_in_route(&mut self.return_route, old_edge, new_edge2, new_edge1, track_count);
    }

    fn replace_split_edge_in_route(route: &mut Vec<RouteSegment>, old_edge: usize, first_edge: usize, second_edge: usize, track_count: usize) {
        let mut new_route = Vec::new();

        for segment in route.iter() {
            if segment.edge_index == old_edge {
                // Split this segment into two through the junction
                new_route.push(RouteSegment {
                    edge_index: first_edge,
                    track_index: segment.track_index.min(track_count.saturating_sub(1)),
                    origin_platform: segment.origin_platform,
                    destination_platform: 0,
                    duration: segment.duration / 2,
                    wait_time: segment.wait_time,
                });
                new_route.push(RouteSegment {
                    edge_index: second_edge,
                    track_index: segment.track_index.min(track_count.saturating_sub(1)),
                    origin_platform: 0,
                    destination_platform: segment.destination_platform,
                    duration: segment.duration / 2,
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

    /// Sync return route from forward route if `sync_routes` is enabled
    /// Preserves user-configured wait times, track indices, and platform assignments
    /// from existing return segments while syncing the route structure and durations
    pub fn apply_route_sync_if_enabled(&mut self) {
        use std::collections::HashMap;

        if !self.sync_routes {
            return;
        }

        // Build a map of edge_index -> (wait_time, track_index, origin_platform, destination_platform)
        // This preserves all user-configured settings from the existing return route
        let existing_settings: HashMap<usize, (Duration, usize, usize, usize)> = self.return_route
            .iter()
            .map(|seg| (
                seg.edge_index,
                (seg.wait_time, seg.track_index, seg.origin_platform, seg.destination_platform)
            ))
            .collect();

        // Create new return route by reversing forward route
        let mut new_return_route = Vec::new();

        for forward_seg in self.forward_route.iter().rev() {
            // If we have existing settings for this edge in return route, preserve them
            if let Some((wait_time, track_index, origin_platform, destination_platform)) =
                existing_settings.get(&forward_seg.edge_index) {
                // Preserve all user settings
                new_return_route.push(RouteSegment {
                    edge_index: forward_seg.edge_index,
                    track_index: *track_index,
                    origin_platform: *origin_platform,
                    destination_platform: *destination_platform,
                    duration: forward_seg.duration,
                    wait_time: *wait_time,
                });
            } else {
                // This is a new edge not in the return route, use defaults from forward route
                // but swap platforms for the reverse direction
                new_return_route.push(RouteSegment {
                    edge_index: forward_seg.edge_index,
                    track_index: forward_seg.track_index,
                    origin_platform: forward_seg.destination_platform,
                    destination_platform: forward_seg.origin_platform,
                    duration: forward_seg.duration,
                    wait_time: forward_seg.wait_time,
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
                    duration: segment.duration / path.len().max(1) as i32,
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
}

mod duration_serde {
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
            duration: Duration::minutes(5),
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
        let ids = vec!["Line 1".to_string(), "Line 2".to_string()];
        let lines = Line::create_from_ids(&ids);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].id, "Line 1");
        assert_eq!(lines[1].id, "Line 2");
        assert_eq!(lines[0].frequency, Duration::hours(1));
        assert!(lines[0].visible);
        assert_eq!(lines[0].schedule_mode, ScheduleMode::Auto);
    }

    #[test]
    fn test_uses_edge() {
        let line = Line {
            id: "Test".to_string(),
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
        };

        assert!(line.uses_edge(1));
        assert!(line.uses_edge(2));
        assert!(line.uses_edge(3));
        assert!(!line.uses_edge(4));
    }

    #[test]
    fn test_uses_any_edge() {
        let line = Line {
            id: "Test".to_string(),
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
        };

        assert!(line.uses_any_edge(&[1, 5, 6]));
        assert!(line.uses_any_edge(&[2]));
        assert!(!line.uses_any_edge(&[3, 4, 5]));
    }

    #[test]
    fn test_update_route_after_deletion_with_bypass() {
        let mut line = Line {
            id: "Test".to_string(),
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
        assert_eq!(line.forward_route[0].duration, expected_duration);
    }

    #[test]
    fn test_update_route_after_deletion_without_bypass() {
        let mut line = Line {
            id: "Test".to_string(),
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
            id: "Test".to_string(),
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
                duration: Duration::minutes(5),
                wait_time: Duration::seconds(30),
            }],
            return_route: vec![],
            sync_routes: true,
                auto_train_number_format: "{line} {seq:04}".to_string(),
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
            id: "Test".to_string(),
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
        };

        // Split edge 10 into edges 20 and 21
        line.replace_split_edge(10, 20, 21, 1);

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
        assert_eq!(line.forward_route[1].duration, Duration::minutes(5) / 2);
        assert_eq!(line.forward_route[2].duration, Duration::minutes(5) / 2);
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
        });

        // Direct path: A -> B -> C
        let e1 = graph.add_track(a, b, vec![Track { direction: TrackDirection::Bidirectional }]);
        let e2 = graph.add_track(b, c, vec![Track { direction: TrackDirection::Bidirectional }]);

        // Alternative path through junction: B -> J -> C
        let _e3 = graph.add_track(b, j, vec![Track { direction: TrackDirection::Bidirectional }]);
        let _e4 = graph.add_track(j, c, vec![Track { direction: TrackDirection::Bidirectional }]);

        let mut line = Line {
            id: "Test".to_string(),
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
            id: "Test".to_string(),
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