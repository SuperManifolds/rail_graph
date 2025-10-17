use super::ManualDepartureEditor;
use crate::constants::DEFAULT_DEPARTURE_TIME;
use crate::models::{Line, ManualDeparture, RailwayGraph, Stations, Tracks};
use leptos::{component, view, ReadSignal, WriteSignal, IntoView, SignalGet, SignalGetUntracked, SignalSet, For, Signal, store_value};
use std::rc::Rc;

#[component]
pub fn ManualDeparturesList(
    edited_line: ReadSignal<Option<Line>>,
    set_edited_line: WriteSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let on_save_stored = store_value(on_save.clone());

    // Derive station names from current line
    let station_names = Signal::derive(move || {
        let Some(line) = edited_line.get() else {
            return Vec::new();
        };

        let current_graph = graph.get();
        let mut names = Vec::new();

        // Add first station
        if let Some(segment) = line.forward_route.first() {
            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
            let Some((from, _)) = current_graph.get_track_endpoints(edge_idx) else {
                return names;
            };
            if let Some(name) = current_graph.get_station_name(from) {
                names.push(name.to_string());
            }
        }

        // Add all destination stations
        for segment in &line.forward_route {
            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
            let Some((_, to)) = current_graph.get_track_endpoints(edge_idx) else {
                continue;
            };
            if let Some(name) = current_graph.get_station_name(to) {
                names.push(name.to_string());
            }
        }

        names
    });

    view! {
        <div class="form-group">
            <label>"Manual Departures"</label>
            <div class="manual-departures-list">
                <For
                    each=move || {
                        edited_line.get()
                            .map(|line| line.manual_departures.iter().enumerate()
                                .map(|(idx, dep)| (idx, dep.clone()))
                                .collect::<Vec<_>>())
                            .unwrap_or_default()
                    }
                    key=|item| item.1.id
                    children=move |(idx, dep)| {
                        let on_save = on_save_stored.get_value();
                        let current_graph = graph.get();
                        let current_station_names = station_names.get();

                        view! {
                            <ManualDepartureEditor
                                index=idx
                                departure=dep
                                station_names=current_station_names
                                graph=current_graph
                                on_update={
                                    let on_save = on_save.clone();
                                    move |idx, updated_dep| {
                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                            if let Some(departure) = updated_line.manual_departures.get_mut(idx) {
                                                *departure = updated_dep;
                                            }
                                            set_edited_line.set(Some(updated_line.clone()));
                                            on_save(updated_line);
                                        }
                                    }
                                }
                                on_remove={
                                    move |idx| {
                                        if let Some(mut updated_line) = edited_line.get_untracked() {
                                            updated_line.manual_departures.remove(idx);
                                            set_edited_line.set(Some(updated_line.clone()));
                                            on_save(updated_line);
                                        }
                                    }
                                }
                            />
                        }
                    }
                />
            </div>
            <button
                class="add-departure"
                on:click={
                    let on_save = on_save.clone();
                    move |_| {
                        if let Some(mut updated_line) = edited_line.get_untracked() {
                            let current_graph = graph.get();

                            // Get first and last station NodeIndex from forward route
                            let from_station = updated_line.forward_route.first()
                                .and_then(|segment| {
                                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                    current_graph.get_track_endpoints(edge_idx).map(|(from, _)| from)
                                })
                                .unwrap_or_else(|| petgraph::graph::NodeIndex::new(0));

                            let to_station = updated_line.forward_route.last()
                                .and_then(|segment| {
                                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                    current_graph.get_track_endpoints(edge_idx).map(|(_, to)| to)
                                })
                                .unwrap_or_else(|| petgraph::graph::NodeIndex::new(1));

                            let new_departure = ManualDeparture {
                                id: uuid::Uuid::new_v4(),
                                time: DEFAULT_DEPARTURE_TIME,
                                from_station,
                                to_station,
                                days_of_week: crate::models::DaysOfWeek::ALL_DAYS,
                                train_number: None,
                            };
                            updated_line.manual_departures.push(new_departure);
                            set_edited_line.set(Some(updated_line.clone()));
                            on_save(updated_line);
                        }
                    }
                }
            >
                "+ Add Departure"
            </button>
        </div>
    }
}
