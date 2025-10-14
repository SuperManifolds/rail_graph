use crate::components::tab_view::TabPanel;
use crate::models::{Line, RailwayGraph, RouteDirection, Routes, Stations, RouteSegment};
use super::{StopRow, TimeDisplayMode, StationSelect, StationPosition};
use leptos::*;
use chrono::Duration;

#[component]
#[allow(clippy::too_many_lines)]
pub fn StopsTab(
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    active_tab: RwSignal<String>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let (time_mode, set_time_mode) = create_signal(TimeDisplayMode::Difference);
    let (route_direction, set_route_direction) = create_signal(RouteDirection::Forward);
    let (first_station, set_first_station) = create_signal(None::<String>);
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "stops")>
            <div class="line-editor-content">
                <div class="stops-controls">
                    <button
                        class="route-direction-toggle"
                        on:click=move |_| {
                            set_route_direction.update(|dir| {
                                *dir = match *dir {
                                    RouteDirection::Forward => RouteDirection::Return,
                                    RouteDirection::Return => RouteDirection::Forward,
                                };
                            });
                        }
                        title=move || match route_direction.get() {
                            RouteDirection::Forward => "Switch to return route",
                            RouteDirection::Return => "Switch to forward route",
                        }
                    >
                        {move || match route_direction.get() {
                            RouteDirection::Forward => "→ Forward",
                            RouteDirection::Return => "← Return",
                        }}
                    </button>
                    <button
                        class="time-mode-toggle"
                        on:click=move |_| {
                            set_time_mode.update(|mode| {
                                *mode = match *mode {
                                    TimeDisplayMode::Difference => TimeDisplayMode::Absolute,
                                    TimeDisplayMode::Absolute => TimeDisplayMode::Difference,
                                };
                            });
                        }
                        title=move || match time_mode.get() {
                            TimeDisplayMode::Difference => "Switch to cumulative time from start",
                            TimeDisplayMode::Absolute => "Switch to time to next stop",
                        }
                    >
                        {move || match time_mode.get() {
                            TimeDisplayMode::Difference => "Δt",
                            TimeDisplayMode::Absolute => "Σt",
                        }}
                    </button>
                    <span class="time-mode-label">
                        {move || match time_mode.get() {
                            TimeDisplayMode::Difference => "Time to next stop",
                            TimeDisplayMode::Absolute => "Cumulative time from start",
                        }}
                    </span>
                </div>
                <div class="stops-list">
                    {move || {
                        edited_line.get().map(|line| {
                            let current_graph = graph.get();

                            let current_route = match route_direction.get() {
                                RouteDirection::Forward => &line.forward_route,
                                RouteDirection::Return => &line.return_route,
                            };

                            if current_route.is_empty() {
                                let all_stations = current_graph.get_all_station_names();

                                if all_stations.is_empty() {
                                    view! {
                                        <p class="no-stops">"No stations defined. Create stations in the Infrastructure tab first."</p>
                                    }.into_view()
                                } else {
                                    let first_selected = first_station.get();

                                    if let Some(first_name) = first_selected {
                                        // First station selected, now show all other stations
                                        let other_stations: Vec<String> = all_stations.iter()
                                            .filter(|name| *name != &first_name)
                                            .cloned()
                                            .collect();

                                        view! {
                                            <div class="empty-route-setup">
                                                <p class="no-stops">"First stop: " {first_name.clone()} ". Select destination:"</p>
                                                <select
                                                    class="station-select"
                                                    on:change={
                                                        let on_save = on_save.clone();
                                                        let first_name = first_name.clone();
                                                        move |ev| {
                                                            let second_name = event_target_value(&ev);
                                                            if !second_name.is_empty() {
                                                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                                                    let graph = graph.get();
                                                                    if let (Some(first_idx), Some(second_idx)) = (
                                                                        graph.get_station_index(&first_name),
                                                                        graph.get_station_index(&second_name)
                                                                    ) {
                                                                        // Use pathfinding to get all edges between stations
                                                                        if let Some(path) = graph.find_path_between_nodes(first_idx, second_idx) {
                                                                            // Add all segments in the path
                                                                            for (i, edge) in path.iter().enumerate() {
                                                                                // Get the source node of this edge
                                                                                let Some((source, _)) = graph.graph.edge_endpoints(*edge) else {
                                                                                    continue;
                                                                                };

                                                                                let is_passing_loop = graph.graph.node_weight(source)
                                                                                    .and_then(|node| node.as_station())
                                                                                    .is_some_and(|s| s.passing_loop);
                                                                                let default_wait = if is_passing_loop {
                                                                                    Duration::seconds(0)
                                                                                } else {
                                                                                    Duration::seconds(30)
                                                                                };

                                                                                let segment = RouteSegment {
                                                                                    edge_index: edge.index(),
                                                                                    track_index: 0,
                                                                                    origin_platform: 0,
                                                                                    destination_platform: 0,
                                                                                    duration: Duration::minutes(5),
                                                                                    // Only the first segment gets the wait time, representing the dwell time at the origin station.
                                                                                    // Subsequent segments have zero wait time, as trains do not stop at intermediate segments.
                                                                                    wait_time: if i == 0 { default_wait } else { Duration::zero() },
                                                                                };

                                                                                match route_direction.get() {
                                                                                    RouteDirection::Forward => {
                                                                                        updated_line.forward_route.push(segment);
                                                                                    }
                                                                                    RouteDirection::Return => {
                                                                                        updated_line.return_route.push(segment);
                                                                                    }
                                                                                }
                                                                            }

                                                                            // Sync return route if editing forward route and sync is enabled
                                                                            if matches!(route_direction.get(), RouteDirection::Forward) {
                                                                                updated_line.apply_route_sync_if_enabled();
                                                                            }

                                                                            on_save(updated_line);
                                                                            set_first_station.set(None);
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                >
                                                    <option value="">{"Select destination..."}</option>
                                                    {other_stations.iter().map(|name| {
                                                        view! {
                                                            <option value=name.clone()>{name.clone()}</option>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </select>
                                                <button
                                                    class="cancel-button"
                                                    on:click=move |_| set_first_station.set(None)
                                                >
                                                    "Cancel"
                                                </button>
                                            </div>
                                        }.into_view()
                                    } else {
                                        // No station selected yet, show first station dropdown
                                        view! {
                                            <div class="empty-route-setup">
                                                <p class="no-stops">"No stops defined for this route yet. Select first stop:"</p>
                                                <select
                                                    class="station-select"
                                                    on:change=move |ev| {
                                                        let station_name = event_target_value(&ev);
                                                        if !station_name.is_empty() {
                                                            set_first_station.set(Some(station_name));
                                                        }
                                                    }
                                                >
                                                    <option value="">{"Select first stop..."}</option>
                                                    {all_stations.iter().map(|name| {
                                                        view! {
                                                            <option value=name.clone()>{name.clone()}</option>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </select>
                                            </div>
                                        }.into_view()
                                    }
                                }
                            } else {
                                let stations = current_graph.get_stations_from_route(current_route, route_direction.get());

                                let mode = time_mode.get();
                                let dir = route_direction.get();
                                let column_header = match mode {
                                    TimeDisplayMode::Difference => "Travel Time to Next",
                                    TimeDisplayMode::Absolute => "Time from Start",
                                };

                                let (first_station_idx, last_station_idx) = current_graph.get_route_endpoints(current_route, dir);
                                let available_start = current_graph.get_available_start_stations(current_route, dir);
                                let available_end = current_graph.get_available_end_stations(current_route, dir);

                                view! {
                                    <div class="stops-header">
                                        <span>"Station"</span>
                                        <span>"Platform"</span>
                                        <span>"Track"</span>
                                        <span>{column_header}</span>
                                        <span>"Wait Time"</span>
                                        <span></span>
                                    </div>

                                    <StationSelect
                                        available_stations=available_start
                                        station_idx=first_station_idx
                                        position=StationPosition::Start
                                        route_direction=route_direction.get()
                                        graph=graph
                                        edited_line=edited_line
                                        on_save=on_save.clone()
                                    />

                                    {
                                        stations.iter().enumerate().map(|(i, (name, station_idx))| {
                                            let num_stations = stations.len();
                                            view! {
                                                <StopRow
                                                    index=i
                                                    name=name.clone()
                                                    station_idx=*station_idx
                                                    line=line.clone()
                                                    graph=current_graph.clone()
                                                    time_mode=mode
                                                    route_direction=dir
                                                    edited_line=edited_line
                                                    on_save=on_save.clone()
                                                    is_first={i == 0}
                                                    is_last={i == num_stations - 1}
                                                />
                                            }
                                        }).collect::<Vec<_>>()
                                    }

                                    <StationSelect
                                        available_stations=available_end
                                        station_idx=last_station_idx
                                        position=StationPosition::End
                                        route_direction=route_direction.get()
                                        graph=graph
                                        edited_line=edited_line
                                        on_save=on_save.clone()
                                    />
                                }.into_view()
                            }
                        })
                    }}
                </div>
            </div>
        </TabPanel>
    }
}
