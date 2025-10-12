use crate::components::{
    day_selector::DaySelector,
    error_list::ErrorList,
    graph_canvas::GraphCanvas,
    importer::Importer,
    legend::Legend,
    line_controls::LineControls,
    line_editor::LineEditor
};
use crate::models::{Line, RailwayGraph};
use crate::train_journey::TrainJourney;
use crate::conflict::{Conflict, StationCrossing};
use leptos::{component, view, Signal, IntoView, SignalGet, create_signal, create_memo, ReadSignal, WriteSignal, SignalUpdate, SignalSet, create_effect};

#[cfg(target_arch = "wasm32")]
#[inline]
fn setup_worker_conflict_detection(
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    graph: ReadSignal<RailwayGraph>,
) -> (ReadSignal<Vec<Conflict>>, ReadSignal<Vec<StationCrossing>>) {
    use crate::worker_bridge::ConflictDetector;
    use leptos::store_value;

    let (conflicts, set_conflicts) = create_signal(Vec::new());
    let (crossings, set_crossings) = create_signal(Vec::new());

    let detector = store_value(ConflictDetector::new(
        set_conflicts,
        set_crossings,
    ));

    create_effect(move |_| {
        let journeys = train_journeys.get();
        let journeys_vec: Vec<_> = journeys.values().cloned().collect();
        let current_graph = graph.get();

        detector.update_value(|d| {
            d.detect(journeys_vec, current_graph);
        });
    });

    (conflicts, crossings)
}

#[cfg(not(target_arch = "wasm32"))]
#[inline]
fn setup_sync_conflict_detection(
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    graph: ReadSignal<RailwayGraph>,
) -> (Signal<Vec<Conflict>>, Signal<Vec<StationCrossing>>) {
    let conflicts_and_crossings = create_memo(move |_| {
        let journeys = train_journeys.get();
        let journeys_vec: Vec<_> = journeys.values().cloned().collect();
        let current_graph = graph.get();
        crate::conflict::detect_line_conflicts(&journeys_vec, &current_graph)
    });

    let conflicts = Signal::derive(move || conflicts_and_crossings.get().0);
    let crossings = Signal::derive(move || conflicts_and_crossings.get().1);

    (conflicts, crossings)
}

#[component]
#[must_use]
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
    let (selected_day, set_selected_day) = create_signal(None::<chrono::Weekday>);

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

    // Update train journeys when lines configuration or selected day changes
    create_effect(move |_| {
        let current_lines = lines.get();
        let current_graph = graph.get();
        let day_filter = selected_day.get();

        // Filter to only visible lines
        let visible_lines: Vec<_> = current_lines.into_iter()
            .filter(|line| line.visible)
            .collect();

        // Generate journeys for the full day starting from midnight
        let new_journeys = TrainJourney::generate_journeys(&visible_lines, &current_graph, day_filter);
        set_train_journeys.set(new_journeys);
    });

    // Compute conflicts and station crossings
    #[cfg(target_arch = "wasm32")]
    let (conflicts, crossings) = setup_worker_conflict_detection(train_journeys, graph);

    #[cfg(not(target_arch = "wasm32"))]
    let (conflicts, crossings) = setup_sync_conflict_detection(train_journeys, graph);

    let conflicts_and_crossings = create_memo(move |_| (conflicts.get(), crossings.get()));
    let conflicts_only = Signal::derive(move || conflicts.get());

    // Signal for panning to conflicts
    let (pan_to_conflict, set_pan_to_conflict) = create_signal(None::<(f64, f64)>);

    let (new_line_dialog_open, set_new_line_dialog_open) = create_signal(false);
    let (next_line_number, set_next_line_number) = create_signal(1);

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
                    <DaySelector
                        selected_day=selected_day
                        set_selected_day=set_selected_day
                    />
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
                    <button
                        class="import-button"
                        on:click=move |_| set_new_line_dialog_open.set(true)
                        title="Create new line"
                    >
                        <i class="fa-solid fa-plus"></i>
                    </button>
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

            <LineEditor
                initial_line=Signal::derive(move || {
                    if new_line_dialog_open.get() {
                        let line_num = next_line_number.get();
                        let line_id = format!("Line {line_num}");

                        Some(Line::create_from_ids(&[line_id])[0].clone())
                    } else {
                        None
                    }
                })
                is_open=Signal::derive(move || new_line_dialog_open.get())
                set_is_open=move |open: bool| {
                    if open {
                        // Find next available line number when opening
                        let current_lines = lines.get();
                        let mut num = 1;
                        loop {
                            let candidate = format!("Line {num}");
                            if !current_lines.iter().any(|l| l.id == candidate) {
                                set_next_line_number.set(num);
                                break;
                            }
                            num += 1;
                        }
                        set_new_line_dialog_open.set(true);
                    } else {
                        set_new_line_dialog_open.set(false);
                    }
                }
                graph=graph
                on_save=move |new_line: Line| {
                    set_lines.update(|lines_vec| {
                        // Check if this is a new line or an existing one
                        if let Some(existing) = lines_vec.iter_mut().find(|l| l.id == new_line.id) {
                            // Update existing line
                            *existing = new_line;
                        } else {
                            // Add new line
                            lines_vec.push(new_line);
                        }
                    });
                }
            />
        </div>
    }
}

