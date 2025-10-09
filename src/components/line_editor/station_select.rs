use crate::models::{Line, RailwayGraph, RouteSegment, RouteDirection};
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
                                if let (Some(new_station_idx), Some(existing_idx)) = (
                                    graph.get_station_index(&station_name),
                                    station_idx
                                ) {
                                    // Find edge based on route direction and position
                                    let edge = match (route_direction, position) {
                                        (RouteDirection::Forward, StationPosition::Start) => {
                                            graph.graph.find_edge(new_station_idx, existing_idx)
                                        }
                                        (RouteDirection::Forward, StationPosition::End) => {
                                            graph.graph.find_edge(existing_idx, new_station_idx)
                                        }
                                        (RouteDirection::Return, StationPosition::Start) => {
                                            graph.graph.find_edge(existing_idx, new_station_idx)
                                        }
                                        (RouteDirection::Return, StationPosition::End) => {
                                            graph.graph.find_edge(new_station_idx, existing_idx)
                                        }
                                    };

                                    if let Some(edge) = edge {
                                        // Check if station is a passing loop
                                        let is_passing_loop = graph.graph.node_weight(new_station_idx)
                                            .map(|node| node.passing_loop)
                                            .unwrap_or(false);
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
                                            wait_time: default_wait,
                                        };

                                        match (route_direction, position) {
                                            (RouteDirection::Forward, StationPosition::Start) => {
                                                updated_line.forward_route.insert(0, segment);
                                            }
                                            (RouteDirection::Forward, StationPosition::End) => {
                                                updated_line.forward_route.push(segment);
                                            }
                                            (RouteDirection::Return, StationPosition::Start) => {
                                                updated_line.return_route.insert(0, segment);
                                            }
                                            (RouteDirection::Return, StationPosition::End) => {
                                                updated_line.return_route.push(segment);
                                            }
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
