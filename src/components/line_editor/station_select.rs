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
                                        (RouteDirection::Forward | RouteDirection::Return, StationPosition::Start) => {
                                            graph.find_path_between_nodes(new_station_idx, existing_idx)
                                        }
                                        (RouteDirection::Forward | RouteDirection::Return, StationPosition::End) => {
                                            graph.find_path_between_nodes(existing_idx, new_station_idx)
                                        }
                                    };

                                    if let Some(path) = path {
                                        // Determine starting node for path traversal
                                        let mut current_node = match (route_direction, position) {
                                            (RouteDirection::Forward | RouteDirection::Return, StationPosition::Start) => new_station_idx,
                                            (RouteDirection::Forward | RouteDirection::Return, StationPosition::End) => existing_idx,
                                        };

                                        // Convert path edges into route segments
                                        for (i, edge) in path.iter().enumerate() {
                                            // Get the endpoints of this edge
                                            let Some((source, target)) = graph.graph.edge_endpoints(*edge) else {
                                                continue;
                                            };

                                            // Determine direction: are we going source→target (forward) or target→source (backward)?
                                            let is_forward = current_node == source;
                                            let next_node = if is_forward { target } else { source };

                                            // Get track segment to determine compatible track
                                            let track_segment = graph.graph.edge_weight(*edge);
                                            let track_index = Line::find_compatible_track(
                                                track_segment,
                                                is_forward,
                                                track_segment.map_or(0, |ts| ts.tracks.len().saturating_sub(1))
                                            );

                                            // Check if station is a passing loop or if node is a junction
                                            let is_passing_loop_or_junction = graph.graph.node_weight(current_node)
                                                .is_some_and(|node| {
                                                    node.as_station().is_some_and(|s| s.passing_loop) ||
                                                    node.as_junction().is_some()
                                                });
                                            let default_wait = if is_passing_loop_or_junction {
                                                Duration::seconds(0)
                                            } else {
                                                updated_line.default_wait_time
                                            };

                                            let segment = RouteSegment {
                                                edge_index: edge.index(),
                                                track_index,
                                                origin_platform: 0,
                                                destination_platform: 0,
                                                duration: None,
                                                // Use default wait time for stations, zero for passing loops and junctions
                                                wait_time: default_wait,
                                            };

                                            // Update current node for next iteration
                                            current_node = next_node;

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
