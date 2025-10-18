use crate::components::{
    button::Button,
    day_selector::DaySelector,
    error_list::ErrorList,
    graph_canvas::GraphCanvas,
    importer::Importer,
    legend::Legend,
    line_controls::LineControls,
    line_editor::LineEditor
};
use crate::models::{Line, RailwayGraph, GraphView, Stations};
use crate::train_journey::TrainJourney;
use crate::conflict::Conflict;
use leptos::{component, view, Signal, IntoView, SignalGet, create_signal, create_memo, ReadSignal, WriteSignal, SignalUpdate, SignalSet, create_effect};

#[inline]
fn compute_display_nodes(
    view: Option<GraphView>,
    graph: ReadSignal<RailwayGraph>,
) -> Signal<Vec<(petgraph::stable_graph::NodeIndex, crate::models::Node)>> {
    Signal::derive(move || {
        let current_graph = graph.get();
        if let Some(ref graph_view) = view {
            graph_view.get_nodes_for_display(&current_graph)
        } else {
            current_graph.get_all_nodes_ordered()
        }
    })
}

#[inline]
fn compute_station_index_map(
    view: Option<GraphView>,
    graph: ReadSignal<RailwayGraph>,
) -> Signal<std::collections::HashMap<usize, usize>> {
    Signal::derive(move || {
        let current_graph = graph.get();
        if let Some(ref graph_view) = view {
            graph_view.build_station_index_map(&current_graph)
        } else {
            // For full graph view, map station indices to display indices
            // accounting for junctions that occupy display rows but aren't in the station list
            let all_nodes = current_graph.get_all_nodes_ordered();
            let all_stations = current_graph.get_all_stations_ordered();

            // Create a mapping from station NodeIndex to station list index
            let station_node_to_idx: std::collections::HashMap<_, _> = all_stations
                .iter()
                .enumerate()
                .map(|(idx, (node_idx, _))| (*node_idx, idx))
                .collect();

            // Map each station's index to its display row position
            let mut map = std::collections::HashMap::new();
            for (display_idx, (node_idx, _)) in all_nodes.iter().enumerate() {
                if let Some(&station_idx) = station_node_to_idx.get(node_idx) {
                    map.insert(station_idx, display_idx);
                }
            }

            map
        }
    })
}

#[component]
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn TimeGraph(
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    legend: ReadSignal<crate::models::Legend>,
    set_legend: WriteSignal<crate::models::Legend>,
    #[prop(optional)]
    view: Option<GraphView>,
    train_journeys: ReadSignal<std::collections::HashMap<uuid::Uuid, TrainJourney>>,
    selected_day: ReadSignal<Option<chrono::Weekday>>,
    set_selected_day: WriteSignal<Option<chrono::Weekday>>,
    raw_conflicts: Signal<Vec<Conflict>>,
    on_create_view: leptos::Callback<GraphView>,
    on_viewport_change: leptos::Callback<crate::models::ViewportState>,
    set_show_project_manager: WriteSignal<bool>,
) -> impl IntoView {
    let (visualization_time, set_visualization_time) =
        create_signal(chrono::Local::now().naive_local());

    // Extract legend signals
    let show_conflicts = Signal::derive(move || legend.get().show_conflicts);
    let show_line_blocks = Signal::derive(move || legend.get().show_line_blocks);

    let set_show_conflicts = move |value: bool| {
        set_legend.update(|l| l.show_conflicts = value);
    };
    let set_show_line_blocks = move |value: bool| {
        set_legend.update(|l| l.show_line_blocks = value);
    };

    // Track hovered journey for block visualization
    let (hovered_journey_id, set_hovered_journey_id) = create_signal(None::<uuid::Uuid>);

    // Filter journeys for this view
    let (filtered_journeys, set_filtered_journeys) = create_signal(std::collections::HashMap::<uuid::Uuid, TrainJourney>::new());

    create_effect({
        let view = view.clone();
        move |_| {
            let all_journeys = train_journeys.get();
            let filtered = if let Some(ref graph_view) = view {
                let journeys_vec: Vec<_> = all_journeys.values().cloned().collect();
                let current_graph = graph.get();
                let filtered_vec = graph_view.filter_journeys(&journeys_vec, &current_graph);
                filtered_vec.into_iter().map(|j| (j.id, j)).collect()
            } else {
                all_journeys
            };
            set_filtered_journeys.set(filtered);
        }
    });

    // Filter conflicts for this view
    let conflicts = {
        let view = view.clone();
        Signal::derive(move || {
            let all_conflicts = raw_conflicts.get();
            if let Some(ref graph_view) = view {
                let current_graph = graph.get();
                graph_view.filter_conflicts(&all_conflicts, &current_graph)
            } else {
                all_conflicts
            }
        })
    };

    let conflicts_memo = create_memo(move |_| conflicts.get());

    // Signal for panning to conflicts
    let (pan_to_conflict, set_pan_to_conflict) = create_signal(None::<(f64, f64)>);

    let (new_line_dialog_open, set_new_line_dialog_open) = create_signal(false);
    let (next_line_number, set_next_line_number) = create_signal(1);

    // Get nodes (stations and junctions) to display based on view
    let display_stations = compute_display_nodes(view.clone(), graph);
    // Build station index mapping for conflict rendering
    let station_idx_map = compute_station_index_map(view.clone(), graph);

    view! {
        <div class="time-graph-container">
            <div class="main-content">
                <GraphCanvas
                    graph=graph
                    train_journeys=filtered_journeys
                    visualization_time=visualization_time
                    set_visualization_time=set_visualization_time
                    show_conflicts=show_conflicts
                    show_line_blocks=show_line_blocks
                    hovered_journey_id=hovered_journey_id
                    set_hovered_journey_id=set_hovered_journey_id
                    conflicts_memo=conflicts_memo
                    pan_to_conflict_signal=pan_to_conflict
                    display_stations=display_stations
                    station_idx_map=station_idx_map
                    initial_viewport={view.as_ref().map_or(crate::models::ViewportState::default(), |v| v.viewport_state.clone())}
                    on_viewport_change=on_viewport_change
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
                        conflicts=conflicts
                        on_conflict_click=move |time_fraction, station_pos| {
                            set_pan_to_conflict.set(Some((time_fraction, station_pos)));
                        }
                        graph=graph
                    />
                </div>
                <LineControls lines=lines set_lines=set_lines graph=graph on_create_view=on_create_view />
                <div class="sidebar-footer">
                    <Button
                        class="import-button"
                        on_click=leptos::Callback::new(move |_| set_show_project_manager.set(true))
                        shortcut="P"
                        title="Manage Projects"
                    >
                        <i class="fa-solid fa-folder"></i>
                    </Button>
                    <Button
                        class="import-button"
                        on_click=leptos::Callback::new(move |_| set_new_line_dialog_open.set(true))
                        shortcut="L"
                        title="Create new line"
                    >
                        <i class="fa-solid fa-plus"></i>
                    </Button>
                    <Importer lines=lines set_lines=set_lines set_graph=set_graph />
                    <Legend
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

