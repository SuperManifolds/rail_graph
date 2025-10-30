use super::{PlatformColumn, TimeColumn, TimeDisplayMode, TrackColumn, WaitTimeColumn};
use crate::models::{Line, RailwayGraph, RouteDirection, RouteSegment};
use chrono::Duration;
use leptos::{
    component, create_memo, view, IntoView, ReadSignal, SignalGetUntracked, SignalWith,
    SignalWithUntracked,
};
use std::rc::Rc;

pub(super) fn calculate_cumulative_seconds(
    display_durations: &[Option<Duration>],
    route: &[RouteSegment],
    index: usize,
) -> i64 {
    if index == 0 {
        return 0;
    }

    (0..index)
        .map(|i| {
            let duration = display_durations
                .get(i)
                .copied()
                .flatten()
                .unwrap_or(Duration::zero());
            let wait_time = route.get(i).map_or(Duration::zero(), |s| s.wait_time);
            (duration + wait_time).num_seconds()
        })
        .sum()
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

    // Check if this stop is a junction (junctions should not have editable wait times)
    let is_junction = graph.with_untracked(|g| {
        g.graph
            .node_weight(station_idx)
            .is_some_and(|node| node.as_junction().is_some())
    });

    // Separate structural data (rarely changes) from duration data (changes often)
    // This allows Leptos to skip re-rendering structural elements when only durations change
    #[allow(clippy::items_after_statements)]
    type StructData = (Option<usize>, Option<usize>, Option<petgraph::graph::EdgeIndex>, Option<usize>, Option<usize>);

    #[allow(clippy::excessive_nesting)]
    let struct_data: leptos::Memo<Option<StructData>> = create_memo(move |_| {
        edited_line.with(|line| {
            line.as_ref().and_then(|l| {
                let route = match route_direction {
                    RouteDirection::Forward => &l.forward_route,
                    RouteDirection::Return => &l.return_route,
                };

                if index < route.len() {
                    let segment = &route[index];
                    let prev_dest_platform = if index > 0 && index - 1 < route.len() {
                        Some(route[index - 1].destination_platform)
                    } else {
                        None
                    };

                    Some((
                        Some(segment.origin_platform),
                        prev_dest_platform,
                        Some(petgraph::graph::EdgeIndex::new(segment.edge_index)),
                        Some(segment.track_index),
                        Some(route.len()),
                    ))
                } else {
                    None
                }
            })
        })
    });

    view! {
        <div class="stop-row">
            <span class="station-name">{name.clone()}</span>
            {move || {
                struct_data.with(|struct_opt| {
                    struct_opt.as_ref().map(|(current_platform_origin, current_platform_dest, edge_idx, current_track, route_len)| {
                        view! {
                            <>
                                <PlatformColumn
                                    platforms=platforms.clone()
                                    current_platform_origin=*current_platform_origin
                                    current_platform_dest=*current_platform_dest
                                    index=index
                                    is_first=is_first
                                    is_last=is_last
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                                <TrackColumn
                                    graph=graph
                                    edge_idx=*edge_idx
                                    current_track=*current_track
                                    index=index
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                                <TimeColumn
                                    time_mode=time_mode
                                    index=index
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                />
                                <WaitTimeColumn
                                    index=index
                                    route_direction=route_direction
                                    edited_line=edited_line
                                    on_save=on_save.clone()
                                    is_junction=is_junction
                                    is_first=is_first
                                />
                                <DeleteButton
                                    is_first=is_first
                                    is_last=is_last
                                    route_len=route_len.unwrap_or(0)
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
