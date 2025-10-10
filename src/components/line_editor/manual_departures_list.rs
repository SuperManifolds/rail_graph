use super::ManualDepartureEditor;
use crate::constants::DEFAULT_DEPARTURE_TIME;
use crate::models::{Line, ManualDeparture, RailwayGraph, Stations, Tracks};
use leptos::{component, view, ReadSignal, WriteSignal, IntoView, SignalGet, SignalGetUntracked, SignalSet};
use std::rc::Rc;

#[component]
pub fn ManualDeparturesList(
    edited_line: ReadSignal<Option<Line>>,
    set_edited_line: WriteSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    view! {
        <div class="form-group">
            <label>"Manual Departures"</label>
            <div class="manual-departures-list">
                {
                    let on_save = on_save.clone();
                    move || {
                        edited_line.get().map(|line| {
                            let current_graph = graph.get();

                            // Build list of station names from forward route
                            let mut station_names = Vec::new();

                            if let Some(segment) = line.forward_route.first() {
                                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                if let Some((from, _)) = current_graph.get_track_endpoints(edge_idx) {
                                    if let Some(name) = current_graph.get_station_name(from) {
                                        station_names.push(name.to_string());
                                    }
                                }
                            }

                            for segment in &line.forward_route {
                                let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                if let Some((_, to)) = current_graph.get_track_endpoints(edge_idx) {
                                    if let Some(name) = current_graph.get_station_name(to) {
                                        station_names.push(name.to_string());
                                    }
                                }
                            }

                            line.manual_departures.iter().enumerate().map(|(idx, dep)| {
                                let on_save = on_save.clone();
                                let station_names = station_names.clone();
                                let current_graph = current_graph.clone();
                                view! {
                                    <ManualDepartureEditor
                                        index=idx
                                        departure=dep.clone()
                                        station_names=station_names
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
                        }).collect::<Vec<_>>()
                    }).unwrap_or_default()
                    }
                }
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
                                time: DEFAULT_DEPARTURE_TIME,
                                from_station,
                                to_station,
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
