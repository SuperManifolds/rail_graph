use leptos::{component, view, Signal, IntoView, SignalGet, SignalGetUntracked, event_target_value, create_node_ref, html::Input};
use chrono::{NaiveDateTime, Duration};
use crate::constants::BASE_DATE;
use web_sys::KeyboardEvent;
use std::rc::Rc;

#[component]
#[must_use]
pub fn TimeInput(
    label: &'static str,
    value: Signal<NaiveDateTime>,
    default_time: &'static str,
    on_change: Box<dyn Fn(NaiveDateTime) + 'static>,
    #[prop(optional)] show_next_day_indicator: Option<Signal<bool>>,
) -> impl IntoView {
    let input_ref = create_node_ref::<Input>();
    let on_change = Rc::new(on_change);
    let on_change_clone = on_change.clone();

    view! {
        <label class="time-input-label">
            {label}
            <div class="time-input-wrapper">
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
                    on:keydown=move |ev: KeyboardEvent| {
                        let key = ev.key();
                        let adjustment = if key == "j" {
                            Some(Duration::seconds(-30))
                        } else if key == "l" {
                            Some(Duration::seconds(30))
                        } else {
                            None
                        };

                        if let Some(delta) = adjustment {
                            ev.prevent_default();
                            let current = value.get_untracked();
                            let new_datetime = current + delta;
                            if let Some(input_elem) = input_ref.get() {
                                input_elem.set_value(&new_datetime.format("%H:%M:%S").to_string());
                            }
                            on_change_clone(new_datetime);
                        }
                    }
                />
                {move || {
                    if let Some(show_indicator) = show_next_day_indicator {
                        if show_indicator.get() {
                            Some(view! { <span class="next-day-indicator">"+1"</span> })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }}
            </div>
        </label>
    }
}