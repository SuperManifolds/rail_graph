use chrono::{NaiveDateTime, NaiveTime};
use crate::constants::BASE_DATE;

/// Convert a `NaiveDateTime` to a fraction of hours since `BASE_DATE`
pub fn time_to_fraction(time: NaiveDateTime) -> f64 {
    let base_datetime = BASE_DATE.and_hms_opt(0, 0, 0).expect("Valid datetime");
    let duration_since_base = time.signed_duration_since(base_datetime);
    let total_seconds = duration_since_base.num_seconds() as f64;
    total_seconds / 3600.0 // Convert to hours
}

/// Parse a time string in HH:MM:SS format
pub fn parse_time_hms(s: &str) -> Result<NaiveTime, chrono::ParseError> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
}