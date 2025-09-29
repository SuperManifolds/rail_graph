use crate::models::{Departure, Line, Station, TrainJourney};
use chrono::{Duration, NaiveTime, Timelike};
use std::collections::HashMap;

const LINE_COLORS: &[&str] = &[
    "#FF6B6B", "#4ECDC4", "#45B7D1", "#96CEB4", "#FECA57"
];

pub fn parse_csv_data() -> (Vec<Line>, Vec<Station>) {
    let csv_content = include_str!("../lines.csv");

    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(csv_content.as_bytes());

    let mut records = reader.records();

    let Some(Ok(header)) = records.next() else {
        return (Vec::new(), Vec::new());
    };

    let line_ids = extract_line_ids(&header);
    let lines = create_lines(&line_ids);
    let stations = parse_stations(records, &line_ids);

    (lines, stations)
}

fn extract_line_ids(header: &csv::StringRecord) -> Vec<String> {
    header.iter()
        .skip(1)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn create_lines(line_ids: &[String]) -> Vec<Line> {
    line_ids
        .iter()
        .enumerate()
        .map(|(i, id)| Line {
            id: id.clone(),
            frequency: Duration::minutes(30), // Default, configurable by user
            color: LINE_COLORS[i % LINE_COLORS.len()].to_string(),
            first_departure: NaiveTime::from_hms_opt(5, i as u32 * 15, 0)
                .unwrap_or_else(|| NaiveTime::from_hms_opt(5, 0, 0).expect("Valid time")),
        })
        .collect()
}


fn parse_stations(
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

        let times = parse_station_times(&row, line_ids);

        stations.push(Station {
            name: station_name.to_string(),
            times,
        });
    }

    stations
}

fn parse_station_times(
    row: &csv::StringRecord,
    line_ids: &[String],
) -> HashMap<String, Option<NaiveTime>> {
    let mut times = HashMap::new();

    for (i, line_id) in line_ids.iter().enumerate() {
        let time = row
            .get(i + 1)
            .filter(|s| !s.is_empty())
            .and_then(|s| NaiveTime::parse_from_str(s, "%H:%M:%S").ok());

        times.insert(line_id.clone(), time);
    }

    times
}

pub fn generate_departures(
    lines: &[Line],
    stations: &[Station],
    current_time: NaiveTime,
    window_hours: i64,
) -> Vec<Departure> {
    let Some(day_end) = NaiveTime::from_hms_opt(23, 59, 59) else {
        return Vec::new();
    };

    // Show a wider window for debugging
    let window_start = NaiveTime::from_hms_opt(0, 0, 0).unwrap_or(current_time);
    let window_end = NaiveTime::from_hms_opt(23, 59, 59).unwrap_or(current_time);

    let mut departures = Vec::new();

    for line in lines {
        for station in stations {
            if let Some(offset_time) = get_station_time(station, &line.id) {
                // Generate multiple departures throughout the day based on frequency
                let mut base_departure = line.first_departure;

                // Add the offset to get the actual arrival time at this station
                while base_departure <= day_end {
                    let arrival_time = base_departure.overflowing_add_signed(
                        Duration::hours(offset_time.hour() as i64) +
                        Duration::minutes(offset_time.minute() as i64) +
                        Duration::seconds(offset_time.second() as i64)
                    ).0;

                    if arrival_time >= window_start && arrival_time <= window_end {
                        departures.push(Departure {
                            line_id: line.id.clone(),
                            station: station.name.clone(),
                            time: arrival_time,
                        });
                    }

                    // Move to next departure based on frequency
                    let (next_time, _) = base_departure.overflowing_add_signed(line.frequency);
                    base_departure = next_time;

                    if base_departure.hour() > 22 {
                        break; // Stop generating after 10 PM
                    }
                }
            }
        }
    }

    departures.sort_by_key(|d| (d.time, d.line_id.clone(), d.station.clone()));
    departures
}

pub fn generate_train_journeys(
    lines: &[Line],
    stations: &[Station],
    _current_time: NaiveTime,
    _window_hours: i64,
) -> Vec<TrainJourney> {
    let Some(day_end) = NaiveTime::from_hms_opt(23, 59, 59) else {
        return Vec::new();
    };

    // Show a wider window for the Marey chart
    let window_start = NaiveTime::from_hms_opt(0, 0, 0).unwrap_or_default();
    let window_end = NaiveTime::from_hms_opt(23, 59, 59).unwrap_or_default();

    let mut journeys = Vec::new();

    for line in lines {
        // Get all stations that this line serves, in order
        let line_stations: Vec<(String, NaiveTime)> = stations
            .iter()
            .filter_map(|station| {
                station.times.get(&line.id)
                    .and_then(|&time_opt| time_opt)
                    .map(|time| (station.name.clone(), time))
            })
            .collect();

        if line_stations.is_empty() {
            continue;
        }

        // Generate multiple train journeys throughout the day
        let mut departure_time = line.first_departure;

        while departure_time <= day_end {
            // Create a journey with all station times
            let mut station_times = Vec::new();
            let mut journey_valid = true;
            let mut last_time = departure_time;

            for (station_name, offset_time) in &line_stations {
                let (arrival_time, _) = departure_time.overflowing_add_signed(
                    Duration::hours(offset_time.hour() as i64) +
                    Duration::minutes(offset_time.minute() as i64) +
                    Duration::seconds(offset_time.second() as i64)
                );

                // If we've wrapped around midnight (time goes backwards), truncate this journey
                if arrival_time < last_time && last_time.hour() > 20 {
                    // Only include stations up to midnight
                    if last_time < day_end {
                        // Add a final point at 23:59:59 for this station if needed
                        station_times.push((station_name.clone(), day_end));
                    }
                    break;
                }

                station_times.push((station_name.clone(), arrival_time));
                last_time = arrival_time;
            }

            // Only add journey if it has at least 2 stations
            if station_times.len() >= 2 && journey_valid {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time,
                    station_times,
                    color: line.color.clone(),
                });
            }

            // Move to next departure
            let (next_time, _) = departure_time.overflowing_add_signed(line.frequency);
            departure_time = next_time;

            // Stop if we're getting too late
            if departure_time.hour() > 22 {
                break;
            }
        }
    }

    journeys
}

fn get_station_time(station: &Station, line_id: &str) -> Option<NaiveTime> {
    station.times.get(line_id).and_then(|t| *t)
}

fn generate_line_departures(
    line_id: &str,
    station_name: &str,
    base_time: NaiveTime,
    frequency: Duration,
    window_start: NaiveTime,
    window_end: NaiveTime,
    day_end: NaiveTime,
) -> Vec<Departure> {
    let mut departures = Vec::new();
    let mut departure_time = base_time;

    while departure_time <= day_end {
        if departure_time >= window_start && departure_time <= window_end {
            departures.push(Departure {
                line_id: line_id.to_string(),
                station: station_name.to_string(),
                time: departure_time,
            });
        }

        let (next_time, _) = departure_time.overflowing_add_signed(frequency);
        departure_time = next_time;

        // Break if we've wrapped around to the next day
        if departure_time < base_time {
            break;
        }
    }

    departures
}