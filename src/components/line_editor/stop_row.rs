use super::{PlatformColumn, TimeColumn, TimeDisplayMode, TrackColumn, WaitTimeColumn};
use crate::models::{Line, RailwayGraph, RouteDirection, RouteSegment};
use chrono::Duration;
use leptos::{
    component, create_memo, view, IntoView, ReadSignal, SignalGetUntracked, SignalWith,
    SignalWithUntracked,
};
use std::rc::Rc;

fn segment_total_seconds(seg: &RouteSegment) -> i64 {
    (seg.duration.unwrap_or(Duration::zero()) + seg.wait_time).num_seconds()
}

fn delete_stop(
    edited_line: ReadSignal<Option<Line>>,
    route_direction: RouteDirection,
    is_first: bool,
    is_last: bool,
    on_save: &Rc<dyn Fn(Line)>,
) {
    if let Some(mut updated_line) = edited_line.get_untracked() {
        match route_direction {
            RouteDirection::Forward => {
                if is_first && !updated_line.forward_route.is_empty() {
                    updated_line.forward_route.remove(0);
                } else if is_last && !updated_line.forward_route.is_empty() {
                    updated_line.forward_route.pop();
                }
            }
            RouteDirection::Return => {
                if is_first && !updated_line.return_route.is_empty() {
                    updated_line.return_route.remove(0);
                } else if is_last && !updated_line.return_route.is_empty() {
                    updated_line.return_route.pop();
                }
            }
        }

        if matches!(route_direction, RouteDirection::Forward) {
            updated_line.apply_route_sync_if_enabled();
        }

        on_save(updated_line);
    }
}

#[component]
fn DeleteButton(
    is_first: bool,
    is_last: bool,
    route_len: usize,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let can_delete = (is_first || is_last) && route_len > 1;

    if can_delete {
        view! {
            <button
                class="delete-stop-button"
                on:click={
                    let on_save = on_save.clone();
                    move |_| {
                        delete_stop(edited_line, route_direction, is_first, is_last, &on_save);
                    }
                }
                title=if is_first { "Remove first stop" } else { "Remove last stop" }
            >
                <i class="fa-solid fa-circle-minus"></i>
            </button>
        }
        .into_view()
    } else {
        view! { <span></span> }.into_view()
    }
}

#[component]
pub fn StopRow(
    index: usize,
    name: String,
    station_idx: petgraph::graph::NodeIndex,
    time_mode: TimeDisplayMode,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    on_save: Rc<dyn Fn(Line)>,
    is_first: bool,
    is_last: bool,
) -> impl IntoView {
    // Extract platforms once (graph structure doesn't change reactively)
    let platforms = graph.with_untracked(|g| {
        g.graph
            .node_weight(station_idx)
            .and_then(|node| node.as_station().map(|s| s.platforms.clone()))
            .unwrap_or_default()
    });

    // Create memos for row-specific data to minimize re-renders
    let route_data = create_memo(move |_| {
        edited_line.with(|line| {
            line.as_ref().map(|l| {
                let route = match route_direction {
                    RouteDirection::Forward => &l.forward_route,
                    RouteDirection::Return => &l.return_route,
                };

                let first_stop_wait = match route_direction {
                    RouteDirection::Forward => l.first_stop_wait_time,
                    RouteDirection::Return => l.return_first_stop_wait_time,
                };

                let segment = if index < route.len() {
                    Some(route[index].clone())
                } else {
                    None
                };

                let prev_segment = if index > 0 && index - 1 < route.len() {
                    Some(route[index - 1].clone())
                } else {
                    None
                };

                let cumulative_seconds: i64 = if index == 0 {
                    0
                } else {
                    route.iter().take(index).map(segment_total_seconds).sum()
                };

                (segment, prev_segment, cumulative_seconds, route.len(), first_stop_wait)
            })
        })
    });


    view! {
        <div class="stop-row">
            <span class="station-name">{name.clone()}</span>
            {move || {
                route_data.with(|data| {
                    data.as_ref().map(|(segment, prev_segment, cumulative_seconds, route_len, first_stop_wait)| {
                        let segment_duration = segment.as_ref().and_then(|s| s.duration);
                        let wait_duration = if is_first {
                            *first_stop_wait
                        } else {
                            prev_segment.as_ref().map_or(Duration::zero(), |s| s.wait_time)
                        };
                        let current_platform_origin = segment.as_ref().map(|s| s.origin_platform);
                        let current_platform_dest = prev_segment.as_ref().map(|s| s.destination_platform);
                        let current_track = segment.as_ref().map(|s| s.track_index);
                        let edge_idx = segment.as_ref().map(|s| petgraph::graph::EdgeIndex::new(s.edge_index));

                        view! {
                            <>
                                <PlatformColumn
                                    platforms=platforms.clone()
                                    current_platform_origin=current_platform_origin
                                    current_platform_dest=current_platform_dest
                                    index=index
                                    is_first=is_first
                                    is_last=is_last
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                                <TrackColumn
                                    graph=graph
                                    edge_idx=edge_idx
                                    current_track=current_track
                                    index=index
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                                <TimeColumn
                                    time_mode=time_mode
                                    index=index
                                    segment_duration=segment_duration
                                    cumulative_seconds=*cumulative_seconds
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                                <WaitTimeColumn
                                    index=index
                                    wait_duration=wait_duration
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                                <DeleteButton
                                    is_first=is_first
                                    is_last=is_last
                                    route_len=*route_len
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                            </>
                        }
                    })
                })
            }}
        </div>
    }
}
