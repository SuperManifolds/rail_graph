use leptos::{component, view, Signal, IntoView, SignalGet, SignalGetUntracked, event_target_value, create_node_ref, html::Input};
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
    let input_ref = create_node_ref::<Input>();

    view! {
        <label>
            {label}
            <input
                type="text"
                class="time-input"
                prop:value=move || value.get().format("%H:%M:%S").to_string()
                placeholder=default_time
                node_ref=input_ref
                on:change=move |ev| {
                    let time_str = event_target_value(&ev);
                    if let Ok(naive_time) = crate::time::parse_time_hms(&time_str) {
                        let new_datetime = BASE_DATE.and_time(naive_time);
                        on_change(new_datetime);
                    } else {
                        // Reset to last valid value if parsing fails
                        if let Some(input_elem) = input_ref.get() {
                            input_elem.set_value(&value.get_untracked().format("%H:%M:%S").to_string());
                        }
                    }
                }
            />
        </label>
    }
}