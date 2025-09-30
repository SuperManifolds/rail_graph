use chrono::NaiveDate;

/// Base date used for all time calculations
pub const BASE_DATE: NaiveDate = match NaiveDate::from_ymd_opt(2024, 1, 1) {
    Some(date) => date,
    None => panic!("Invalid base date"),
};

/// Hour after which to stop generating journeys/departures (10 PM)
pub const GENERATION_END_HOUR: u32 = 22;