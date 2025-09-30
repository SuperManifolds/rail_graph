use crate::components::{
    graph_canvas::GraphCanvas,
    line_controls::LineControls
};
use crate::models::{SegmentState, TrainJourney};
use crate::storage::{
    load_lines_from_storage, load_segment_state_from_storage, save_lines_to_storage,
    save_segment_state_to_storage,
};
use crate::data::parse_csv_data;
use leptos::*;
use std::collections::HashSet;

#[component]
pub fn TimeGraph() -> impl IntoView {
    let (lines_data, stations) = parse_csv_data();

    // Create the main lines signal at the top level
    let (lines, set_lines) = create_signal(lines_data);

    // Auto-load saved configuration on component mount
    create_effect(move |_| {
        if let Ok(saved_lines) = load_lines_from_storage() {
            set_lines.set(saved_lines);
        }
    });

    // Auto-save configuration whenever lines change
    create_effect(move |_| {
        let current_lines = lines.get();
        // Skip saving on initial load to avoid overwriting with default data
        if !current_lines.is_empty() {
            if let Err(e) = save_lines_to_storage(&current_lines) {
                web_sys::console::error_1(&format!("Auto-save failed: {}", e).into());
            }
        }
    });

    let (visualization_time, set_visualization_time) =
        create_signal(chrono::Local::now().naive_local());
    let (train_journeys, set_train_journeys) = create_signal(Vec::<TrainJourney>::new());

    // Segment state for double tracking
    let (segment_state, set_segment_state) = create_signal(SegmentState {
        double_tracked_segments: HashSet::new(),
    });

    // Auto-load saved segment state on component mount
    create_effect(move |_| {
        match load_segment_state_from_storage() {
            Ok(saved_state) => {
                set_segment_state.set(saved_state);
            }
            Err(_) => {
                // If no saved state found, use default empty state
                set_segment_state.set(SegmentState {
                    double_tracked_segments: HashSet::new(),
                });
            }
        }
    });

    // Auto-save segment state whenever it changes
    create_effect(move |_| {
        let current_state = segment_state.get();
        if let Err(e) = save_segment_state_to_storage(&current_state) {
            web_sys::console::error_1(&format!("Auto-save segment state failed: {}", e).into());
        }
    });

    let stations_clone = stations.clone();

    // Update train journeys only when lines configuration changes
    create_effect(move |_| {
        let current_lines = lines.get();
        let stations_for_journeys = stations_clone.clone();

        // Generate journeys for the full day starting from midnight
        let new_journeys = TrainJourney::generate_journeys(&current_lines, &stations_for_journeys);
        set_train_journeys.set(new_journeys);
    });

    view! {
        <div class="time-graph-container">
            <div class="main-content">
                <GraphCanvas
                    stations=stations.clone()
                    train_journeys=train_journeys
                    visualization_time=visualization_time
                    set_visualization_time=set_visualization_time
                    segment_state=segment_state
                    set_segment_state=set_segment_state
                />
            </div>
            <div class="sidebar">
                <div class="sidebar-header">
                    <h2>"Railway Time Graph"</h2>
                </div>
                <LineControls lines=lines set_lines=set_lines />
            </div>
        </div>
    }
}

