use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use crate::constants::{BASE_DATE, BASE_MIDNIGHT};
use petgraph::graph::NodeIndex;
use super::{RailwayGraph, TrackSegment, TrackDirection, Tracks};

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
    pub manual_departures: Vec<ManualDeparture>,
    #[serde(default)]
    pub forward_route: Vec<RouteSegment>,
    #[serde(default)]
    pub return_route: Vec<RouteSegment>,
}

fn default_visible() -> bool {
    true
}

fn default_thickness() -> f64 {
    2.0
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
                    manual_departures: Vec::new(),
                    forward_route: Vec::new(),
                    return_route: Vec::new(),
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
        let max_track_index = new_track_count.saturating_sub(1);
        let edge_idx = petgraph::graph::EdgeIndex::new(edge_index);

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

    /// Check if this line uses any of the given edges in either route
    #[must_use]
    pub fn uses_any_edge(&self, edge_indices: &[usize]) -> bool {
        self.forward_route.iter().any(|segment| edge_indices.contains(&segment.edge_index)) ||
        self.return_route.iter().any(|segment| edge_indices.contains(&segment.edge_index))
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
    use petgraph::graph::NodeIndex;
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