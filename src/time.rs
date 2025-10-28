use chrono::{NaiveDateTime, NaiveTime};
use crate::constants::BASE_DATE;
use wasm_bindgen::JsValue;

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

/// Parse a flexible time string that accepts NIMBY Rails format
///
/// Supports:
/// - Single number: seconds (e.g., "45" = 00:00:45)
/// - Two numbers: minutes, seconds (e.g., "3.30" = 00:03:30)
/// - Three numbers: hours, minutes, seconds (e.g., "5.15." = 05:15:00)
/// - Separators: . , : ; (e.g., "1:2:3" or "1.2.3" or "1,2,3" or "1;2;3")
/// - Empty parts treated as zero (e.g., "6.." = 06:00:00)
#[must_use]
pub fn parse_flexible_time(input: &str) -> Option<(i64, i64, i64)> {
    // Reject empty or whitespace-only strings
    if input.trim().is_empty() {
        return None;
    }

    let parts: Vec<&str> = input.split(['.', ',', ':', ';']).collect();

    let parse_or_zero = |s: &str| -> Option<i64> {
        if s.is_empty() {
            Some(0)
        } else {
            s.parse().ok()
        }
    };

    match parts.len() {
        1 => {
            // Single part must not be empty
            if parts[0].is_empty() {
                return None;
            }
            let seconds = parse_or_zero(parts[0])?;
            Some((0, 0, seconds))
        }
        2 => {
            let minutes = parse_or_zero(parts[0])?;
            let seconds = parse_or_zero(parts[1])?;
            Some((0, minutes, seconds))
        }
        3 => {
            let hours = parse_or_zero(parts[0])?;
            let minutes = parse_or_zero(parts[1])?;
            let seconds = parse_or_zero(parts[2])?;
            Some((hours, minutes, seconds))
        }
        _ => None,
    }
}

/// Parse a time string in HH:MM:SS format or NIMBY Rails format
///
/// # Errors
///
/// Returns an error if the string cannot be parsed as a valid time.
pub fn parse_time_hms(s: &str) -> Result<NaiveTime, chrono::ParseError> {
    // Try flexible format first
    if let Some((hours, minutes, seconds)) = parse_flexible_time(s) {
        // Validate ranges
        if (0..24).contains(&hours) && (0..60).contains(&minutes) && (0..60).contains(&seconds) {
            // Safe to cast: we just validated the ranges
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            if let Some(time) = NaiveTime::from_hms_opt(hours as u32, minutes as u32, seconds as u32) {
                return Ok(time);
            }
        }
    }

    // Fall back to strict format
    NaiveTime::parse_from_str(s, "%H:%M:%S")
}

/// Format a duration as HH:MM:SS
#[must_use]
pub fn format_duration_hms(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

/// Format an RFC3339 timestamp string to local time using the user's locale
///
/// Uses Intl.DateTimeFormat with the user's locale for proper localized formatting.
/// Falls back to the original string if parsing or formatting fails.
#[must_use]
pub fn format_rfc3339_local(rfc3339: &str) -> String {
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(rfc3339) else {
        return rfc3339.to_string();
    };

    let timestamp_millis = dt.timestamp_millis();
    #[allow(clippy::cast_precision_loss)]
    let js_date = js_sys::Date::new(&JsValue::from_f64(timestamp_millis as f64));

    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"dateStyle".into(), &"medium".into());
    let _ = js_sys::Reflect::set(&options, &"timeStyle".into(), &"short".into());

    let formatter = js_sys::Intl::DateTimeFormat::new(&js_sys::Array::new(), &options);

    // Call the format function with the date
    let format_fn = formatter.format();
    let result = format_fn.call1(&JsValue::NULL, &js_date);

    result
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| rfc3339.to_string())
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
    fn test_parse_time_hms_two_parts_as_minutes_seconds() {
        // With flexible format, two parts are treated as MM:SS
        let result = parse_time_hms("08:30");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 8);
        assert_eq!(time.second(), 30);
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

    #[test]
    fn test_nimby_format_seconds_only() {
        let result = parse_time_hms("45");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 45);
    }

    #[test]
    fn test_nimby_format_minutes_seconds() {
        let result = parse_time_hms("3.30");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 3);
        assert_eq!(time.second(), 30);
    }

    #[test]
    fn test_nimby_format_hours_minutes_seconds() {
        let result = parse_time_hms("5.15.");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 5);
        assert_eq!(time.minute(), 15);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_nimby_format_empty_parts() {
        let result = parse_time_hms("6..");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 6);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_nimby_format_colon_separator() {
        let result = parse_time_hms("1:2:3");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 1);
        assert_eq!(time.minute(), 2);
        assert_eq!(time.second(), 3);
    }

    #[test]
    fn test_nimby_format_comma_separator() {
        let result = parse_time_hms("1,2,3");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 1);
        assert_eq!(time.minute(), 2);
        assert_eq!(time.second(), 3);
    }

    #[test]
    fn test_nimby_format_semicolon_separator() {
        let result = parse_time_hms("1;2;3");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 1);
        assert_eq!(time.minute(), 2);
        assert_eq!(time.second(), 3);
    }

    #[test]
    fn test_nimby_format_full_example() {
        let result = parse_time_hms("05.15.00");
        assert!(result.is_ok());
        let time = result.expect("should parse");
        assert_eq!(time.hour(), 5);
        assert_eq!(time.minute(), 15);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_nimby_format_invalid_range() {
        // 25 hours is invalid
        let result = parse_time_hms("25.0.0");
        assert!(result.is_err());

        // 61 minutes is invalid
        let result = parse_time_hms("1.61.0");
        assert!(result.is_err());

        // 61 seconds is invalid
        let result = parse_time_hms("1.2.61");
        assert!(result.is_err());
    }
}