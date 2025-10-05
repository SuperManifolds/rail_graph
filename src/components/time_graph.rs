use crate::components::{
    error_list::ErrorList,
    graph_canvas::GraphCanvas,
    importer::Importer,
    legend::Legend,
    line_controls::LineControls
};
use crate::models::{Line, RailwayGraph};
use crate::train_journey::TrainJourney;
use leptos::*;

#[component]
pub fn TimeGraph(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) -> impl IntoView {
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
        let current_graph = graph.get();
        crate::conflict::detect_line_conflicts(&journeys, &current_graph)
    });

    let conflicts_only = Signal::derive(move || conflicts_and_crossings.get().0);

    // Signal for panning to conflicts
    let (pan_to_conflict, set_pan_to_conflict) = create_signal(None::<(f64, f64)>);

    view! {
        <div class="time-graph-container">
            <div class="main-content">
                <GraphCanvas
                    graph=graph
                    set_graph=set_graph
                    train_journeys=train_journeys
                    visualization_time=visualization_time
                    set_visualization_time=set_visualization_time
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

