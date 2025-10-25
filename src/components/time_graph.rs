use crate::components::{
    button::Button,
    day_selector::DaySelector,
    error_list::ErrorList,
    graph_canvas::GraphCanvas,
    importer::Importer,
    legend::Legend,
    line_controls::LineControls,
    line_editor::LineEditor,
    settings::Settings
};
use crate::models::{Line, RailwayGraph, GraphView, Stations, Routes};
use crate::train_journey::TrainJourney;
use crate::conflict::Conflict;
use leptos::{component, view, Signal, IntoView, SignalGet, create_signal, create_memo, ReadSignal, WriteSignal, SignalUpdate, SignalSet, create_effect};
use petgraph::visit::EdgeRef;

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

fn compute_edge_path(
    view: Option<GraphView>,
    graph: ReadSignal<RailwayGraph>,
) -> Signal<Vec<usize>> {
    Signal::derive(move || {
        let current_graph = graph.get();
        if let Some(ref graph_view) = view {
            // Use view's edge_path if available, otherwise calculate from station_range
            let edge_path = if let Some(ref edge_path) = graph_view.edge_path {
                edge_path.clone()
            } else if let Some((from, to)) = graph_view.station_range {
                // Calculate edge path from station range
                current_graph.find_path_between_nodes(from, to)
                    .map(|edges| edges.iter().map(|e| e.index()).collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            // Log the computed edge path
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&format!(
                "View '{}' edge_path: {:?}",
                graph_view.name, edge_path
            )));

            edge_path
        } else {
            // No view - return empty edge path (full view mode not using edge matching)
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("No view - using full graph"));
            Vec::new()
        }
    })
}

fn build_station_index_mapping(graph: &RailwayGraph) -> std::collections::HashMap<usize, usize> {
    // Build a map from conflict detection indices (enumeration of all nodes)
    // to display indices (BFS order of all nodes)
    // This matches how conflicts are created in worker_bridge.rs
    //
    // Note: This duplicates BFS logic from get_all_nodes_ordered() because we need
    // the mapping from enumeration indices to BFS positions, not just the BFS order itself.
    // Conflicts store station indices as node_indices().enumerate(), but rendering uses BFS order.

    // First, create NodeIndex -> enumeration index (what conflicts use)
    let node_to_enum_idx: std::collections::HashMap<_, _> = graph.graph.node_indices()
        .enumerate()
        .map(|(enum_idx, node_idx)| (node_idx, enum_idx))
        .collect();

    // Second, map enumeration indices to display indices via BFS
    let mut map = std::collections::HashMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut display_idx = 0;

    let Some(start_node) = graph.graph.node_indices().next() else {
        return map;
    };

    queue.push_back(start_node);
    seen.insert(start_node);

    while let Some(node_idx) = queue.pop_front() {
        if let Some(&enum_idx) = node_to_enum_idx.get(&node_idx) {
            map.insert(enum_idx, display_idx);
        }
        display_idx += 1;

        for edge in graph.graph.edges(node_idx) {
            let target = edge.target();
            if seen.insert(target) {
                queue.push_back(target);
            }
        }
    }

    // Handle disconnected nodes
    for node_idx in graph.graph.node_indices() {
        if !seen.insert(node_idx) {
            continue;
        }
        if let Some(&enum_idx) = node_to_enum_idx.get(&node_idx) {
            map.insert(enum_idx, display_idx);
        }
        display_idx += 1;
    }

    map
}

#[inline]
fn compute_station_index_map(
    view: Option<GraphView>,
    graph: ReadSignal<RailwayGraph>,
) -> leptos::Memo<std::collections::HashMap<usize, usize>> {
    leptos::create_memo(move |_| {
        let current_graph = graph.get();
        if let Some(ref graph_view) = view {
            graph_view.build_station_index_map(&current_graph)
        } else {
            build_station_index_mapping(&current_graph)
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
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_settings: WriteSignal<crate::models::ProjectSettings>,
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
    let spacing_mode = Signal::derive(move || legend.get().spacing_mode);

    let set_show_conflicts = move |value: bool| {
        set_legend.update(|l| l.show_conflicts = value);
    };
    let set_show_line_blocks = move |value: bool| {
        set_legend.update(|l| l.show_line_blocks = value);
    };
    let set_spacing_mode = move |value: crate::models::SpacingMode| {
        set_legend.update(|l| l.spacing_mode = value);
    };

    // Track hovered journey for block visualization
    let (hovered_journey_id, set_hovered_journey_id) = create_signal(None::<uuid::Uuid>);

    // Filter journeys for this view
    let (filtered_journeys, set_filtered_journeys) = create_signal(std::collections::HashMap::<uuid::Uuid, TrainJourney>::new());

    create_effect(move |_| {
        let all_journeys = train_journeys.get();
        // Don't filter journeys for views - the edge-based matching handles partial visibility
        set_filtered_journeys.set(all_journeys);
    });

    // Get nodes (stations and junctions) to display based on view
    let display_stations = compute_display_nodes(view.clone(), graph);
    // Get edge path for journey rendering
    let view_edge_path = compute_edge_path(view.clone(), graph);
    // Build station index mapping for conflict rendering
    let station_idx_map = compute_station_index_map(view.clone(), graph);

    // Filter conflicts for this view (use display_stations to avoid re-computing nodes)
    let conflicts = {
        let view = view.clone();
        Signal::derive(move || {
            let all_conflicts = raw_conflicts.get();
            if let Some(ref graph_view) = view {
                let current_graph = graph.get();
                let nodes = display_stations.get();
                graph_view.filter_conflicts(&all_conflicts, &current_graph, &nodes)
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
                    spacing_mode=spacing_mode
                    hovered_journey_id=hovered_journey_id
                    set_hovered_journey_id=set_hovered_journey_id
                    conflicts_memo=conflicts_memo
                    pan_to_conflict_signal=pan_to_conflict
                    display_stations=display_stations
                    station_idx_map=station_idx_map
                    view_edge_path=view_edge_path
                    initial_viewport={view.as_ref().map_or(crate::models::ViewportState::default(), |v| v.viewport_state.clone())}
                    on_viewport_change=on_viewport_change
                />
            </div>
            <div class="sidebar">
                <div class="sidebar-header">
                    <h2>
                        <img src="/static/railgraph.svg" alt="RailGraph" class="logo-icon" />
                        "railgraph.app"
                    </h2>
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
                        station_idx_map=station_idx_map
                    />
                </div>
                <LineControls lines=lines set_lines=set_lines graph=graph on_create_view=on_create_view settings=settings />
                <div class="sidebar-footer">
                    <Button
                        class="import-button"
                        on_click=leptos::Callback::new(move |_| set_show_project_manager.set(true))
                        shortcut_id="manage_projects"
                        title="Manage Projects"
                    >
                        <i class="fa-solid fa-folder"></i>
                    </Button>
                    <Button
                        class="import-button"
                        on_click=leptos::Callback::new(move |_| set_new_line_dialog_open.set(true))
                        shortcut_id="create_line"
                        title="Create new line"
                    >
                        <i class="fa-solid fa-plus"></i>
                    </Button>
                    <Importer lines=lines set_lines=set_lines graph=graph set_graph=set_graph settings=settings />
                    <Legend
                        show_conflicts=show_conflicts
                        set_show_conflicts=set_show_conflicts
                        show_line_blocks=show_line_blocks
                        set_show_line_blocks=set_show_line_blocks
                        spacing_mode=spacing_mode
                        set_spacing_mode=set_spacing_mode
                    />
                    <Settings
                        settings=Signal::derive(move || settings.get())
                        set_settings=move |s| set_settings.set(s)
                    />
                </div>
            </div>

            <LineEditor
                initial_line=Signal::derive(move || {
                    if new_line_dialog_open.get() {
                        let line_num = next_line_number.get();
                        let line_id = format!("Line {line_num}");
                        let existing_line_count = lines.get().len();

                        Some(Line::create_from_ids(&[line_id], existing_line_count)[0].clone())
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
                            if !current_lines.iter().any(|l| l.name == candidate) {
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
                settings=settings
            />
        </div>
    }
}

