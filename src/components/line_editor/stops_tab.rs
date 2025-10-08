use crate::components::{duration_input::DurationInput, tab_view::TabPanel, time_input::TimeInput};
use crate::models::{Line, RailwayGraph};
use crate::constants::BASE_MIDNIGHT;
use leptos::*;
use chrono::Duration;
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq)]
enum TimeDisplayMode {
    Difference,  // Time between consecutive stops
    Absolute,    // Cumulative time from start
}

#[derive(Clone, Copy, PartialEq)]
enum RouteDirection {
    Forward,
    Return,
}

#[component]
fn StopRow(
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
                let preview_text = format!("(Σ {:02}:{:02}:{:02})", hours, minutes, seconds);

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
            <select
                class="platform-select"
                on:change={
                    let on_save = on_save.clone();
                    move |ev| {
                        if let Ok(platform_idx) = event_target_value(&ev).parse::<usize>() {
                            if let Some(mut updated_line) = edited_line.get_untracked() {
                                match route_direction {
                                    RouteDirection::Forward => {
                                        if index < updated_line.forward_route.len() {
                                            updated_line.forward_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                    RouteDirection::Return => {
                                        if index < updated_line.return_route.len() {
                                            updated_line.return_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                }
                                on_save(updated_line);
                            }
                        }
                    }
                }
            >
                {platforms.iter().enumerate().map(|(i, platform)| {
                    view! {
                        <option value=i.to_string() selected=i == current_platform>
                            {platform.name.clone()}
                        </option>
                    }
                }).collect::<Vec<_>>()}
            </select>
        }.into_view()
    } else if is_last && index > 0 && index - 1 < route.len() {
        // Last stop: arrival platform
        let current_platform = route[index - 1].destination_platform;
        view! {
            <select
                class="platform-select"
                on:change={
                    let on_save = on_save.clone();
                    move |ev| {
                        if let Ok(platform_idx) = event_target_value(&ev).parse::<usize>() {
                            if let Some(mut updated_line) = edited_line.get_untracked() {
                                match route_direction {
                                    RouteDirection::Forward => {
                                        if index > 0 && index - 1 < updated_line.forward_route.len() {
                                            updated_line.forward_route[index - 1].destination_platform = platform_idx;
                                        }
                                    }
                                    RouteDirection::Return => {
                                        if index > 0 && index - 1 < updated_line.return_route.len() {
                                            updated_line.return_route[index - 1].destination_platform = platform_idx;
                                        }
                                    }
                                }
                                on_save(updated_line);
                            }
                        }
                    }
                }
            >
                {platforms.iter().enumerate().map(|(i, platform)| {
                    view! {
                        <option value=i.to_string() selected=i == current_platform>
                            {platform.name.clone()}
                        </option>
                    }
                }).collect::<Vec<_>>()}
            </select>
        }.into_view()
    } else if !is_first && !is_last && index > 0 && index - 1 < route.len() && index < route.len() {
        // Middle stop: platform where train is (update both arrival and departure)
        let current_platform = route[index - 1].destination_platform;
        view! {
            <select
                class="platform-select"
                on:change={
                    let on_save = on_save.clone();
                    move |ev| {
                        if let Ok(platform_idx) = event_target_value(&ev).parse::<usize>() {
                            if let Some(mut updated_line) = edited_line.get_untracked() {
                                match route_direction {
                                    RouteDirection::Forward => {
                                        // Update both arrival (destination of previous) and departure (origin of current)
                                        if index > 0 && index - 1 < updated_line.forward_route.len() {
                                            updated_line.forward_route[index - 1].destination_platform = platform_idx;
                                        }
                                        if index < updated_line.forward_route.len() {
                                            updated_line.forward_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                    RouteDirection::Return => {
                                        // Update both arrival (destination of previous) and departure (origin of current)
                                        if index > 0 && index - 1 < updated_line.return_route.len() {
                                            updated_line.return_route[index - 1].destination_platform = platform_idx;
                                        }
                                        if index < updated_line.return_route.len() {
                                            updated_line.return_route[index].origin_platform = platform_idx;
                                        }
                                    }
                                }
                                on_save(updated_line);
                            }
                        }
                    }
                }
            >
                {platforms.iter().enumerate().map(|(i, platform)| {
                    view! {
                        <option value=i.to_string() selected=i == current_platform>
                            {platform.name.clone()}
                        </option>
                    }
                }).collect::<Vec<_>>()}
            </select>
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

#[component]
pub fn StopsTab(
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    active_tab: RwSignal<String>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let (time_mode, set_time_mode) = create_signal(TimeDisplayMode::Difference);
    let (route_direction, set_route_direction) = create_signal(RouteDirection::Forward);
    let on_save_add = store_value(on_save.clone());
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
                                view! {
                                    <p class="no-stops">"No stops defined for this route yet. Import a CSV to set up the route."</p>
                                }.into_view()
                            } else {
                                // Build list of stations (name, NodeIndex) from route
                                let mut stations: Vec<(String, petgraph::graph::NodeIndex)> = Vec::new();

                                match route_direction.get() {
                                    RouteDirection::Forward => {
                                        // Forward: extract from -> to for each edge
                                        if let Some(segment) = current_route.first() {
                                            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                            if let Some((from, _)) = current_graph.get_track_endpoints(edge_idx) {
                                                if let Some(name) = current_graph.get_station_name(from) {
                                                    stations.push((name.to_string(), from));
                                                }
                                            }
                                        }

                                        for segment in current_route {
                                            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                            if let Some((_, to)) = current_graph.get_track_endpoints(edge_idx) {
                                                if let Some(name) = current_graph.get_station_name(to) {
                                                    stations.push((name.to_string(), to));
                                                }
                                            }
                                        }
                                    }
                                    RouteDirection::Return => {
                                        // Return: extract to -> from for each edge (traveling backwards)
                                        if let Some(segment) = current_route.first() {
                                            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                            if let Some((_, to)) = current_graph.get_track_endpoints(edge_idx) {
                                                if let Some(name) = current_graph.get_station_name(to) {
                                                    stations.push((name.to_string(), to));
                                                }
                                            }
                                        }

                                        for segment in current_route {
                                            let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                            if let Some((from, _)) = current_graph.get_track_endpoints(edge_idx) {
                                                if let Some(name) = current_graph.get_station_name(from) {
                                                    stations.push((name.to_string(), from));
                                                }
                                            }
                                        }
                                    }
                                }

                                let mode = time_mode.get();
                                let dir = route_direction.get();
                                let column_header = match mode {
                                    TimeDisplayMode::Difference => "Travel Time to Next",
                                    TimeDisplayMode::Absolute => "Time from Start",
                                };

                                // Get first and last stations for add stop functionality
                                // Account for route direction: return routes travel backwards along edges
                                let (first_station_idx, last_station_idx) = match route_direction.get() {
                                    RouteDirection::Forward => {
                                        let first = current_route.first()
                                            .and_then(|seg| {
                                                let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                                                current_graph.get_track_endpoints(edge).map(|(from, _)| from)
                                            });
                                        let last = current_route.last()
                                            .and_then(|seg| {
                                                let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                                                current_graph.get_track_endpoints(edge).map(|(_, to)| to)
                                            });
                                        (first, last)
                                    }
                                    RouteDirection::Return => {
                                        // Return route segments travel backwards on edges
                                        // First segment's 'to' is the starting station
                                        let first = current_route.first()
                                            .and_then(|seg| {
                                                let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                                                current_graph.get_track_endpoints(edge).map(|(_, to)| to)
                                            });
                                        // Last segment's 'from' is the ending station
                                        let last = current_route.last()
                                            .and_then(|seg| {
                                                let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                                                current_graph.get_track_endpoints(edge).map(|(from, _)| from)
                                            });
                                        (first, last)
                                    }
                                };

                                // Get available stations for start
                                let available_start: Vec<String> = first_station_idx
                                    .map(|first_idx| {
                                        current_graph.get_all_stations_ordered()
                                            .iter()
                                            .filter_map(|station| {
                                                let station_idx = current_graph.get_station_index(&station.name)?;
                                                // For forward: find edge from station_idx to first_idx
                                                // For return: find edge from first_idx to station_idx (traveling backwards)
                                                let has_edge = match route_direction.get() {
                                                    RouteDirection::Forward => current_graph.graph.find_edge(station_idx, first_idx).is_some(),
                                                    RouteDirection::Return => current_graph.graph.find_edge(first_idx, station_idx).is_some(),
                                                };
                                                if has_edge {
                                                    Some(station.name.clone())
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                // Get available stations for end
                                let available_end: Vec<String> = last_station_idx
                                    .map(|last_idx| {
                                        current_graph.get_all_stations_ordered()
                                            .iter()
                                            .filter_map(|station| {
                                                let station_idx = current_graph.get_station_index(&station.name)?;
                                                // For forward: find edge from last_idx to station_idx
                                                // For return: find edge from station_idx to last_idx (traveling backwards)
                                                let has_edge = match route_direction.get() {
                                                    RouteDirection::Forward => current_graph.graph.find_edge(last_idx, station_idx).is_some(),
                                                    RouteDirection::Return => current_graph.graph.find_edge(station_idx, last_idx).is_some(),
                                                };
                                                if has_edge {
                                                    Some(station.name.clone())
                                                } else {
                                                    None
                                                }
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default();

                                view! {
                                    <div class="stops-header">
                                        <span>"Station"</span>
                                        <span>"Platform"</span>
                                        <span>"Track"</span>
                                        <span>{column_header}</span>
                                        <span>"Wait Time"</span>
                                        <span></span>
                                    </div>

                                    {if !available_start.is_empty() {
                                        let avail = available_start.clone();
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
                                                                    if let (Some(station_idx), Some(first_idx)) = (
                                                                        graph.get_station_index(&station_name),
                                                                        first_station_idx
                                                                    ) {
                                                                        // Find edge based on route direction
                                                                        let edge = match route_direction.get() {
                                                                            RouteDirection::Forward => graph.graph.find_edge(station_idx, first_idx),
                                                                            RouteDirection::Return => graph.graph.find_edge(first_idx, station_idx),
                                                                        };

                                                                        if let Some(edge) = edge {
                                                                            // Check if station is a passing loop
                                                                            let is_passing_loop = graph.graph.node_weight(station_idx)
                                                                                .map(|node| node.passing_loop)
                                                                                .unwrap_or(false);
                                                                            let default_wait = if is_passing_loop {
                                                                                Duration::seconds(0)
                                                                            } else {
                                                                                Duration::seconds(30)
                                                                            };

                                                                            let segment = crate::models::RouteSegment {
                                                                                edge_index: edge.index(),
                                                                                track_index: 0,
                                                                                origin_platform: 0,
                                                                                destination_platform: 0,
                                                                                duration: Duration::minutes(5),
                                                                                wait_time: default_wait,
                                                                            };

                                                                            match route_direction.get() {
                                                                                RouteDirection::Forward => {
                                                                                    updated_line.forward_route.insert(0, segment);
                                                                                }
                                                                                RouteDirection::Return => {
                                                                                    updated_line.return_route.insert(0, segment);
                                                                                }
                                                                            }
                                                                            on_save_add.with_value(|f| f(updated_line));
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                >
                                                    <option value="">"+ Add stop at start..."</option>
                                                    {avail.iter().map(|name| {
                                                        view! {
                                                            <option value=name.clone()>{name.clone()}</option>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </select>
                                            </div>
                                        }.into_view()
                                    } else {
                                        view! {}.into_view()
                                    }}

                                    {
                                        let num_stations = stations.len();
                                        stations.into_iter().enumerate().map(|(i, (name, station_idx))| {
                                            view! {
                                                <StopRow
                                                    index=i
                                                    name=name
                                                    station_idx=station_idx
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

                                    {if !available_end.is_empty() {
                                        let avail = available_end.clone();
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
                                                                    if let (Some(station_idx), Some(last_idx)) = (
                                                                        graph.get_station_index(&station_name),
                                                                        last_station_idx
                                                                    ) {
                                                                        // Find edge based on route direction
                                                                        let edge = match route_direction.get() {
                                                                            RouteDirection::Forward => graph.graph.find_edge(last_idx, station_idx),
                                                                            RouteDirection::Return => graph.graph.find_edge(station_idx, last_idx),
                                                                        };

                                                                        if let Some(edge) = edge {
                                                                            // Check if station is a passing loop
                                                                            let is_passing_loop = graph.graph.node_weight(station_idx)
                                                                                .map(|node| node.passing_loop)
                                                                                .unwrap_or(false);
                                                                            let default_wait = if is_passing_loop {
                                                                                Duration::seconds(0)
                                                                            } else {
                                                                                Duration::seconds(30)
                                                                            };

                                                                            let segment = crate::models::RouteSegment {
                                                                                edge_index: edge.index(),
                                                                                track_index: 0,
                                                                                origin_platform: 0,
                                                                                destination_platform: 0,
                                                                                duration: Duration::minutes(5),
                                                                                wait_time: default_wait,
                                                                            };

                                                                            match route_direction.get() {
                                                                                RouteDirection::Forward => {
                                                                                    updated_line.forward_route.push(segment);
                                                                                }
                                                                                RouteDirection::Return => {
                                                                                    updated_line.return_route.push(segment);
                                                                                }
                                                                            }
                                                                            on_save_add.with_value(|f| f(updated_line));
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                >
                                                    <option value="">"+ Add stop at end..."</option>
                                                    {avail.iter().map(|name| {
                                                        view! {
                                                            <option value=name.clone()>{name.clone()}</option>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </select>
                                            </div>
                                        }.into_view()
                                    } else {
                                        view! {}.into_view()
                                    }}
                                }.into_view()
                            }
                        })
                    }}
                </div>
            </div>
        </TabPanel>
    }
}
