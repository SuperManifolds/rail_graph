use crate::components::{duration_input::DurationInput, time_input::TimeInput};
use crate::models::{Line, RailwayGraph, RouteDirection};
use crate::constants::BASE_MIDNIGHT;
use super::{PlatformSelect, PlatformField};
use leptos::{component, view, ReadSignal, Props, IntoView, Signal, SignalGetUntracked, event_target_value};
use chrono::Duration;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
pub enum TimeDisplayMode {
    Difference,  // Time between consecutive stops
    Absolute,    // Cumulative time from start
}

#[component]
pub fn StopRow(
    index: usize,
    name: String,
    station_idx: petgraph::graph::NodeIndex,
    line: Line,
    graph: RailwayGraph,
    time_mode: TimeDisplayMode,
    route_direction: RouteDirection,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
    is_first: bool,
    is_last: bool,
) -> impl IntoView {
    let route = match route_direction {
        RouteDirection::Forward => &line.forward_route,
        RouteDirection::Return => &line.return_route,
    };

    let cumulative_seconds: i64 = if index == 0 {
        0
    } else {
        route.iter().take(index).map(|seg| (seg.duration + seg.wait_time).num_seconds()).sum()
    };

    let column_content = match time_mode {
        TimeDisplayMode::Difference => {
            if index < route.len() {
                let segment_duration = route[index].duration;
                let hours = cumulative_seconds / 3600;
                let minutes = (cumulative_seconds % 3600) / 60;
                let seconds = cumulative_seconds % 60;
                let preview_text = format!("(Σ {hours:02}:{minutes:02}:{seconds:02})");

                view! {
                    <div class="time-input-with-preview">
                        <DurationInput
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
                                        on_save(updated_line);
                                    }
                                }
                            }
                        />
                        <span class="cumulative-preview">{preview_text}</span>
                    </div>
                }.into_view()
            } else {
                view! { <span class="travel-time">"-"</span> }.into_view()
            }
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
                                                .map(|seg| (seg.duration + seg.wait_time).num_seconds())
                                                .sum();
                                            let prev_wait_seconds = updated_line.forward_route[index - 1].wait_time.num_seconds();
                                            let segment_duration_seconds = new_cumulative_seconds - prev_cumulative_seconds - prev_wait_seconds;

                                            if segment_duration_seconds >= 0 {
                                                updated_line.forward_route[index - 1].duration = Duration::seconds(segment_duration_seconds);
                                                on_save(updated_line);
                                            }
                                        }
                                        RouteDirection::Return => {
                                            let prev_cumulative_seconds: i64 = updated_line.return_route.iter()
                                                .take(index - 1)
                                                .map(|seg| (seg.duration + seg.wait_time).num_seconds())
                                                .sum();
                                            let prev_wait_seconds = updated_line.return_route[index - 1].wait_time.num_seconds();
                                            let segment_duration_seconds = new_cumulative_seconds - prev_cumulative_seconds - prev_wait_seconds;

                                            if segment_duration_seconds >= 0 {
                                                updated_line.return_route[index - 1].duration = Duration::seconds(segment_duration_seconds);
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
    };

    let wait_time_content = if index > 0 && index - 1 < route.len() {
        let wait_duration = route[index - 1].wait_time;
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
    };

    // Get station platforms
    let platforms = graph.graph.node_weight(station_idx)
        .map(|node| node.platforms.clone())
        .unwrap_or_default();

    // Platform selector - one per station
    // For first stop: use origin_platform of first segment
    // For middle stops: use destination_platform of previous segment (= origin_platform of current segment)
    // For last stop: use destination_platform of last segment
    let platform_content = if is_first && index < route.len() {
        // First stop: departure platform
        let current_platform = route[index].origin_platform;
        view! {
            <PlatformSelect
                platforms=platforms.clone()
                current_platform=current_platform
                index=index
                field=PlatformField::Origin
                route_direction=route_direction
                edited_line=edited_line
                on_save=on_save.clone()
            />
        }.into_view()
    } else if is_last && index > 0 && index - 1 < route.len() {
        // Last stop: arrival platform
        let current_platform = route[index - 1].destination_platform;
        view! {
            <PlatformSelect
                platforms=platforms.clone()
                current_platform=current_platform
                index=index - 1
                field=PlatformField::Destination
                route_direction=route_direction
                edited_line=edited_line
                on_save=on_save.clone()
            />
        }.into_view()
    } else if !is_first && !is_last && index > 0 && index - 1 < route.len() && index < route.len() {
        // Middle stop: platform where train is (update both arrival and departure)
        let current_platform = route[index - 1].destination_platform;
        view! {
            <PlatformSelect
                platforms=platforms.clone()
                current_platform=current_platform
                index=index
                field=PlatformField::Both
                route_direction=route_direction
                edited_line=edited_line
                on_save=on_save.clone()
            />
        }.into_view()
    } else {
        view! { <span class="platform-placeholder">"-"</span> }.into_view()
    };

    // Track selector (for segment leaving this station)
    let track_content = if index < route.len() {
        let current_track = route[index].track_index;
        let edge_idx = petgraph::graph::EdgeIndex::new(route[index].edge_index);

        // Get available tracks for this edge based on direction
        let available_tracks: Vec<(usize, String)> = if let Some(track_segment) = graph.graph.edge_weight(edge_idx) {
            track_segment.tracks.iter().enumerate()
                .filter(|(_, track)| {
                    match route_direction {
                        RouteDirection::Forward => {
                            // Forward route can only use Forward or Bidirectional tracks
                            matches!(track.direction, crate::models::TrackDirection::Forward | crate::models::TrackDirection::Bidirectional)
                        }
                        RouteDirection::Return => {
                            // Return route can only use Backward or Bidirectional tracks
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
        };

        if available_tracks.len() == 1 {
            // Only one track - show as read-only text
            view! {
                <span class="track-info">{available_tracks[0].1.clone()}</span>
            }.into_view()
        } else {
            // Multiple tracks - show selector
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
    } else {
        view! { <span class="track-placeholder">"-"</span> }.into_view()
    };

    let can_delete = (is_first || is_last) && route.len() > 1;

    view! {
        <div class="stop-row">
            <span class="station-name">{name}</span>
            {platform_content}
            {track_content}
            {column_content}
            {wait_time_content}
            {if can_delete {
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
            }}
        </div>
    }
}
