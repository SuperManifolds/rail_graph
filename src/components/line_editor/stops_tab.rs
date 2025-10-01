use crate::components::{duration_input::DurationInput, tab_view::TabPanel, time_input::TimeInput};
use crate::models::{Line, RailwayGraph};
use crate::constants::BASE_DATE;
use leptos::*;
use chrono::Duration;

#[derive(Clone, Copy, PartialEq)]
enum TimeDisplayMode {
    Difference,  // Time between consecutive stops
    Absolute,    // Cumulative time from start
}

#[component]
pub fn StopsTab(
    edited_line: ReadSignal<Option<Line>>,
    graph: ReadSignal<RailwayGraph>,
    active_tab: RwSignal<String>,
    on_save: std::rc::Rc<dyn Fn(Line)>,
) -> impl IntoView {
    let (time_mode, set_time_mode) = create_signal(TimeDisplayMode::Difference);
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
                                    </div>
                                    {stations.into_iter().enumerate().map(|(i, name)| {
                                        let on_save_clone = on_save.clone();
                                        let line_for_calc = line.clone();

                                        view! {
                                            <div class="stop-row">
                                                <span class="station-name">{name}</span>
                                                {move || {
                                                    let mode = time_mode.get();
                                                    let cumulative: i64 = if i == 0 {
                                                        0
                                                    } else {
                                                        line_for_calc.route.iter().take(i).map(|seg| seg.duration.num_minutes()).sum()
                                                    };

                                                    match mode {
                                                        TimeDisplayMode::Difference => {
                                                            if i < line_for_calc.route.len() {
                                                                let segment_duration = line_for_calc.route[i].duration;
                                                                // Show cumulative time at current stop (not including segment to next)
                                                                let cumulative_seconds: i64 = line_for_calc.route.iter()
                                                                    .take(i)
                                                                    .map(|seg| seg.duration.num_seconds())
                                                                    .sum();
                                                                let hours = cumulative_seconds / 3600;
                                                                let minutes = (cumulative_seconds % 3600) / 60;
                                                                let seconds = cumulative_seconds % 60;
                                                                let preview_text = format!("(Σ {:02}:{:02}:{:02})", hours, minutes, seconds);

                                                                view! {
                                                                    <div class="time-input-with-preview">
                                                                        <DurationInput
                                                                            duration=Signal::derive(move || segment_duration)
                                                                            on_change={
                                                                                let on_save = on_save_clone.clone();
                                                                                move |new_duration| {
                                                                                    if let Some(mut updated_line) = edited_line.get_untracked() {
                                                                                        updated_line.route[i].duration = new_duration;
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
                                                            if i > 0 {
                                                                let cumulative_time = BASE_DATE.and_hms_opt(0, 0, 0).unwrap() + Duration::minutes(cumulative);
                                                                view! {
                                                                    <TimeInput
                                                                        label=""
                                                                        value=Signal::derive(move || cumulative_time)
                                                                        default_time="00:00:00"
                                                                        on_change={
                                                                            let on_save = on_save_clone.clone();
                                                                            Box::new(move |new_time| {
                                                                                if let Some(mut updated_line) = edited_line.get_untracked() {
                                                                                    // Calculate minutes from midnight
                                                                                    let base_midnight = BASE_DATE.and_hms_opt(0, 0, 0).unwrap();
                                                                                    let new_cumulative = (new_time - base_midnight).num_minutes();

                                                                                    // Calculate segment duration
                                                                                    let prev_cumulative: i64 = updated_line.route.iter()
                                                                                        .take(i - 1)
                                                                                        .map(|seg| seg.duration.num_minutes())
                                                                                        .sum();
                                                                                    let segment_duration = new_cumulative - prev_cumulative;

                                                                                    if segment_duration >= 0 {
                                                                                        updated_line.route[i - 1].duration = Duration::minutes(segment_duration);
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
                                                    }
                                                }}
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                }.into_view()
                            }
                        })
                    }}
                </div>
            </div>
        </TabPanel>
    }
}
