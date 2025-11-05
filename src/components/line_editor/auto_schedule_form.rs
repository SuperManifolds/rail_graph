use crate::components::{
    days_of_week_selector::DaysOfWeekSelector,
    duration_input::DurationInput,
    time_input::TimeInput,
};
use crate::models::{Line, DaysOfWeek};
use leptos::{component, view, IntoView, Signal, SignalGet, event_target_value, SignalGetUntracked, Callback, Callable};

#[component]
#[allow(clippy::too_many_lines)]
pub fn AutoScheduleForm(
    edited_line: Signal<Option<Line>>,
    on_update: Callback<Line>,
) -> impl IntoView {
    view! {
        <div class="form-group">
            <label>"Train Number Format"</label>
            <input
                type="text"
                class="train-number-format-input"
                placeholder="e.g., {line} {seq:04}"
                value=move || edited_line.get().map(|l| l.auto_train_number_format).unwrap_or_default()
                on:input=move |ev| {
                    let format = event_target_value(&ev);
                    if let Some(mut updated_line) = edited_line.get_untracked() {
                        updated_line.auto_train_number_format = format;
                        on_update.call(updated_line);
                    }
                }
            />
            <small class="help-text">"Format: {line} for line ID, {seq:04} for sequence number"</small>
        </div>

        <div class="form-group">
            <label>"Operating days"</label>
            <DaysOfWeekSelector
                days_of_week=Signal::derive(move || edited_line.get().map(|l| l.days_of_week).unwrap_or_default())
                set_days_of_week=move |days: DaysOfWeek| {
                    if let Some(mut updated_line) = edited_line.get_untracked() {
                        updated_line.days_of_week = days;
                        on_update.call(updated_line);
                    }
                }
            />
        </div>

        <div class="form-group">
            <label>"Frequency"</label>
            <DurationInput
                duration=Signal::derive(move || edited_line.get().map(|l| l.frequency).unwrap_or_default())
                on_change=move |freq| {
                    if let Some(mut updated_line) = edited_line.get_untracked() {
                        updated_line.frequency = freq;
                        on_update.call(updated_line);
                    }
                }
            />
        </div>

        <div class="time-fields-row">
            <div class="form-group">
                <label>"First Departure"</label>
                <TimeInput
                    label=""
                    value=Signal::derive(move || edited_line.get().map(|l| l.first_departure).unwrap_or_default())
                    default_time="05:00"
                    on_change=Box::new(move |new_time| {
                        if let Some(mut updated_line) = edited_line.get_untracked() {
                            if updated_line.sync_departure_offsets {
                                let offset = new_time.signed_duration_since(updated_line.first_departure);
                                updated_line.first_departure = new_time;
                                updated_line.return_first_departure += offset;
                            } else {
                                updated_line.first_departure = new_time;
                            }
                            on_update.call(updated_line);
                        }
                    })
                />
            </div>

            <button
                class=move || if edited_line.get().is_some_and(|l| l.sync_departure_offsets) {
                    "sync-toggle active"
                } else {
                    "sync-toggle"
                }
                on:click=move |_| {
                    if let Some(mut updated_line) = edited_line.get_untracked() {
                        updated_line.sync_departure_offsets = !updated_line.sync_departure_offsets;
                        on_update.call(updated_line);
                    }
                }
                title=move || if edited_line.get().is_some_and(|l| l.sync_departure_offsets) {
                    "Unlock departure offset (times change independently)"
                } else {
                    "Lock departure offset (times change together)"
                }
            >
                <i class=move || if edited_line.get().is_some_and(|l| l.sync_departure_offsets) {
                    "fa-solid fa-lock"
                } else {
                    "fa-solid fa-unlock"
                }></i>
            </button>

            <div class="form-group">
                <label>"Return First Departure"</label>
                <TimeInput
                    label=""
                    value=Signal::derive(move || edited_line.get().map(|l| l.return_first_departure).unwrap_or_default())
                    default_time="06:00"
                    on_change=Box::new(move |new_time| {
                        if let Some(mut updated_line) = edited_line.get_untracked() {
                            if updated_line.sync_departure_offsets {
                                let offset = new_time.signed_duration_since(updated_line.return_first_departure);
                                updated_line.return_first_departure = new_time;
                                updated_line.first_departure += offset;
                            } else {
                                updated_line.return_first_departure = new_time;
                            }
                            on_update.call(updated_line);
                        }
                    })
                />
            </div>
        </div>

        <div class="time-fields-row">
            <div class="form-group">
                <label>"Last Departure Before"</label>
                <TimeInput
                    label=""
                    value=Signal::derive(move || edited_line.get().map(|l| l.last_departure).unwrap_or_default())
                    default_time="22:00"
                    on_change=Box::new(move |new_time| {
                        if let Some(mut updated_line) = edited_line.get_untracked() {
                            if updated_line.sync_departure_offsets {
                                let offset = new_time.signed_duration_since(updated_line.last_departure);
                                updated_line.last_departure = new_time;
                                updated_line.return_last_departure += offset;
                            } else {
                                updated_line.last_departure = new_time;
                            }
                            on_update.call(updated_line);
                        }
                    })
                    show_next_day_indicator=Signal::derive(move || {
                        edited_line.get().is_some_and(|l| l.last_departure.time() < l.first_departure.time())
                    })
                />
            </div>

            <div class="sync-toggle-placeholder"></div>

            <div class="form-group">
                <label>"Return Last Departure Before"</label>
                <TimeInput
                    label=""
                    value=Signal::derive(move || edited_line.get().map(|l| l.return_last_departure).unwrap_or_default())
                    default_time="22:00"
                    on_change=Box::new(move |new_time| {
                        if let Some(mut updated_line) = edited_line.get_untracked() {
                            if updated_line.sync_departure_offsets {
                                let offset = new_time.signed_duration_since(updated_line.return_last_departure);
                                updated_line.return_last_departure = new_time;
                                updated_line.last_departure += offset;
                            } else {
                                updated_line.return_last_departure = new_time;
                            }
                            on_update.call(updated_line);
                        }
                    })
                    show_next_day_indicator=Signal::derive(move || {
                        edited_line.get().is_some_and(|l| l.return_last_departure.time() < l.return_first_departure.time())
                    })
                />
            </div>
        </div>
    }
}
