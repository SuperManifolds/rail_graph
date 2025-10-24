use crate::components::{days_of_week_selector::DaysOfWeekSelector, time_input::TimeInput, duration_input::OptionalDurationInput};
use crate::models::{ManualDeparture, RailwayGraph, Stations, DaysOfWeek};
use leptos::{component, view, IntoView, create_signal, store_value, Signal, SignalGet, SignalUpdate, SignalGetUntracked, event_target_value};
use crate::constants::BASE_DATE;

#[component]
#[allow(clippy::needless_pass_by_value)]
pub fn ManualDepartureEditor(
    index: usize,
    #[prop(into)] departure: ManualDeparture,
    station_names: Vec<String>,
    graph: RailwayGraph,
    on_update: impl Fn(usize, ManualDeparture) + 'static,
    on_remove: impl Fn(usize) + 'static,
) -> impl IntoView {
    let (local_departure, set_local_departure) = create_signal(departure.clone());

    let on_update = store_value(on_update);
    let on_remove = store_value(on_remove);

    view! {
        <div class="manual-departure-item">
            <div class="departure-time-row">
                <TimeInput
                    label=""
                    value=Signal::derive(move || local_departure.get().time)
                    default_time="08:00"
                    on_change={
                        Box::new(move |time| {
                            set_local_departure.update(|dep| dep.time = time);
                            on_update.with_value(|f| f(index, local_departure.get_untracked()));
                        })
                    }
                />
            <select
                class="station-input"
                on:change={
                    let graph = graph.clone();
                    move |ev| {
                        let station_name = event_target_value(&ev);
                        if let Some(node_idx) = graph.get_station_index(&station_name) {
                            set_local_departure.update(|dep| dep.from_station = node_idx);
                            on_update.with_value(|f| f(index, local_departure.get_untracked()));
                        }
                    }
                }
            >
                {
                    let graph = graph.clone();
                    station_names.iter().map(|name| {
                        let name_clone = name.clone();
                        let graph_clone = graph.clone();
                        view! {
                            <option
                                value=name.clone()
                                selected=move || {
                                    graph_clone.get_station_name(local_departure.get().from_station)
                                        .is_some_and(|n| n == name_clone.as_str())
                                }
                            >
                                {name.clone()}
                            </option>
                        }
                    }).collect::<Vec<_>>()
                }
            </select>
            <span class="arrow">"→"</span>
            <select
                class="station-input"
                on:change={
                    let graph = graph.clone();
                    move |ev| {
                        let station_name = event_target_value(&ev);
                        if let Some(node_idx) = graph.get_station_index(&station_name) {
                            set_local_departure.update(|dep| dep.to_station = node_idx);
                            on_update.with_value(|f| f(index, local_departure.get_untracked()));
                        }
                    }
                }
            >
                {
                    station_names.iter().map(|name| {
                        let name_clone = name.clone();
                        let graph_clone = graph.clone();
                        view! {
                            <option
                                value=name.clone()
                                selected=move || {
                                    graph_clone.get_station_name(local_departure.get().to_station)
                                        .is_some_and(|n| n == name_clone.as_str())
                                }
                            >
                                {name.clone()}
                            </option>
                        }
                    }).collect::<Vec<_>>()
                }
            </select>
            <button
                class="remove-departure"
                on:click=move |_| on_remove.with_value(|f| f(index))
            >
                "×"
            </button>
            <DaysOfWeekSelector
                days_of_week=Signal::derive(move || local_departure.get().days_of_week)
                set_days_of_week=move |days: DaysOfWeek| {
                    set_local_departure.update(|dep| dep.days_of_week = days);
                    on_update.with_value(|f| f(index, local_departure.get_untracked()));
                }
            />
            </div>
            <div class="train-repeat-row">
                <div class="form-group">
                    <label>"Train Number (optional)"</label>
                    <input
                        type="text"
                        class="train-number-input"
                        placeholder="Auto-generated if empty"
                        value=move || local_departure.get().train_number.unwrap_or_default()
                        on:input=move |ev| {
                            let value = event_target_value(&ev);
                            let train_number = if value.is_empty() { None } else { Some(value) };
                            set_local_departure.update(|dep| dep.train_number = train_number);
                            on_update.with_value(|f| f(index, local_departure.get_untracked()));
                        }
                    />
                </div>
                <div class="form-group">
                    <label>"Repeat every"</label>
                    <OptionalDurationInput
                        duration=Signal::derive(move || local_departure.get().repeat_interval)
                        on_change=move |duration| {
                            set_local_departure.update(|dep| dep.repeat_interval = duration);
                            on_update.with_value(|f| f(index, local_departure.get_untracked()));
                        }
                    />
                </div>
                <div class="form-group">
                    <label>"Until (optional)"</label>
                    <input
                        type="time"
                        class="time-input"
                        step="1"
                        lang="en-GB"
                        placeholder="End of day"
                        prop:value=move || {
                            local_departure.get().repeat_until
                                .map_or(String::new(), |dt| dt.format("%H:%M:%S").to_string())
                        }
                        on:input=move |ev| {
                            let time_str = event_target_value(&ev);
                            let repeat_until = if time_str.is_empty() {
                                None
                            } else if let Ok(naive_time) = crate::time::parse_time_hms(&time_str) {
                                Some(BASE_DATE.and_time(naive_time))
                            } else {
                                local_departure.get_untracked().repeat_until
                            };
                            set_local_departure.update(|dep| dep.repeat_until = repeat_until);
                            on_update.with_value(|f| f(index, local_departure.get_untracked()));
                        }
                    />
                </div>
            </div>
        </div>
    }
}
