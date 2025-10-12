use crate::components::{days_of_week_selector::DaysOfWeekSelector, time_input::TimeInput};
use crate::models::{ManualDeparture, RailwayGraph, Stations, DaysOfWeek};
use leptos::{component, view, IntoView, create_signal, store_value, Signal, SignalGet, SignalUpdate, SignalGetUntracked, event_target_value};

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
        </div>
    }
}
