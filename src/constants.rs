use chrono::{NaiveDate, NaiveDateTime};

/// Base date used for all time calculations
pub const BASE_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2024, 1, 1) {
    Some(date) => date,
    None => panic!("Invalid base date"),
};

/// Base midnight datetime (`BASE_DATE` at 00:00:00)
pub const BASE_MIDNIGHT: NaiveDateTime = match BASE_DATE.and_hms_opt(0, 0, 0) {
    Some(dt) => dt,
    None => panic!("Invalid base midnight"),
};

/// Default departure time for new manual departures (`BASE_DATE` at 08:00:00)
pub const DEFAULT_DEPARTURE_TIME: NaiveDateTime = match BASE_DATE.and_hms_opt(8, 0, 0) {
    Some(dt) => dt,
    None => panic!("Invalid default departure time"),
};

/// Hour after which to stop generating journeys/departures (10 PM)
pub const GENERATION_END_HOUR: u32 = 22;