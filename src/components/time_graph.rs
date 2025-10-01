use crate::components::{
    error_list::ErrorList,
    graph_canvas::GraphCanvas,
    importer::Importer,
    legend::Legend,
    line_controls::LineControls
};
use crate::models::{Project, SegmentState, TrainJourney, RailwayGraph};
use crate::storage::{
    load_project_from_storage, save_project_to_storage,
};
use leptos::*;
use std::collections::HashSet;

#[component]
pub fn TimeGraph() -> impl IntoView {
    // Create reactive signals for lines, graph, and segment state, starting with empty project
    let (lines, set_lines) = create_signal(Vec::new());
    let (graph, set_graph) = create_signal(RailwayGraph::new());
    let (segment_state, set_segment_state) = create_signal(SegmentState {
        double_tracked_segments: HashSet::new(),
    });

    // Auto-load saved project on component mount
    create_effect(move |_| {
        spawn_local(async move {
            match load_project_from_storage().await {
                Ok(project) => {
                    set_lines.set(project.lines);
                    set_graph.set(project.graph);
                    set_segment_state.set(project.segment_state);
                }
                Err(_) => {
                    // No saved project, start with empty project
                    set_lines.set(Vec::new());
                    set_graph.set(RailwayGraph::new());
                    set_segment_state.set(SegmentState {
                        double_tracked_segments: HashSet::new(),
                    });
                }
            }
        });
    });

    // Auto-save project whenever lines, graph, or segment state change
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_graph = graph.get();
        let current_segment_state = segment_state.get();

        // Only save if we have data (skip initial empty state)
        if !current_lines.is_empty() || !current_graph.graph.node_count() > 0 {
            let project = Project::new(current_lines, current_graph, current_segment_state);
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
        let current_graph = graph.get();

        // Filter to only visible lines
        let visible_lines: Vec<_> = current_lines.into_iter()
            .filter(|line| line.visible)
            .collect();

        // Generate journeys for the full day starting from midnight
        let new_journeys = TrainJourney::generate_journeys(&visible_lines, &current_graph);
        set_train_journeys.set(new_journeys);
    });

    // Compute conflicts and station crossings
    let conflicts_and_crossings = create_memo(move |_| {
        let journeys = train_journeys.get();
        let seg_state = segment_state.get();
        let current_graph = graph.get();
        crate::models::detect_line_conflicts(&journeys, &current_graph, &seg_state)
    });

    let conflicts_only = Signal::derive(move || conflicts_and_crossings.get().0);

    // Signal for panning to conflicts
    let (pan_to_conflict, set_pan_to_conflict) = create_signal(None::<(f64, f64)>);

    view! {
        <div class="time-graph-container">
            <div class="main-content">
                <GraphCanvas
                    graph=graph
                    train_journeys=train_journeys
                    visualization_time=visualization_time
                    set_visualization_time=set_visualization_time
                    segment_state=segment_state
                    set_segment_state=set_segment_state
                    show_station_crossings=show_station_crossings
                    show_conflicts=show_conflicts
                    conflicts_and_crossings=conflicts_and_crossings
                    pan_to_conflict_signal=pan_to_conflict
                />
            </div>
            <div class="sidebar">
                <div class="sidebar-header">
                    <h2>"Railway Time Graph"</h2>
                    <ErrorList
                        conflicts=conflicts_only
                        on_conflict_click=move |time_fraction, station_pos| {
                            set_pan_to_conflict.set(Some((time_fraction, station_pos)));
                        }
                    />
                </div>
                <LineControls lines=lines set_lines=set_lines graph=graph />
                <div class="sidebar-footer">
                    <Importer set_lines=set_lines set_graph=set_graph />
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

