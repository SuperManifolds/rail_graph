use crate::components::{
    error_list::ErrorList,
    graph_canvas::GraphCanvas,
    importer::Importer,
    legend::Legend,
    line_controls::LineControls
};
use crate::models::{Project, SegmentState, TrainJourney};
use crate::storage::{
    load_project_from_storage, save_project_to_storage,
};
use leptos::*;
use std::collections::HashSet;

#[component]
pub fn TimeGraph() -> impl IntoView {
    // Create reactive signals for lines, stations, and segment state, starting with empty project
    let (lines, set_lines) = create_signal(Vec::new());
    let (stations, set_stations) = create_signal(Vec::new());
    let (segment_state, set_segment_state) = create_signal(SegmentState {
        double_tracked_segments: HashSet::new(),
    });

    // Auto-load saved project on component mount
    create_effect(move |_| {
        spawn_local(async move {
            match load_project_from_storage().await {
                Ok(project) => {
                    set_lines.set(project.lines);
                    set_stations.set(project.stations);
                    set_segment_state.set(project.segment_state);
                }
                Err(_) => {
                    // No saved project, start with empty project
                    set_lines.set(Vec::new());
                    set_stations.set(Vec::new());
                    set_segment_state.set(SegmentState {
                        double_tracked_segments: HashSet::new(),
                    });
                }
            }
        });
    });

    // Auto-save project whenever lines, stations, or segment state change
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_stations = stations.get();
        let current_segment_state = segment_state.get();

        // Only save if we have data (skip initial empty state)
        if !current_lines.is_empty() || !current_stations.is_empty() {
            let project = Project::new(current_lines, current_stations, current_segment_state);
            spawn_local(async move {
                if let Err(e) = save_project_to_storage(&project).await {
                    web_sys::console::error_1(&format!("Auto-save failed: {}", e).into());
                }
            });
        }
    });

    let (visualization_time, set_visualization_time) =
        create_signal(chrono::Local::now().naive_local());
    let (train_journeys, set_train_journeys) = create_signal(Vec::<TrainJourney>::new());

    // Legend visibility toggles
    let (show_station_crossings, set_show_station_crossings) = create_signal(true);
    let (show_conflicts, set_show_conflicts) = create_signal(true);

    // Update train journeys only when lines configuration changes
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_stations = stations.get();

        // Filter to only visible lines
        let visible_lines: Vec<_> = current_lines.into_iter()
            .filter(|line| line.visible)
            .collect();

        // Generate journeys for the full day starting from midnight
        let new_journeys = TrainJourney::generate_journeys(&visible_lines, &current_stations);
        set_train_journeys.set(new_journeys);
    });

    // Compute conflicts and station crossings
    let station_names = create_memo(move |_| {
        stations.get().iter().map(|s| s.name.clone()).collect::<Vec<String>>()
    });
    let conflicts_and_crossings = create_memo(move |_| {
        let journeys = train_journeys.get();
        let seg_state = segment_state.get();
        let names = station_names.get();
        crate::models::detect_line_conflicts(&journeys, &names, &seg_state)
    });

    let conflicts_only = Signal::derive(move || conflicts_and_crossings.get().0);

    view! {
        <div class="time-graph-container">
            <div class="main-content">
                <GraphCanvas
                    stations=stations
                    train_journeys=train_journeys
                    visualization_time=visualization_time
                    set_visualization_time=set_visualization_time
                    segment_state=segment_state
                    set_segment_state=set_segment_state
                    show_station_crossings=show_station_crossings
                    show_conflicts=show_conflicts
                    conflicts_and_crossings=conflicts_and_crossings
                />
            </div>
            <div class="sidebar">
                <div class="sidebar-header">
                    <h2>"Railway Time Graph"</h2>
                    <ErrorList conflicts=conflicts_only />
                </div>
                <LineControls lines=lines set_lines=set_lines />
                <div class="sidebar-footer">
                    <Importer set_lines=set_lines set_stations=set_stations />
                    <Legend
                        show_station_crossings=show_station_crossings
                        set_show_station_crossings=set_show_station_crossings
                        show_conflicts=show_conflicts
                        set_show_conflicts=set_show_conflicts
                    />
                </div>
            </div>
        </div>
    }
}

