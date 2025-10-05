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

#[component]
fn StopRow(
    index: usize,
    name: String,
    line: Line,
    time_mode: TimeDisplayMode,
    edited_line: ReadSignal<Option<Line>>,
    on_save: Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let cumulative_seconds: i64 = if index == 0 {
        0
    } else {
        line.route.iter().take(index).map(|seg| (seg.duration + seg.wait_time).num_seconds()).sum()
    };

    let column_content = match time_mode {
        TimeDisplayMode::Difference => {
            if index < line.route.len() {
                let segment_duration = line.route[index].duration;
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
                                        updated_line.route[index].duration = new_duration;
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
                                    let prev_cumulative_seconds: i64 = updated_line.route.iter()
                                        .take(index - 1)
                                        .map(|seg| (seg.duration + seg.wait_time).num_seconds())
                                        .sum();
                                    let prev_wait_seconds = updated_line.route[index - 1].wait_time.num_seconds();
                                    let segment_duration_seconds = new_cumulative_seconds - prev_cumulative_seconds - prev_wait_seconds;

                                    if segment_duration_seconds >= 0 {
                                        updated_line.route[index - 1].duration = Duration::seconds(segment_duration_seconds);
                                        on_save(updated_line);
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

    let wait_time_content = if index > 0 && index - 1 < line.route.len() {
        let wait_duration = line.route[index - 1].wait_time;
        view! {
            <DurationInput
                duration=Signal::derive(move || wait_duration)
                on_change={
                    let on_save = on_save.clone();
                    move |new_wait_time| {
                        if let Some(mut updated_line) = edited_line.get_untracked() {
                            if index > 0 && index - 1 < updated_line.route.len() {
                                updated_line.route[index - 1].wait_time = new_wait_time;
                                on_save(updated_line);
                            }
                        }
                    }
                }
            />
        }.into_view()
    } else {
        view! { <span class="travel-time">"-"</span> }.into_view()
    };

    view! {
        <div class="stop-row">
            <span class="station-name">{name}</span>
            {column_content}
            {wait_time_content}
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
    let (show_add_start, set_show_add_start) = create_signal(false);
    let (show_add_end, set_show_add_end) = create_signal(false);
    let on_save_add = store_value(on_save.clone());
    view! {
        <TabPanel when=Signal::derive(move || active_tab.get() == "stops")>
            <div class="line-editor-content">
                <div class="stops-controls">
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

                            if line.route.is_empty() {
                                view! {
                                    <p class="no-stops">"No stops defined for this line yet. Import a CSV to set up the route."</p>
                                }.into_view()
                            } else {
                                // Build list of stations from route
                                let mut stations = Vec::new();

                                // Add first station
                                if let Some(segment) = line.route.first() {
                                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                    if let Some((from, _)) = current_graph.get_track_endpoints(edge_idx) {
                                        if let Some(name) = current_graph.get_station_name(from) {
                                            stations.push(name.to_string());
                                        }
                                    }
                                }

                                // Add stations from each segment
                                for segment in &line.route {
                                    let edge_idx = petgraph::graph::EdgeIndex::new(segment.edge_index);
                                    if let Some((_, to)) = current_graph.get_track_endpoints(edge_idx) {
                                        if let Some(name) = current_graph.get_station_name(to) {
                                            stations.push(name.to_string());
                                        }
                                    }
                                }

                                let mode = time_mode.get();
                                let column_header = match mode {
                                    TimeDisplayMode::Difference => "Travel Time to Next",
                                    TimeDisplayMode::Absolute => "Time from Start",
                                };

                                view! {
                                    <div class="stops-header">
                                        <span>"Station"</span>
                                        <span>{column_header}</span>
                                        <span>"Wait Time"</span>
                                    </div>
                                    {stations.into_iter().enumerate().map(|(i, name)| {
                                        view! {
                                            <StopRow
                                                index=i
                                                name=name
                                                line=line.clone()
                                                time_mode=mode
                                                edited_line=edited_line
                                                on_save=on_save.clone()
                                            />
                                        }
                                    }).collect::<Vec<_>>()}
                                }.into_view()
                            }
                        })
                    }}
                </div>

                {move || {
                        edited_line.get().and_then(|line| {
                            if line.route.is_empty() {
                                return None;
                            }

                            let current_graph = graph.get();

                        // Get first and last stations
                        let first_station_idx = line.route.first()
                            .and_then(|seg| {
                                let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                                current_graph.get_track_endpoints(edge).map(|(from, _)| from)
                            });

                        let last_station_idx = line.route.last()
                            .and_then(|seg| {
                                let edge = petgraph::graph::EdgeIndex::new(seg.edge_index);
                                current_graph.get_track_endpoints(edge).map(|(_, to)| to)
                            });

                        // Get available stations for start (stations that connect TO first station)
                        let available_start: Vec<String> = first_station_idx
                            .map(|first_idx| {
                                current_graph.get_all_stations_ordered()
                                    .iter()
                                    .filter_map(|station| {
                                        let station_idx = current_graph.get_station_index(&station.name)?;
                                        if current_graph.graph.find_edge(station_idx, first_idx).is_some() {
                                            Some(station.name.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        // Get available stations for end (stations that connect FROM last station)
                        let available_end: Vec<String> = last_station_idx
                            .map(|last_idx| {
                                current_graph.get_all_stations_ordered()
                                    .iter()
                                    .filter_map(|station| {
                                        let station_idx = current_graph.get_station_index(&station.name)?;
                                        if current_graph.graph.find_edge(last_idx, station_idx).is_some() {
                                            Some(station.name.clone())
                                        } else {
                                            None
                                        }
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        Some(view! {
                            <div class="add-stops-section">
                                {if !available_start.is_empty() {
                                    view! {
                                        <div class="add-stop-control">
                                            <button
                                                class="add-stop-button"
                                                on:click=move |_| set_show_add_start.update(|v| *v = !*v)
                                            >
                                                "+ Add stop at start"
                                            </button>
                                            {move || {
                                                if show_add_start.get() {
                                                    let avail = available_start.clone();
                                                    Some(view! {
                                                        <select
                                                            class="station-select"
                                                            on:change={
                                                                move |ev| {
                                                                    let station_name = event_target_value(&ev);
                                                                    if let Some(mut updated_line) = edited_line.get_untracked() {
                                                                        let graph = graph.get();
                                                                        if let (Some(station_idx), Some(first_idx)) = (
                                                                            graph.get_station_index(&station_name),
                                                                            first_station_idx
                                                                        ) {
                                                                            if let Some(edge) = graph.graph.find_edge(station_idx, first_idx) {
                                                                                updated_line.route.insert(0, crate::models::RouteSegment {
                                                                                    edge_index: edge.index(),
                                                                                    duration: Duration::minutes(5),
                                                                                    wait_time: Duration::seconds(30),
                                                                                });
                                                                                on_save_add.with_value(|f| f(updated_line));
                                                                                set_show_add_start.set(false);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        >
                                                            <option value="">"Select station..."</option>
                                                            {avail.iter().map(|name| {
                                                                view! {
                                                                    <option value=name.clone()>{name.clone()}</option>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </select>
                                                    })
                                                } else {
                                                    None
                                                }
                                            }}
                                        </div>
                                    }.into_view()
                                } else {
                                    view! {}.into_view()
                                }}

                                {if !available_end.is_empty() {
                                    view! {
                                        <div class="add-stop-control">
                                            <button
                                                class="add-stop-button"
                                                on:click=move |_| set_show_add_end.update(|v| *v = !*v)
                                            >
                                                "+ Add stop at end"
                                            </button>
                                            {move || {
                                                if show_add_end.get() {
                                                    let avail = available_end.clone();
                                                    Some(view! {
                                                        <select
                                                            class="station-select"
                                                            on:change={
                                                                move |ev| {
                                                                    let station_name = event_target_value(&ev);
                                                                    if let Some(mut updated_line) = edited_line.get_untracked() {
                                                                        let graph = graph.get();
                                                                        if let (Some(station_idx), Some(last_idx)) = (
                                                                            graph.get_station_index(&station_name),
                                                                            last_station_idx
                                                                        ) {
                                                                            if let Some(edge) = graph.graph.find_edge(last_idx, station_idx) {
                                                                                updated_line.route.push(crate::models::RouteSegment {
                                                                                    edge_index: edge.index(),
                                                                                    duration: Duration::minutes(5),
                                                                                    wait_time: Duration::seconds(30),
                                                                                });
                                                                                on_save_add.with_value(|f| f(updated_line));
                                                                                set_show_add_end.set(false);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        >
                                                            <option value="">"Select station..."</option>
                                                            {avail.iter().map(|name| {
                                                                view! {
                                                                    <option value=name.clone()>{name.clone()}</option>
                                                                }
                                                            }).collect::<Vec<_>>()}
                                                        </select>
                                                    })
                                                } else {
                                                    None
                                                }
                                            }}
                                        </div>
                                    }.into_view()
                                } else {
                                    view! {}.into_view()
                                }}
                            </div>
                        })
                    })
                }}
            </div>
        </TabPanel>
    }
}
