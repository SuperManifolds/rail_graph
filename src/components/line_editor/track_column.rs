use crate::models::{Line, RailwayGraph, RouteDirection};
use leptos::{component, view, ReadSignal, IntoView, SignalGetUntracked, SignalWithUntracked, event_target_value};
use std::rc::Rc;

fn get_available_tracks(
    graph: ReadSignal<RailwayGraph>,
    edge_idx: petgraph::graph::EdgeIndex,
    route_direction: RouteDirection,
) -> Vec<(usize, String)> {
    graph.with_untracked(|g| {
        if let Some(track_segment) = g.graph.edge_weight(edge_idx) {
            track_segment.tracks.iter().enumerate()
                .filter(|(_, track)| {
                    match route_direction {
                        RouteDirection::Forward => {
                            matches!(track.direction, crate::models::TrackDirection::Forward | crate::models::TrackDirection::Bidirectional)
                        }
                        RouteDirection::Return => {
                            matches!(track.direction, crate::models::TrackDirection::Backward | crate::models::TrackDirection::Bidirectional)
                        }
                    }
                })
                .map(|(i, track)| {
                    let direction_str = match track.direction {
                        crate::models::TrackDirection::Bidirectional => "↔",
                        crate::models::TrackDirection::Forward => "→",
                        crate::models::TrackDirection::Backward => "←",
                    };
                    (i, format!("{} {}", i + 1, direction_str))
                })
                .collect()
        } else {
            vec![]
        }
    })
}

fn update_track_index(
    edited_line: ReadSignal<Option<Line>>,
    route_direction: RouteDirection,
    index: usize,
    track_idx: usize,
    on_save: &Rc<dyn Fn(Line)>,
) {
    if let Some(mut updated_line) = edited_line.get_untracked() {
        match route_direction {
            RouteDirection::Forward => {
                if index < updated_line.forward_route.len() {
                    updated_line.forward_route[index].track_index = track_idx;
                }
            }
            RouteDirection::Return => {
                if index < updated_line.return_route.len() {
                    updated_line.return_route[index].track_index = track_idx;
                }
            }
        }
        on_save(updated_line);
    }
}

#[component]
pub fn TrackColumn(
    graph: ReadSignal<RailwayGraph>,
    edge_idx: Option<petgraph::graph::EdgeIndex>,
    current_track: Option<usize>,
    index: usize,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let Some(edge_idx) = edge_idx else {
        return view! { <span class="track-placeholder">"-"</span> }.into_view();
    };

    let Some(current_track) = current_track else {
        return view! { <span class="track-placeholder">"-"</span> }.into_view();
    };

    let available_tracks = get_available_tracks(graph, edge_idx, route_direction);

    if available_tracks.is_empty() {
        return view! { <span class="track-placeholder">"-"</span> }.into_view();
    }

    if available_tracks.len() == 1 {
        view! {
            <span class="track-info">{available_tracks[0].1.clone()}</span>
        }.into_view()
    } else {
        view! {
            <select
                class="track-select"
                on:change={
                    let on_save = on_save.clone();
                    move |ev| {
                        if let Ok(track_idx) = event_target_value(&ev).parse::<usize>() {
                            update_track_index(edited_line, route_direction, index, track_idx, &on_save);
                        }
                    }
                }
            >
                {available_tracks.iter().map(|(i, label)| {
                    view! {
                        <option value=i.to_string() selected=*i == current_track>
                            {label.clone()}
                        </option>
                    }
                }).collect::<Vec<_>>()}
            </select>
        }.into_view()
    }
}
