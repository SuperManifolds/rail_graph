use chrono::{NaiveDateTime, NaiveTime};
use crate::constants::BASE_DATE;

/// Convert a `NaiveDateTime` to a fraction of hours since `BASE_DATE`
#[must_use]
pub fn time_to_fraction(time: NaiveDateTime) -> f64 {
    let base_datetime = BASE_DATE.and_time(NaiveTime::MIN);
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn test_time_to_fraction_midnight() {
        let midnight = BASE_DATE.and_hms_opt(0, 0, 0).expect("valid time");
        let fraction = time_to_fraction(midnight);
        assert_eq!(fraction, 0.0);
    }

    #[test]
    fn test_time_to_fraction_noon() {
        let noon = BASE_DATE.and_hms_opt(12, 0, 0).expect("valid time");
        let fraction = time_to_fraction(noon);
        assert_eq!(fraction, 12.0);
    }

    #[test]
    fn test_time_to_fraction_with_minutes() {
        let time = BASE_DATE.and_hms_opt(8, 30, 0).expect("valid time");
        let fraction = time_to_fraction(time);
        assert_eq!(fraction, 8.5);
    }

    #[test]
    fn test_time_to_fraction_with_seconds() {
        let time = BASE_DATE.and_hms_opt(1, 0, 30).expect("valid time");
        let fraction = time_to_fraction(time);
        assert_eq!(fraction, 1.0 + 30.0 / 3600.0);
    }

    #[test]
    fn test_time_to_fraction_complex() {
        let time = BASE_DATE.and_hms_opt(14, 45, 30).expect("valid time");
        let fraction = time_to_fraction(time);
        let expected = 14.0 + 45.0 / 60.0 + 30.0 / 3600.0;
        assert_eq!(fraction, expected);
    }

    #[test]
    fn test_parse_time_hms_valid() {
        let result = parse_time_hms("08:30:45");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 8);
        assert_eq!(time.minute(), 30);
        assert_eq!(time.second(), 45);
    }

    #[test]
    fn test_parse_time_hms_midnight() {
        let result = parse_time_hms("00:00:00");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_parse_time_hms_invalid_format() {
        let result = parse_time_hms("08:30");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_hms_invalid_hour() {
        let result = parse_time_hms("25:00:00");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_hms_invalid_minute() {
        let result = parse_time_hms("12:60:00");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_time_hms_empty_string() {
        let result = parse_time_hms("");
        assert!(result.is_err());
    }
}