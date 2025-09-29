use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub times: HashMap<String, Option<NaiveDateTime>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    pub id: String,
    #[serde(with = "duration_serde")]
    pub frequency: Duration,
    pub color: String,
    #[serde(with = "naive_datetime_serde")]
    pub first_departure: NaiveDateTime,
    #[serde(with = "naive_datetime_serde")]
    pub return_first_departure: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Departure {
    pub line_id: String,
    pub station: String,
    pub time: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct TrainJourney {
    pub line_id: String,
    pub departure_time: NaiveDateTime,
    pub station_times: Vec<(String, NaiveDateTime)>, // (station_name, arrival_time)
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SegmentState {
    // Key is the index of the second station in the segment
    // So segment between stations[i] and stations[i+1] is stored at key i+1
    pub double_tracked_segments: std::collections::HashSet<usize>,
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