use leptos::{component, event_target_value, IntoView, Signal, SignalGet, SignalGetUntracked, view};
use chrono::Duration;

fn duration_to_hhmmss(duration: Duration) -> String {
    let total_seconds = duration.num_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn parse_hhmmss(input: &str) -> Option<Duration> {
    // Try flexible format (NIMBY Rails format)
    if let Some((hours, minutes, seconds)) = crate::time::parse_flexible_time(input) {
        // For durations, we allow any non-negative values (no 24-hour or 60-minute limit)
        if hours >= 0 && minutes >= 0 && seconds >= 0 {
            return Some(Duration::hours(hours) + Duration::minutes(minutes) + Duration::seconds(seconds));
        }
    }

    // Fall back to strict format
    let parts: Vec<&str> = input.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;
    let seconds: i64 = parts[2].parse().ok()?;

    if hours < 0 || !(0..60).contains(&minutes) || !(0..60).contains(&seconds) {
        return None;
    }

    Some(Duration::hours(hours) + Duration::minutes(minutes) + Duration::seconds(seconds))
}

#[component]
pub fn DurationInput(
    duration: Signal<Duration>,
    on_change: impl Fn(Duration) + 'static,
) -> impl IntoView {
    use leptos::html::Input;
    use leptos::create_node_ref;

    let input_ref = create_node_ref::<Input>();

    view! {
        <input
            type="text"
            class="duration-input"
            placeholder="00:30:00"
            prop:value=move || duration_to_hhmmss(duration.get())
            node_ref=input_ref
            on:change=move |ev| {
                let input_str = event_target_value(&ev);
                if let Some(new_duration) = parse_hhmmss(&input_str) {
                    on_change(new_duration);
                } else {
                    // Reset to last valid value if parsing fails
                    if let Some(input_elem) = input_ref.get() {
                        input_elem.set_value(&duration_to_hhmmss(duration.get_untracked()));
                    }
                }
            }
        />
    }
}

#[component]
pub fn OptionalDurationInput(
    duration: Signal<Option<Duration>>,
    on_change: impl Fn(Option<Duration>) + 'static,
) -> impl IntoView {
    use leptos::html::Input;
    use leptos::create_node_ref;

    let input_ref = create_node_ref::<Input>();

    view! {
        <input
            type="text"
            class="duration-input"
            placeholder="-"
            prop:value=move || {
                duration.get().map_or(String::new(), duration_to_hhmmss)
            }
            node_ref=input_ref
            on:change=move |ev| {
                let input_str = event_target_value(&ev).trim().to_string();
                if input_str.is_empty() || input_str == "-" {
                    on_change(None);
                } else if let Some(new_duration) = parse_hhmmss(&input_str) {
                    on_change(Some(new_duration));
                } else {
                    // Reset to last valid value if parsing fails
                    if let Some(input_elem) = input_ref.get() {
                        let valid_value = duration.get_untracked().map_or(String::new(), duration_to_hhmmss);
                        input_elem.set_value(&valid_value);
                    }
                }
            }
        />
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_large_hours() {
        // Durations should accept any non-negative hours
        let result = parse_hhmmss("26.0.0");
        assert!(result.is_some());
        let duration = result.expect("should parse");
        assert_eq!(duration.num_hours(), 26);
    }

    #[test]
    fn test_parse_duration_large_minutes() {
        // Durations should accept any non-negative minutes
        let result = parse_hhmmss("0.70.0");
        assert!(result.is_some());
        let duration = result.expect("should parse");
        assert_eq!(duration.num_minutes(), 70);
    }

    #[test]
    fn test_parse_duration_large_seconds() {
        // Durations should accept any non-negative seconds
        let result = parse_hhmmss("0.0.90");
        assert!(result.is_some());
        let duration = result.expect("should parse");
        assert_eq!(duration.num_seconds(), 90);
    }

    #[test]
    fn test_parse_duration_nimby_format() {
        let result = parse_hhmmss("5.15.");
        assert!(result.is_some());
        let duration = result.expect("should parse");
        assert_eq!(duration.num_hours(), 5);
        assert_eq!(duration.num_minutes(), 5 * 60 + 15);
    }

    #[test]
    fn test_parse_duration_standard_format() {
        let result = parse_hhmmss("01:30:45");
        assert!(result.is_some());
        let duration = result.expect("should parse");
        assert_eq!(duration.num_seconds(), 3600 + 30 * 60 + 45);
    }

    #[test]
    fn test_duration_to_hhmmss() {
        let duration = Duration::hours(2) + Duration::minutes(15) + Duration::seconds(30);
        assert_eq!(duration_to_hhmmss(duration), "02:15:30");
    }
}