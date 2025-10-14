use crate::models::{Line, RailwayGraph, RouteSegment, RouteDirection, Stations, Routes};
use leptos::*;
use chrono::Duration;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
pub enum StationPosition {
    Start,
    End,
}

#[component]
pub fn StationSelect(
    available_stations: Vec<String>,
    station_idx: Option<petgraph::graph::NodeIndex>,
    position: StationPosition,
    route_direction: RouteDirection,
    graph: ReadSignal<RailwayGraph>,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    if available_stations.is_empty() {
        return view! {}.into_view();
    }

    let avail = available_stations.clone();
    let label = match position {
        StationPosition::Start => "+ Add stop at start...",
        StationPosition::End => "+ Add stop at end...",
    };

    view! {
        <div class="add-stop-row">
            <select
                class="station-select"
                on:change={
                    move |ev| {
                        let station_name = event_target_value(&ev);
                        if !station_name.is_empty() {
                            if let Some(mut updated_line) = edited_line.get_untracked() {
                                let graph = graph.get();
                                let new_station_idx = graph.get_station_index(&station_name);

                                // Handle empty route case (first station)
                                if station_idx.is_none() && new_station_idx.is_some() {
                                    // For an empty route, just mark this as the starting point
                                    // The actual segments will be added when the second station is selected
                                    on_save(updated_line);
                                    return;
                                }

                                if let (Some(new_station_idx), Some(existing_idx)) = (
                                    new_station_idx,
                                    station_idx
                                ) {
                                    // Find path based on route direction and position
                                    let path = match (route_direction, position) {
                                        (RouteDirection::Forward, StationPosition::Start)
                                        | (RouteDirection::Return, StationPosition::End) => {
                                            graph.find_path_between_nodes(new_station_idx, existing_idx)
                                        }
                                        (RouteDirection::Forward, StationPosition::End)
                                        | (RouteDirection::Return, StationPosition::Start) => {
                                            graph.find_path_between_nodes(existing_idx, new_station_idx)
                                        }
                                    };

                                    if let Some(path) = path {
                                        // Convert path edges into route segments
                                        for (i, edge) in path.iter().enumerate() {
                                            // Get the source node of this edge
                                            let Some((source, _)) = graph.graph.edge_endpoints(*edge) else {
                                                continue;
                                            };

                                            // Check if station is a passing loop
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
                                                // Only the first segment gets the wait time
                                                // Subsequent segments have zero wait time (no stops at intermediate junctions)
                                                wait_time: if i == 0 { default_wait } else { Duration::zero() },
                                            };

                                            match (route_direction, position) {
                                                (RouteDirection::Forward, StationPosition::Start) => {
                                                    updated_line.forward_route.insert(i, segment);
                                                }
                                                (RouteDirection::Forward, StationPosition::End) => {
                                                    updated_line.forward_route.push(segment);
                                                }
                                                (RouteDirection::Return, StationPosition::Start) => {
                                                    updated_line.return_route.insert(i, segment);
                                                }
                                                (RouteDirection::Return, StationPosition::End) => {
                                                    updated_line.return_route.push(segment);
                                                }
                                            }
                                        }

                                        // Sync return route if editing forward route and sync is enabled
                                        if matches!(route_direction, RouteDirection::Forward) {
                                            updated_line.apply_route_sync_if_enabled();
                                        }

                                        on_save(updated_line);
                                    }
                                }
                            }
                        }
                    }
                }
            >
                <option value="">{label}</option>
                {avail.iter().map(|name| {
                    view! {
                        <option value=name.clone()>{name.clone()}</option>
                    }
                }).collect::<Vec<_>>()}
            </select>
        </div>
    }.into_view()
}
