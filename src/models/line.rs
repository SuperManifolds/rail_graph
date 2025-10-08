use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use crate::constants::BASE_DATE;
use petgraph::graph::NodeIndex;
use super::{RailwayGraph, TrackSegment, TrackDirection};

fn generate_random_color(seed: usize) -> String {
    // Use a simple hash-based color generator for deterministic but varied colors
    let hue = ((seed * 137) % 360) as f64;
    let saturation = 65.0 + ((seed * 97) % 20) as f64; // 65-85%
    let lightness = 55.0 + ((seed * 53) % 15) as f64;  // 55-70%

    // Convert HSL to RGB
    let c = (1.0 - (2.0 * lightness / 100.0 - 1.0).abs()) * saturation / 100.0;
    let x = c * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = lightness / 100.0 - c / 2.0;

    let (r, g, b) = match hue as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    format!("#{:02X}{:02X}{:02X}",
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8
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
    pub fn create_from_ids(line_ids: &[String]) -> Vec<Line> {
        line_ids
            .iter()
            .enumerate()
            .map(|(i, id)| Line {
                id: id.clone(),
                frequency: Duration::hours(1), // Default, configurable by user
                color: generate_random_color(i),
                thickness: 2.0,
                first_departure: BASE_DATE.and_hms_opt(5, i as u32 * 15, 0)
                    .unwrap_or_else(|| BASE_DATE.and_hms_opt(5, 0, 0).expect("Valid time")),
                return_first_departure: BASE_DATE.and_hms_opt(6, i as u32 * 15, 0)
                    .unwrap_or_else(|| BASE_DATE.and_hms_opt(6, 0, 0).expect("Valid time")),
                visible: true,
                schedule_mode: ScheduleMode::Auto,
                manual_departures: Vec::new(),
                forward_route: Vec::new(),
                return_route: Vec::new(),
            })
            .collect()
    }

    /// Update route after station deletion with bypass edges
    /// removed_edges: edges that were removed
    /// bypass_mapping: maps (old_edge1, old_edge2) -> new_bypass_edge
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
            let next_segment = match route.get(i + 1) {
                Some(seg) => seg,
                None => {
                    i += 1;
                    continue;
                }
            };

            // Check if we have a bypass edge for this pair
            let bypass_edge_idx = match bypass_mapping.get(&(segment.edge_index, next_segment.edge_index)) {
                Some(&idx) => idx,
                None => {
                    i += 1;
                    continue;
                }
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

    pub fn serialize<S>(node: &NodeIndex, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(node.index() as u32)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NodeIndex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let index = u32::deserialize(deserializer)?;
        Ok(NodeIndex::new(index as usize))
    }
}