use crate::components::{duration_input::{DurationInput, OptionalDurationInput}, time_input::TimeInput};
use crate::models::{Line, RailwayGraph, RouteDirection};
use crate::constants::BASE_MIDNIGHT;
use super::{PlatformSelect, PlatformField};
use leptos::{component, view, ReadSignal, IntoView, Signal, SignalGetUntracked, SignalWithUntracked, SignalWith, create_memo, event_target_value};
use chrono::Duration;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
pub enum TimeDisplayMode {
    Difference,  // Time between consecutive stops
    Absolute,    // Cumulative time from start
}

fn render_time_column_from_data(
    time_mode: TimeDisplayMode,
    index: usize,
    segment_duration: Option<Duration>,
    cumulative_seconds: i64,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> leptos::View {
    match time_mode {
        TimeDisplayMode::Difference => {
            let hours = cumulative_seconds / 3600;
            let minutes = (cumulative_seconds % 3600) / 60;
            let seconds = cumulative_seconds % 60;
            let preview_text = format!("(Σ {hours:02}:{minutes:02}:{seconds:02})");

            view! {
                <div class="time-input-with-preview">
                    <OptionalDurationInput
                        duration=Signal::derive(move || segment_duration)
                        on_change={
                            let on_save = on_save.clone();
                            move |new_duration| {
                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                    match route_direction {
                                        RouteDirection::Forward => {
                                            if index < updated_line.forward_route.len() {
                                                updated_line.forward_route[index].duration = new_duration;
                                            }
                                        }
                                        RouteDirection::Return => {
                                            if index < updated_line.return_route.len() {
                                                updated_line.return_route[index].duration = new_duration;
                                            }
                                        }
                                    }

                                    if matches!(route_direction, RouteDirection::Forward) {
                                        updated_line.apply_route_sync_if_enabled();
                                    }

                                    on_save(updated_line);
                                }
                            }
                        }
                    />
                    <span class="cumulative-preview">{preview_text}</span>
                </div>
            }.into_view()
        }
        TimeDisplayMode::Absolute => {
            if index > 0 {
                let cumulative_time = BASE_MIDNIGHT + Duration::seconds(cumulative_seconds);
                view! {
                    <TimeInput
                        label=""
                        value=Signal::derive(move || cumulative_time)
                        default_time="00:00:00"
                        on_change={
                            let on_save = on_save.clone();
                            Box::new(move |new_time| {
                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                    let new_cumulative_seconds = (new_time - BASE_MIDNIGHT).num_seconds();

                                    match route_direction {
                                        RouteDirection::Forward => {
                                            let prev_cumulative_seconds: i64 = updated_line.forward_route.iter()
                                                .take(index - 1)
                                                .map(|seg| (seg.duration.unwrap_or(Duration::zero()) + seg.wait_time).num_seconds())
                                                .sum();
                                            let prev_wait_seconds = updated_line.forward_route[index - 1].wait_time.num_seconds();
                                            let segment_duration_seconds = new_cumulative_seconds - prev_cumulative_seconds - prev_wait_seconds;

                                            if segment_duration_seconds >= 0 {
                                                updated_line.forward_route[index - 1].duration = Some(Duration::seconds(segment_duration_seconds));
                                                updated_line.apply_route_sync_if_enabled();
                                                on_save(updated_line);
                                            }
                                        }
                                        RouteDirection::Return => {
                                            let prev_cumulative_seconds: i64 = updated_line.return_route.iter()
                                                .take(index - 1)
                                                .map(|seg| (seg.duration.unwrap_or(Duration::zero()) + seg.wait_time).num_seconds())
                                                .sum();
                                            let prev_wait_seconds = updated_line.return_route[index - 1].wait_time.num_seconds();
                                            let segment_duration_seconds = new_cumulative_seconds - prev_cumulative_seconds - prev_wait_seconds;

                                            if segment_duration_seconds >= 0 {
                                                updated_line.return_route[index - 1].duration = Some(Duration::seconds(segment_duration_seconds));
                                                on_save(updated_line);
                                            }
                                        }
                                    }
                                }
                            })
                        }
                    />
                }.into_view()
            } else {
                view! { <span class="travel-time">"00:00:00"</span> }.into_view()
            }
        }
    }
}

fn render_wait_time_from_data(
    index: usize,
    wait_duration: Duration,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> leptos::View {
    if index > 0 {
        view! {
            <DurationInput
                duration=Signal::derive(move || wait_duration)
                on_change={
                    let on_save = on_save.clone();
                    move |new_wait_time| {
                        if let Some(mut updated_line) = edited_line.get_untracked() {
                            match route_direction {
                                RouteDirection::Forward => {
                                    if index > 0 && index - 1 < updated_line.forward_route.len() {
                                        updated_line.forward_route[index - 1].wait_time = new_wait_time;
                                    }
                                }
                                RouteDirection::Return => {
                                    if index > 0 && index - 1 < updated_line.return_route.len() {
                                        updated_line.return_route[index - 1].wait_time = new_wait_time;
                                    }
                                }
                            }
                            on_save(updated_line);
                        }
                    }
                }
            />
        }.into_view()
    } else {
        view! { <span class="travel-time">"-"</span> }.into_view()
    }
}

#[allow(clippy::too_many_arguments)]
fn render_platform_column(
    platforms: Vec<crate::models::Platform>,
    current_platform_origin: Option<usize>,
    current_platform_dest: Option<usize>,
    index: usize,
    is_first: bool,
    is_last: bool,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> leptos::View {
    if platforms.is_empty() {
        return view! { <span class="platform-placeholder">"-"</span> }.into_view();
    }

    if is_first {
        if let Some(current_platform) = current_platform_origin {
            return view! {
                <PlatformSelect
                    platforms=platforms
                    current_platform=current_platform
                    index=index
                    field=PlatformField::Origin
                    route_direction=route_direction
                    edited_line=edited_line
                    on_save=on_save
                />
            }.into_view();
        }
    } else if is_last {
        if let Some(current_platform) = current_platform_dest {
            return view! {
                <PlatformSelect
                    platforms=platforms
                    current_platform=current_platform
                    index=index - 1
                    field=PlatformField::Destination
                    route_direction=route_direction
                    edited_line=edited_line
                    on_save=on_save
                />
            }.into_view();
        }
    } else if !is_first && !is_last {
        if let Some(current_platform) = current_platform_dest {
            return view! {
                <PlatformSelect
                    platforms=platforms
                    current_platform=current_platform
                    index=index
                    field=PlatformField::Both
                    route_direction=route_direction
                    edited_line=edited_line
                    on_save=on_save
                />
            }.into_view();
        }
    }

    view! { <span class="platform-placeholder">"-"</span> }.into_view()
}

fn render_track_column(
    graph: ReadSignal<RailwayGraph>,
    edge_idx: Option<petgraph::graph::EdgeIndex>,
    current_track: Option<usize>,
    index: usize,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> leptos::View {
    let Some(edge_idx) = edge_idx else {
        return view! { <span class="track-placeholder">"-"</span> }.into_view();
    };

    let Some(current_track) = current_track else {
        return view! { <span class="track-placeholder">"-"</span> }.into_view();
    };

    let available_tracks: Vec<(usize, String)> = graph.with_untracked(|g| {
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
    });

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

fn render_delete_button(
    is_first: bool,
    is_last: bool,
    route_len: usize,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> leptos::View {
    let can_delete = (is_first || is_last) && route_len > 1;

    if can_delete {
        view! {
            <button
                class="delete-stop-button"
                on:click={
                    let on_save = on_save.clone();
                    move |_| {
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

                            // Sync return route if editing forward route and sync is enabled
                            if matches!(route_direction, RouteDirection::Forward) {
                                updated_line.apply_route_sync_if_enabled();
                            }

                            on_save(updated_line);
                        }
                    }
                }
                title=if is_first { "Remove first stop" } else { "Remove last stop" }
            >
                <i class="fa-solid fa-circle-minus"></i>
            </button>
        }.into_view()
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
        g.graph.node_weight(station_idx)
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
                    route.iter()
                        .take(index)
                        .map(|seg| (seg.duration.unwrap_or(Duration::zero()) + seg.wait_time).num_seconds())
                        .sum()
                };

                (segment, prev_segment, cumulative_seconds, route.len())
            })
        })
    });

    view! {
        <div class="stop-row">
            <span class="station-name">{name.clone()}</span>
            {move || {
                route_data.with(|data| {
                    data.as_ref().map(|(segment, prev_segment, cumulative_seconds, route_len)| {
                        let segment_duration = segment.as_ref().and_then(|s| s.duration);
                        let wait_duration = prev_segment.as_ref().map_or(Duration::zero(), |s| s.wait_time);
                        let current_platform_origin = segment.as_ref().map(|s| s.origin_platform);
                        let current_platform_dest = prev_segment.as_ref().map(|s| s.destination_platform);
                        let current_track = segment.as_ref().map(|s| s.track_index);
                        let edge_idx = segment.as_ref().map(|s| petgraph::graph::EdgeIndex::new(s.edge_index));

                        view! {
                            <>
                                {render_platform_column(
                                    platforms.clone(),
                                    current_platform_origin,
                                    current_platform_dest,
                                    index,
                                    is_first,
                                    is_last,
                                    route_direction,
                                    edited_line,
                                    on_save.clone()
                                )}
                                {render_track_column(
                                    graph,
                                    edge_idx,
                                    current_track,
                                    index,
                                    route_direction,
                                    edited_line,
                                    on_save.clone()
                                )}
                                {render_time_column_from_data(
                                    time_mode,
                                    index,
                                    segment_duration,
                                    *cumulative_seconds,
                                    route_direction,
                                    edited_line,
                                    on_save.clone()
                                )}
                                {render_wait_time_from_data(
                                    index,
                                    wait_duration,
                                    route_direction,
                                    edited_line,
                                    on_save.clone()
                                )}
                                {render_delete_button(
                                    is_first,
                                    is_last,
                                    *route_len,
                                    route_direction,
                                    edited_line,
                                    on_save.clone()
                                )}
                            </>
                        }
                    })
                })
            }}
        </div>
    }
}
