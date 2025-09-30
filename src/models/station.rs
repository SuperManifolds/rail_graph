use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Station {
    pub name: String,
    pub times: HashMap<String, Option<NaiveDateTime>>,
}

impl Station {
    /// Get the time for a specific line at this station
    pub fn get_time(&self, line_id: &str) -> Option<NaiveDateTime> {
        self.times.get(line_id).and_then(|t| *t)
    }

    /// Parse station times from a CSV row
    pub fn parse_times_from_csv(
        row: &csv::StringRecord,
        line_ids: &[String],
    ) -> HashMap<String, Option<NaiveDateTime>> {
        let base_date = NaiveDate::from_ymd_opt(2024, 1, 1).expect("Valid date");
        let mut times = HashMap::new();

        for (i, line_id) in line_ids.iter().enumerate() {
            let time = row
                .get(i + 1)
                .filter(|s| !s.is_empty())
                .and_then(|s| {
                    // Parse as time and combine with base date
                    if let Ok(naive_time) = chrono::NaiveTime::parse_from_str(s, "%H:%M:%S") {
                        Some(base_date.and_time(naive_time))
                    } else {
                        None
                    }
                });

            times.insert(line_id.clone(), time);
        }

        times
    }

    /// Parse stations from CSV records
    pub fn parse_from_csv(
        records: csv::StringRecordsIter<&[u8]>,
        line_ids: &[String],
    ) -> Vec<Station> {
        let mut stations = Vec::new();

        for record in records {
            let Ok(row) = record else { continue };

            let Some(station_name) = row.get(0) else { continue };
            if station_name.is_empty() {
                continue;
            }

            let times = Self::parse_times_from_csv(&row, line_ids);

            stations.push(Station {
                name: station_name.to_string(),
                times,
            });
        }

        stations
    }
}