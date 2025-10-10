use chrono::{NaiveDateTime, NaiveTime};
use crate::constants::BASE_DATE;

/// Convert a `NaiveDateTime` to a fraction of hours since `BASE_DATE`
#[must_use]
pub fn time_to_fraction(time: NaiveDateTime) -> f64 {
    let base_datetime = BASE_DATE.and_hms_opt(0, 0, 0).expect("Valid datetime");
    let duration_since_base = time.signed_duration_since(base_datetime);

    // Break down into hours, minutes, seconds to avoid precision loss
    // This avoids converting large i64 millisecond values to f64
    let total_seconds = duration_since_base.num_seconds();
    let hours = total_seconds / 3600;
    let remaining_seconds = total_seconds % 3600;
    let minutes = remaining_seconds / 60;
    let seconds = remaining_seconds % 60;

    // Get subsecond precision from milliseconds
    let total_ms = duration_since_base.num_milliseconds();
    let milliseconds = total_ms % 1000;

    // Convert to fractional hours with full precision
    // These casts truncate for very large values, but are correct for typical timetable ranges
    #[allow(clippy::cast_possible_truncation)]
    let hours_f64 = f64::from(hours as i32);
    #[allow(clippy::cast_possible_truncation)]
    let minutes_f64 = f64::from(minutes as i32);
    #[allow(clippy::cast_possible_truncation)]
    let seconds_f64 = f64::from(seconds as i32);
    #[allow(clippy::cast_possible_truncation)]
    let millis_f64 = f64::from(milliseconds as i32);

    hours_f64 + minutes_f64 / 60.0 + seconds_f64 / 3600.0 + millis_f64 / 3_600_000.0
}

/// Parse a time string in HH:MM:SS format
///
/// # Errors
///
/// Returns an error if the string cannot be parsed as a valid time in HH:MM:SS format.
pub fn parse_time_hms(s: &str) -> Result<NaiveTime, chrono::ParseError> {
    NaiveTime::parse_from_str(s, "%H:%M:%S")
}