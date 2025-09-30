use leptos::*;
use chrono::NaiveDateTime;
use crate::constants::BASE_DATE;

#[component]
pub fn TimeInput(
    label: &'static str,
    value: Signal<NaiveDateTime>,
    default_time: &'static str,
    on_change: Box<dyn Fn(NaiveDateTime) + 'static>,
) -> impl IntoView {
    view! {
        <label>
            {label}
            <input
                type="time"
                prop:value=move || value.get().format("%H:%M").to_string()
                placeholder=default_time
                on:input=move |ev| {
                    let time_str = event_target_value(&ev);
                    if let Ok(naive_time) = chrono::NaiveTime::parse_from_str(&format!("{}:00", time_str), "%H:%M:%S") {
                        let new_datetime = BASE_DATE.and_time(naive_time);
                        on_change(new_datetime);
                    }
                }
            />
        </label>
    }
}