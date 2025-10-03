use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use crate::constants::BASE_DATE;

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
    #[serde(with = "duration_serde")]
    pub duration: Duration,
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
    pub from_station: String,
    pub to_station: String,
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