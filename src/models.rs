use chrono::{Duration, NaiveTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub times: HashMap<String, Option<NaiveTime>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    pub id: String,
    #[serde(with = "duration_serde")]
    pub frequency: Duration,
    pub color: String,
    #[serde(with = "naive_time_serde")]
    pub first_departure: NaiveTime,
    #[serde(with = "naive_time_serde")]
    pub return_first_departure: NaiveTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Departure {
    pub line_id: String,
    pub station: String,
    pub time: NaiveTime,
}

#[derive(Debug, Clone)]
pub struct TrainJourney {
    pub line_id: String,
    pub departure_time: NaiveTime,
    pub station_times: Vec<(String, NaiveTime)>, // (station_name, arrival_time)
    pub color: String,
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

mod naive_time_serde {
    use chrono::NaiveTime;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(time: &NaiveTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&time.format("%H:%M:%S").to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NaiveTime::parse_from_str(&s, "%H:%M:%S")
            .map_err(serde::de::Error::custom)
    }
}