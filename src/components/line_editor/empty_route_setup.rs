use crate::models::{Line, RailwayGraph, RouteDirection, RouteSegment, Stations, Routes, Tracks};
use leptos::*;
use chrono::Duration;

#[component]
fn NoStationsMessage() -> impl IntoView {
    view! {
        <p class="no-stops">"No stations defined. Create stations in the Infrastructure tab first."</p>
    }
}

#[component]
fn FirstStationSelect(
    all_stations: Vec<String>,
    first_station: RwSignal<Option<String>>,
) -> impl IntoView {
    view! {
        <div class="empty-route-setup">
            <p class="no-stops">"No stops defined for this route yet. Select first stop:"</p>
            <select
                class="station-select"
                on:change=move |ev| {
                    let station_name = event_target_value(&ev);
                    if !station_name.is_empty() {
                        first_station.set(Some(station_name));
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
    }
}

fn create_route_between_stations(
    first_name: &str,
    second_name: &str,
    mut line: Line,
    graph: &RailwayGraph,
    direction: RouteDirection,
) -> Option<Line> {
    let first_idx = graph.get_station_index(first_name)?;
    let second_idx = graph.get_station_index(second_name)?;
    let path = graph.find_path_between_nodes(first_idx, second_idx)?;

    for edge in &path {
        let Some((source, target)) = graph.graph.edge_endpoints(*edge) else {
            continue;
        };

        let is_passing_loop = graph.graph.node_weight(source)
            .and_then(|node| node.as_station())
            .is_some_and(|s| s.passing_loop);
        let default_wait = if is_passing_loop {
            Duration::seconds(0)
        } else {
            line.default_wait_time
        };

        let source_platform_count = graph.graph.node_weight(source)
            .and_then(|n| n.as_station())
            .map_or(1, |s| s.platforms.len());

        let target_platform_count = graph.graph.node_weight(target)
            .and_then(|n| n.as_station())
            .map_or(1, |s| s.platforms.len());

        let origin_platform = graph.get_default_platform_for_arrival(*edge, false, source_platform_count);
        let destination_platform = graph.get_default_platform_for_arrival(*edge, true, target_platform_count);

        let segment = RouteSegment {
            edge_index: edge.index(),
            track_index: 0,
            origin_platform,
            destination_platform,
            duration: None,
            // Use default wait time for stations, zero for passing loops
            wait_time: default_wait,
        };

        match direction {
            RouteDirection::Forward => {
                line.forward_route.push(segment);
            }
            RouteDirection::Return => {
                line.return_route.push(segment);
            }
        }
    }

    if matches!(direction, RouteDirection::Forward) {
        line.apply_route_sync_if_enabled();
    }

    Some(line)
}

#[component]
#[allow(clippy::too_many_arguments)]
fn SecondStationSelect(
    first_name: String,
    all_stations: Vec<String>,
    first_station: RwSignal<Option<String>>,
    route_direction: RwSignal<RouteDirection>,
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let other_stations: Vec<String> = all_stations.iter()
        .filter(|name| *name != &first_name)
        .cloned()
        .collect();

    let first_name_for_handler = first_name.clone();
    let handle_selection = move |ev| {
        let second_name = event_target_value(&ev);
        if second_name.is_empty() {
            return;
        }

        let Some(line) = edited_line.get_untracked() else { return };
        let graph_data = graph.get();
        let direction = route_direction.get();

        if let Some(updated_line) = create_route_between_stations(&first_name_for_handler, &second_name, line, &graph_data, direction) {
            on_save(updated_line);
            first_station.set(None);
        }
    };

    view! {
        <div class="empty-route-setup">
            <p class="no-stops">"First stop: " {first_name.clone()} ". Select destination:"</p>
            <select
                class="station-select"
                on:change=handle_selection
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
                on:click=move |_| first_station.set(None)
            >
                "Cancel"
            </button>
        </div>
    }
}

#[component]
pub fn EmptyRouteSetup(
    first_station: RwSignal<Option<String>>,
    route_direction: RwSignal<RouteDirection>,
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    // Extract data before the view
    let all_stations = create_memo(move |_| {
        graph.with_untracked(RailwayGraph::get_all_station_names)
    });

    view! {
        {move || {
            let stations = all_stations.get();

            if stations.is_empty() {
                view! { <NoStationsMessage /> }.into_view()
            } else {
                let first_selected = first_station.get();

                if let Some(first_name) = first_selected {
                    view! {
                        <SecondStationSelect
                            first_name=first_name
                            all_stations=stations
                            first_station=first_station
                            route_direction=route_direction
                            edited_line=edited_line
                            graph=graph
                            on_save=on_save.clone()
                        />
                    }.into_view()
                } else {
                    view! {
                        <FirstStationSelect
                            all_stations=stations
                            first_station=first_station
                        />
                    }.into_view()
                }
            }
        }}
    }
}
