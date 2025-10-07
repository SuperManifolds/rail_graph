use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use crate::constants::BASE_DATE;
use petgraph::graph::NodeIndex;

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
    pub route: Vec<RouteSegment>,
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
                route: Vec::new(),
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
        let mut new_route = Vec::new();
        let mut i = 0;

        while i < self.route.len() {
            let segment = &self.route[i];

            // If this segment uses a removed edge
            if removed_edges.contains(&segment.edge_index) {
                // Look ahead to find the next segment
                if i + 1 < self.route.len() {
                    let next_segment = &self.route[i + 1];

                    // Check if we have a bypass edge for this pair
                    if let Some(&bypass_edge_idx) = bypass_mapping.get(&(segment.edge_index, next_segment.edge_index)) {
                        // Combine durations (travel time + wait time at deleted station + next travel time)
                        let combined_duration = segment.duration + segment.wait_time + next_segment.duration;

                        new_route.push(RouteSegment {
                            edge_index: bypass_edge_idx,
                            track_index: 0,
                            duration: combined_duration,
                            wait_time: next_segment.wait_time,
                        });

                        i += 2; // Skip both segments
                        continue;
                    }
                }

                // If we can't create a bypass, skip this segment
                i += 1;
            } else {
                // Keep segments that don't use removed edges
                new_route.push(segment.clone());
                i += 1;
            }
        }

        self.route = new_route;
    }

    /// Fix track indices after track count changes on an edge
    /// Resets track_index to 0 if it references a track that no longer exists
    pub fn fix_track_indices_after_change(&mut self, edge_index: usize, new_track_count: usize) {
        let max_track_index = new_track_count.saturating_sub(1);

        for segment in &mut self.route {
            if segment.edge_index == edge_index && segment.track_index > max_track_index {
                segment.track_index = 0;
            }
        }
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