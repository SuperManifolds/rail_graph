use crate::models::{Line, Station};
use crate::constants::{BASE_DATE, GENERATION_END_HOUR};
use chrono::{Duration, NaiveDateTime, Timelike};

const MAX_JOURNEYS_PER_LINE: i32 = 100; // Limit to prevent performance issues

#[derive(Debug, Clone)]
pub struct TrainJourney {
    pub line_id: String,
    pub departure_time: NaiveDateTime,
    pub station_times: Vec<(String, NaiveDateTime)>, // (station_name, arrival_time)
    pub color: String,
}

impl TrainJourney {
    /// Generate train journeys for all lines throughout the day
    pub fn generate_journeys(lines: &[Line], stations: &[Station]) -> Vec<TrainJourney> {
        let Some(day_end) = BASE_DATE.and_hms_opt(23, 59, 59) else {
            return Vec::new();
        };

        let mut journeys = Vec::new();

        for line in lines {
            // Get all stations that this line serves, in order
            let line_stations: Vec<(String, NaiveDateTime)> = stations
                .iter()
                .filter_map(|station| {
                    station
                        .times
                        .get(&line.id)
                        .and_then(|&time_opt| time_opt)
                        .map(|time| (station.name.clone(), time))
                })
                .collect();

            if line_stations.is_empty() {
                continue;
            }

            // Generate forward journeys
            Self::generate_forward_journeys(&mut journeys, line, &line_stations, day_end);

            // Generate return journeys
            Self::generate_return_journeys(&mut journeys, line, &line_stations, day_end);
        }

        journeys
    }

    fn generate_forward_journeys(
        journeys: &mut Vec<TrainJourney>,
        line: &Line,
        line_stations: &[(String, NaiveDateTime)],
        day_end: NaiveDateTime,
    ) {
        let mut departure_time = line.first_departure;
        let mut journey_count = 0;

        while departure_time <= day_end && journey_count < MAX_JOURNEYS_PER_LINE {
            let mut station_times = Vec::new();

            for (station_name, offset_time) in line_stations {
                let offset_duration = Duration::hours(offset_time.hour() as i64)
                    + Duration::minutes(offset_time.minute() as i64)
                    + Duration::seconds(offset_time.second() as i64);

                let arrival_time = departure_time + offset_duration;
                station_times.push((station_name.clone(), arrival_time));
            }

            if station_times.len() >= 2 {
                journeys.push(TrainJourney {
                    line_id: line.id.clone(),
                    departure_time,
                    station_times,
                    color: line.color.clone(),
                });
                journey_count += 1;
            }

            departure_time += line.frequency;

            if departure_time.hour() > GENERATION_END_HOUR {
                break;
            }
        }
    }

    fn generate_return_journeys(
        journeys: &mut Vec<TrainJourney>,
        line: &Line,
        line_stations: &[(String, NaiveDateTime)],
        day_end: NaiveDateTime,
    ) {
        let mut return_stations = line_stations.to_vec();
        return_stations.reverse();

        if let Some((_, last_time)) = line_stations.last() {
            let mut return_departure_time = line.return_first_departure;
            let mut return_journey_count = 0;

            while return_departure_time <= day_end && return_journey_count < MAX_JOURNEYS_PER_LINE {
                let mut station_times = Vec::new();

                for (i, (station_name, _)) in return_stations.iter().enumerate() {
                    let return_offset = if let Some((_, original_time)) =
                        line_stations.get(return_stations.len() - 1 - i)
                    {
                        Duration::hours(last_time.hour() as i64 - original_time.hour() as i64)
                            + Duration::minutes(
                                last_time.minute() as i64 - original_time.minute() as i64,
                            )
                            + Duration::seconds(
                                last_time.second() as i64 - original_time.second() as i64,
                            )
                    } else {
                        Duration::zero()
                    };

                    let arrival_time = return_departure_time + return_offset;
                    station_times.push((station_name.clone(), arrival_time));
                }

                if station_times.len() >= 2 {
                    journeys.push(TrainJourney {
                        line_id: line.id.clone(),
                        departure_time: return_departure_time,
                        station_times,
                        color: line.color.clone(),
                    });
                    return_journey_count += 1;
                }

                return_departure_time += line.frequency;

                if return_departure_time.hour() > GENERATION_END_HOUR {
                    break;
                }
            }
        }
    }
}

