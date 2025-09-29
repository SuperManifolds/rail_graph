use leptos::*;
use chrono::Duration;

fn duration_to_hhmmss(duration: Duration) -> String {
    let total_seconds = duration.num_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

fn parse_hhmmss(input: &str) -> Option<Duration> {
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
pub fn FrequencyInput(
    frequency: Signal<Duration>,
    on_change: impl Fn(Duration) + 'static,
) -> impl IntoView {
    view! {
        <label>
            "Frequency: "
            <input
                type="text"
                placeholder="00:30:00"
                value={duration_to_hhmmss(frequency.get_untracked())}
                on:change=move |ev| {
                    let input_str = event_target_value(&ev);
                    if let Some(new_frequency) = parse_hhmmss(&input_str) {
                        on_change(new_frequency);
                    }
                }
                style="font-family: monospace; width: 100px;"
            />
        </label>
    }
}