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
    legend: ReadSignal<crate::models::Legend>,
    set_legend: WriteSignal<crate::models::Legend>,
) -> impl IntoView {
    let (visualization_time, set_visualization_time) =
        create_signal(chrono::Local::now().naive_local());
    let (train_journeys, set_train_journeys) = create_signal(std::collections::HashMap::<uuid::Uuid, TrainJourney>::new());

    // Extract legend signals
    let show_station_crossings = Signal::derive(move || legend.get().show_station_crossings);
    let show_conflicts = Signal::derive(move || legend.get().show_conflicts);
    let show_line_blocks = Signal::derive(move || legend.get().show_line_blocks);

    let set_show_station_crossings = move |value: bool| {
        set_legend.update(|l| l.show_station_crossings = value);
    };
    let set_show_conflicts = move |value: bool| {
        set_legend.update(|l| l.show_conflicts = value);
    };
    let set_show_line_blocks = move |value: bool| {
        set_legend.update(|l| l.show_line_blocks = value);
    };

    // Track hovered journey for block visualization
    let (hovered_journey_id, set_hovered_journey_id) = create_signal(None::<uuid::Uuid>);

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
        let journeys_vec: Vec<_> = journeys.values().cloned().collect();
        let current_graph = graph.get();
        crate::conflict::detect_line_conflicts(&journeys_vec, &current_graph)
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
                    set_lines=set_lines
                    train_journeys=train_journeys
                    visualization_time=visualization_time
                    set_visualization_time=set_visualization_time
                    show_station_crossings=show_station_crossings
                    show_conflicts=show_conflicts
                    show_line_blocks=show_line_blocks
                    hovered_journey_id=hovered_journey_id
                    set_hovered_journey_id=set_hovered_journey_id
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
                        graph=graph
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
                        show_line_blocks=show_line_blocks
                        set_show_line_blocks=set_show_line_blocks
                    />
                </div>
            </div>
        </div>
    }
}

