use leptos::{component, view, Signal, IntoView, SignalGet, event_target_value};
use chrono::NaiveDateTime;
use crate::constants::BASE_DATE;

#[component]
#[must_use]
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
                class="time-input"
                step="1"
                prop:value=move || value.get().format("%H:%M:%S").to_string()
                placeholder=default_time
                on:input=move |ev| {
                    let time_str = event_target_value(&ev);
                    if let Ok(naive_time) = crate::time::parse_time_hms(&time_str) {
                        let new_datetime = BASE_DATE.and_time(naive_time);
                        on_change(new_datetime);
                    }
                }
            />
        </label>
    }
}