use crate::models::{Line, Station, ScheduleMode};
use crate::constants::GENERATION_END_HOUR;
use chrono::{Duration, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Departure {
    pub line_id: String,
    pub station: String,
    pub time: NaiveDateTime,
}

impl Departure {
    /// Generate departures for all lines and stations for a given day
    pub fn generate_departures(
        lines: &[Line],
        stations: &[Station],
        current_time: NaiveDateTime,
    ) -> Vec<Departure> {
        let base_date = current_time.date();
        let Some(day_end) = base_date.and_hms_opt(23, 59, 59) else {
            return Vec::new();
        };

        // Show a wider window for debugging
        let window_start = base_date.and_hms_opt(0, 0, 0).unwrap_or(current_time);
        let window_end = base_date.and_hms_opt(23, 59, 59).unwrap_or(current_time);

        let mut departures = Vec::new();

        for line in lines {
            for station in stations {
                generate_station_departures(
                    line,
                    station,
                    day_end,
                    window_start,
                    window_end,
                    &mut departures,
                );
            }
        }

        departures.sort_by_key(|d| (d.time, d.line_id.clone(), d.station.clone()));
        departures
    }
}

fn generate_station_departures(
    line: &Line,
    station: &Station,
    day_end: NaiveDateTime,
    window_start: NaiveDateTime,
    window_end: NaiveDateTime,
    departures: &mut Vec<Departure>,
) {
    let Some(offset_time) = station.get_time(&line.id) else {
        return;
    };

    match line.schedule_mode {
        ScheduleMode::Auto => {
            // Generate multiple departures throughout the day based on frequency
            let mut base_departure = line.first_departure;

            // Calculate the time offset from the offset_time (assuming it's relative to start of day)
            let offset_duration = Duration::hours(offset_time.hour() as i64)
                + Duration::minutes(offset_time.minute() as i64)
                + Duration::seconds(offset_time.second() as i64);

            // Add the offset to get the actual arrival time at this station
            while base_departure <= day_end {
                let arrival_time = base_departure + offset_duration;

                if arrival_time >= window_start && arrival_time <= window_end {
                    departures.push(Departure {
                        line_id: line.id.clone(),
                        station: station.name.clone(),
                        time: arrival_time,
                    });
                }

                // Move to next departure based on frequency
                base_departure += line.frequency;

                if base_departure.hour() > GENERATION_END_HOUR {
                    break; // Stop generating after 10 PM
                }
            }
        }
        ScheduleMode::Manual => {
            // Generate departures from manual departure list
            for manual_dep in &line.manual_departures {
                // Only generate departure if this station is either the from or to station
                if station.name != manual_dep.from_station && station.name != manual_dep.to_station {
                    continue;
                }

                // Calculate the time offset from the offset_time
                let offset_duration = Duration::hours(offset_time.hour() as i64)
                    + Duration::minutes(offset_time.minute() as i64)
                    + Duration::seconds(offset_time.second() as i64);

                // The manual departure time is the departure from the from_station
                let arrival_time = if station.name == manual_dep.from_station {
                    manual_dep.time
                } else {
                    // For other stations, add the offset
                    manual_dep.time + offset_duration
                };

                if arrival_time >= window_start && arrival_time <= window_end {
                    departures.push(Departure {
                        line_id: line.id.clone(),
                        station: station.name.clone(),
                        time: arrival_time,
                    });
                }
            }
        }
    }
}

