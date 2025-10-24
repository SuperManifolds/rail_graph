use crate::models::{RailwayGraph, Line, Track, TrackDirection, Stations, Tracks, Junctions};
use crate::components::infrastructure_canvas::{auto_layout, renderer, hit_detection};
use crate::components::infrastructure_toolbar::{InfrastructureToolbar, EditMode};
use crate::components::canvas_viewport;
use crate::components::graph_canvas::types::ViewportState;
use crate::components::add_station::AddStation;
use crate::components::create_view_dialog::CreateViewDialog;
use crate::components::delete_station_confirmation::DeleteStationConfirmation;
use crate::components::edit_junction::EditJunction;
use crate::components::edit_station::EditStation;
use crate::components::edit_track::EditTrack;
use leptos::{wasm_bindgen, web_sys, component, view, ReadSignal, WriteSignal, IntoView, create_node_ref, create_signal, create_effect, SignalGet, SignalSet, SignalGetUntracked, Callable, Signal};
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, MouseEvent, WheelEvent};

fn handle_mouse_down_adding_track(
    clicked_station: NodeIndex,
    selected_station: ReadSignal<Option<NodeIndex>>,
    set_selected_station: WriteSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) {
    use crate::models::{Track, TrackDirection};

    let Some(first_station) = selected_station.get() else {
        set_selected_station.set(Some(clicked_station));
        return;
    };

    if first_station != clicked_station {
        let mut updated_graph = graph.get();
        updated_graph.add_track(first_station, clicked_station, vec![Track { direction: TrackDirection::Bidirectional }]);
        set_graph.set(updated_graph);
    }
    set_selected_station.set(None);
}

fn handle_mouse_move_hover_detection(
    x: f64,
    y: f64,
    viewport: ViewportState,
    graph: ReadSignal<RailwayGraph>,
    set_is_over_station: WriteSignal<bool>,
    set_is_over_track: WriteSignal<bool>,
) {
    let world_x = (x - viewport.pan_offset_x) / viewport.zoom_level;
    let world_y = (y - viewport.pan_offset_y) / viewport.zoom_level;

    let current_graph = graph.get();

    // Check for label or station
    let hovered_node = hit_detection::find_label_at_position(&current_graph, world_x, world_y, viewport.zoom_level)
        .or_else(|| hit_detection::find_station_at_position(&current_graph, world_x, world_y));

    if hovered_node.is_some() {
        set_is_over_station.set(true);
        set_is_over_track.set(false);
    } else if hit_detection::find_track_at_position(&current_graph, world_x, world_y).is_some() {
        set_is_over_station.set(false);
        set_is_over_track.set(true);
    } else {
        set_is_over_station.set(false);
        set_is_over_track.set(false);
    }
}

fn screen_to_world(screen_x: f64, screen_y: f64, zoom: f64, pan_x: f64, pan_y: f64) -> (f64, f64) {
    ((screen_x - pan_x) / zoom, (screen_y - pan_y) / zoom)
}

/// Apply autolayout snapping after dragging a station
fn apply_drag_snap(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    station_idx: NodeIndex,
    world_x: f64,
    world_y: f64,
) {
    let mut current_graph = graph.get();

    if should_reorient_branch(&current_graph, station_idx, world_x, world_y) {
        // Significant angle change - reorient entire branch
        auto_layout::snap_to_angle(&mut current_graph, station_idx, world_x, world_y);
    } else {
        // Moving along branch - just reposition this station
        auto_layout::snap_station_along_branch(&mut current_graph, station_idx, world_x, world_y);
    }

    set_graph.set(current_graph);
}

/// Determine if a station drag should reorient the branch or just reposition along it
#[allow(clippy::similar_names)]
fn should_reorient_branch(graph: &RailwayGraph, station_idx: NodeIndex, target_x: f64, target_y: f64) -> bool {
    use crate::models::Stations;
    use petgraph::Direction;

    let Some(current_pos) = graph.get_station_position(station_idx) else {
        return false;
    };

    // Get neighbors to determine current branch direction
    let neighbors: Vec<_> = graph.graph.edges(station_idx)
        .filter_map(|e| {
            let target = e.target();
            graph.get_station_position(target).map(|pos| (target, pos))
        })
        .chain(graph.graph.edges_directed(station_idx, Direction::Incoming)
            .filter_map(|e| {
                let source = e.source();
                graph.get_station_position(source).map(|pos| (source, pos))
            }))
        .collect();

    if neighbors.is_empty() {
        return false; // No neighbors, no branch to reorient
    }

    // Calculate average branch direction
    let mut total_dx = 0.0;
    let mut total_dy = 0.0;
    let mut count = 0;

    for (_, neighbor_pos) in &neighbors {
        let dx = neighbor_pos.0 - current_pos.0;
        let dy = neighbor_pos.1 - current_pos.1;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist > 0.01 {
            total_dx += dx / dist;
            total_dy += dy / dist;
            count += 1;
        }
    }

    if count == 0 {
        return false; // No meaningful direction
    }

    let avg_dx = total_dx / f64::from(count);
    let avg_dy = total_dy / f64::from(count);
    let branch_len = (avg_dx * avg_dx + avg_dy * avg_dy).sqrt();

    if branch_len < 0.01 {
        return false; // No clear branch direction
    }

    let branch_dir_x = avg_dx / branch_len;
    let branch_dir_y = avg_dy / branch_len;

    // Calculate drag direction
    let drag_dx = target_x - current_pos.0;
    let drag_dy = target_y - current_pos.1;
    let drag_dist = (drag_dx * drag_dx + drag_dy * drag_dy).sqrt();

    if drag_dist < 20.0 {
        return false; // Very small drag, don't reorient
    }

    let drag_dir_x = drag_dx / drag_dist;
    let drag_dir_y = drag_dy / drag_dist;

    // Calculate angle between branch direction and drag direction
    let dot_product = branch_dir_x * drag_dir_x + branch_dir_y * drag_dir_y;
    let angle = dot_product.acos();

    // If angle is more than 30 degrees (π/6), consider it a reorientation
    angle.abs() > std::f64::consts::PI / 6.0
}

#[allow(clippy::too_many_arguments)]
fn handle_adding_junction(
    world_x: f64,
    world_y: f64,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_editing_junction: WriteSignal<Option<NodeIndex>>,
    set_edit_mode: WriteSignal<EditMode>,
    _auto_layout_enabled: ReadSignal<bool>,
) {
    use crate::models::{Junction, Junctions, Tracks};

    let current_graph = graph.get();
    let Some(clicked_edge) = hit_detection::find_track_at_position(&current_graph, world_x, world_y) else {
        return;
    };

    // Get edge details before we modify the graph
    let Some(edge_ref) = current_graph.graph.edge_references().find(|e| e.id() == clicked_edge) else {
        return;
    };
    let from_node = edge_ref.source();
    let to_node = edge_ref.target();
    let tracks = edge_ref.weight().tracks.clone();
    let old_edge_index = clicked_edge.index();
    let track_count = tracks.len();

    let mut updated_graph = current_graph;
    let mut current_lines = lines.get();

    // Create junction without initial position - autolayout will position it
    let junction = Junction {
        name: None,
        position: None,
        routing_rules: vec![],
    };
    let junction_idx = updated_graph.add_junction(junction);

    // Remove the old edge
    updated_graph.graph.remove_edge(clicked_edge);

    // Create two new edges: from_node -> junction and junction -> to_node
    let edge1 = updated_graph.add_track(from_node, junction_idx, tracks.clone());
    let edge2 = updated_graph.add_track(junction_idx, to_node, tracks);

    // Set default routing rules to allow through traffic
    if let Some(j) = updated_graph.get_junction_mut(junction_idx) {
        j.set_routing_rule(edge1, edge2, true);
        j.set_routing_rule(edge2, edge1, true);
    }

    // Update all lines that used the old edge to now use the two new edges
    for line in &mut current_lines {
        line.replace_split_edge(old_edge_index, edge1.index(), edge2.index(), track_count);
    }

    set_graph.set(updated_graph);
    set_lines.set(current_lines);

    // Open the edit dialog for the newly created junction
    set_editing_junction.set(Some(junction_idx));

    // Exit junction placement mode
    set_edit_mode.set(EditMode::None);
}

fn add_station_handler(
    name: String,
    passing_loop: bool,
    connect_to: Option<NodeIndex>,
    platforms: Vec<crate::models::Platform>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_show_add_station: WriteSignal<bool>,
    set_last_added_station: WriteSignal<Option<NodeIndex>>,
) {
    use crate::models::{Track, TrackDirection};

    let mut current_graph = graph.get();
    let node_idx = current_graph.add_or_get_station(name.clone());

    if let Some(node) = current_graph.graph.node_weight_mut(node_idx) {
        if let Some(station) = node.as_station_mut() {
            station.passing_loop = passing_loop;
            station.platforms = platforms;
        }
    }

    if let Some(connect_idx) = connect_to {
        if let Some(connect_pos) = current_graph.get_station_position(connect_idx) {
            current_graph.set_station_position(node_idx, (connect_pos.0 + 80.0, connect_pos.1 + 40.0));
        }
        current_graph.add_track(connect_idx, node_idx, vec![Track { direction: TrackDirection::Bidirectional }]);
    }

    set_graph.set(current_graph);
    set_last_added_station.set(Some(node_idx));
    set_show_add_station.set(false);
}

fn edit_station_handler(
    station_idx: NodeIndex,
    new_name: String,
    passing_loop: bool,
    platforms: Vec<crate::models::Platform>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_editing_station: WriteSignal<Option<NodeIndex>>,
) {
    let mut current_graph = graph.get();

    if let Some(node) = current_graph.graph.node_weight_mut(station_idx) {
        if let Some(station) = node.as_station_mut() {
            let old_name = station.name.clone();
            station.name.clone_from(&new_name);
            station.passing_loop = passing_loop;
            station.platforms = platforms;

            current_graph.station_name_to_index.remove(&old_name);
            current_graph.station_name_to_index.insert(new_name, station_idx);
        }
    }

    set_graph.set(current_graph);
    set_editing_station.set(None);
}

fn delete_station_handler(
    station_idx: NodeIndex,
    graph: ReadSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_delete_affected_lines: WriteSignal<Vec<String>>,
    set_station_to_delete: WriteSignal<Option<NodeIndex>>,
    set_delete_station_name: WriteSignal<String>,
    set_show_delete_confirmation: WriteSignal<bool>,
    set_editing_station: WriteSignal<Option<NodeIndex>>,
) {
    let current_graph = graph.get();
    let current_lines = lines.get();

    let station_edges = current_graph.get_station_edges(station_idx);

    let affected: Vec<String> = current_lines
        .iter()
        .filter(|line| line.uses_any_edge(&station_edges))
        .map(|line| line.name.clone())
        .collect();

    set_delete_affected_lines.set(affected);
    set_station_to_delete.set(Some(station_idx));
    if let Some(name) = current_graph.get_station_name(station_idx) {
        set_delete_station_name.set(name.to_string());
    }
    set_show_delete_confirmation.set(true);
    set_editing_station.set(None);
}

fn confirm_delete_station_handler(
    station_to_delete: ReadSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_show_delete_confirmation: WriteSignal<bool>,
    set_station_to_delete: WriteSignal<Option<NodeIndex>>,
) {
    let Some(station_idx) = station_to_delete.get() else { return };

    let mut current_graph = graph.get();
    let mut current_lines = lines.get();

    let (removed_edges, bypass_mapping) = current_graph.delete_station(station_idx);

    for line in &mut current_lines {
        line.update_route_after_deletion(&removed_edges, &bypass_mapping);
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_show_delete_confirmation.set(false);
    set_station_to_delete.set(None);
}

fn edit_track_handler(
    edge_idx: EdgeIndex,
    new_tracks: Vec<Track>,
    new_distance: Option<f64>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_editing_track: WriteSignal<Option<EdgeIndex>>,
) {
    let mut current_graph = graph.get();
    let mut current_lines = lines.get();
    let edge_index = edge_idx.index();
    let new_track_count = new_tracks.len();

    if let Some(track_segment) = current_graph.graph.edge_weight_mut(edge_idx) {
        track_segment.tracks = new_tracks;
        track_segment.distance = new_distance;
    }

    for line in &mut current_lines {
        line.fix_track_indices_after_change(edge_index, new_track_count, &current_graph);
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_editing_track.set(None);
}

fn delete_track_handler(
    edge_idx: EdgeIndex,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_editing_track: WriteSignal<Option<EdgeIndex>>,
) {
    let mut current_graph = graph.get();
    let mut current_lines = lines.get();
    let edge_index = edge_idx.index();

    // Get endpoints before deleting the edge
    let endpoints = current_graph.get_track_endpoints(edge_idx);

    // Try to reroute lines before deleting the edge
    if let Some((from_node, to_node)) = endpoints {
        for line in &mut current_lines {
            line.reroute_deleted_edge(edge_index, from_node, to_node, &current_graph);
        }
    }

    // Now delete the edge
    current_graph.graph.remove_edge(edge_idx);

    // Clean up any segments that still reference the deleted edge (if rerouting failed)
    for line in &mut current_lines {
        line.forward_route.retain(|segment| segment.edge_index != edge_index);
        line.return_route.retain(|segment| segment.edge_index != edge_index);
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_editing_track.set(None);
}

fn edit_junction_handler(
    junction_idx: NodeIndex,
    new_name: Option<String>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_editing_junction: WriteSignal<Option<NodeIndex>>,
) {
    let mut current_graph = graph.get();

    if let Some(node) = current_graph.graph.node_weight_mut(junction_idx) {
        if let Some(junction) = node.as_junction_mut() {
            junction.name = new_name;
        }
    }

    set_graph.set(current_graph);
    set_editing_junction.set(None);
}

fn delete_junction_handler(
    junction_idx: NodeIndex,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_editing_junction: WriteSignal<Option<NodeIndex>>,
) {
    let mut current_graph = graph.get();
    let mut current_lines = lines.get();

    let removed_edges = current_graph.delete_junction(junction_idx);

    for line in &mut current_lines {
        for edge_index in &removed_edges {
            line.forward_route.retain(|segment| segment.edge_index != *edge_index);
            line.return_route.retain(|segment| segment.edge_index != *edge_index);
        }
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_editing_junction.set(None);
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn create_handler_callbacks(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_show_add_station: WriteSignal<bool>,
    set_last_added_station: WriteSignal<Option<NodeIndex>>,
    set_editing_station: WriteSignal<Option<NodeIndex>>,
    set_editing_junction: WriteSignal<Option<NodeIndex>>,
    set_editing_track: WriteSignal<Option<EdgeIndex>>,
    set_delete_affected_lines: WriteSignal<Vec<String>>,
    set_station_to_delete: WriteSignal<Option<NodeIndex>>,
    set_delete_station_name: WriteSignal<String>,
    set_show_delete_confirmation: WriteSignal<bool>,
    station_to_delete: ReadSignal<Option<NodeIndex>>,
) -> (
    Rc<dyn Fn(String, bool, Option<NodeIndex>, Vec<crate::models::Platform>)>,
    Rc<dyn Fn(NodeIndex, String, bool, Vec<crate::models::Platform>)>,
    Rc<dyn Fn(NodeIndex)>,
    Rc<dyn Fn()>,
    Rc<dyn Fn(EdgeIndex, Vec<Track>, Option<f64>)>,
    Rc<dyn Fn(EdgeIndex)>,
    Rc<dyn Fn(NodeIndex, Option<String>)>,
    Rc<dyn Fn(NodeIndex)>,
) {
    let handle_add_station = Rc::new(move |name: String, passing_loop: bool, connect_to: Option<NodeIndex>, platforms: Vec<crate::models::Platform>| {
        add_station_handler(name, passing_loop, connect_to, platforms, graph, set_graph, set_show_add_station, set_last_added_station);
    });

    let handle_edit_station = Rc::new(move |station_idx: NodeIndex, new_name: String, passing_loop: bool, platforms: Vec<crate::models::Platform>| {
        edit_station_handler(station_idx, new_name, passing_loop, platforms, graph, set_graph, set_editing_station);
    });

    let handle_delete_station = Rc::new(move |station_idx: NodeIndex| {
        delete_station_handler(station_idx, graph, lines, set_delete_affected_lines, set_station_to_delete, set_delete_station_name, set_show_delete_confirmation, set_editing_station);
    });

    let confirm_delete_station = Rc::new(move || {
        confirm_delete_station_handler(station_to_delete, graph, set_graph, lines, set_lines, set_show_delete_confirmation, set_station_to_delete);
    });

    let handle_edit_track = Rc::new(move |edge_idx: EdgeIndex, new_tracks: Vec<Track>, new_distance: Option<f64>| {
        edit_track_handler(edge_idx, new_tracks, new_distance, graph, set_graph, lines, set_lines, set_editing_track);
    });

    let handle_delete_track = Rc::new(move |edge_idx: EdgeIndex| {
        delete_track_handler(edge_idx, graph, set_graph, lines, set_lines, set_editing_track);
    });

    let handle_edit_junction = Rc::new(move |junction_idx: NodeIndex, new_name: Option<String>| {
        edit_junction_handler(junction_idx, new_name, graph, set_graph, set_editing_junction);
    });

    let handle_delete_junction = Rc::new(move |junction_idx: NodeIndex| {
        delete_junction_handler(junction_idx, graph, set_graph, lines, set_lines, set_editing_junction);
    });

    (handle_add_station, handle_edit_station, handle_delete_station, confirm_delete_station, handle_edit_track, handle_delete_track, handle_edit_junction, handle_delete_junction)
}

fn get_canvas_cursor_style(
    dragging_station: ReadSignal<Option<NodeIndex>>,
    edit_mode: ReadSignal<EditMode>,
    editing_station: ReadSignal<Option<NodeIndex>>,
    is_over_station: ReadSignal<bool>,
    is_over_track: ReadSignal<bool>,
    is_panning: ReadSignal<bool>,
    space_pressed: ReadSignal<bool>,
) -> &'static str {
    if dragging_station.get().is_some() || is_panning.get() {
        "cursor: grabbing;"
    } else if space_pressed.get() {
        "cursor: grab;"
    } else {
        match edit_mode.get() {
            EditMode::AddingTrack | EditMode::AddingJunction | EditMode::CreatingView => "cursor: pointer;",
            EditMode::None => {
                if is_over_station.get() && editing_station.get().is_some() {
                    "cursor: grab;"
                } else if is_over_station.get() || is_over_track.get() {
                    "cursor: pointer;"
                } else {
                    "cursor: default;"
                }
            }
        }
    }
}

fn setup_auto_layout_effect(
    auto_layout_enabled: ReadSignal<bool>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
) {
    create_effect(move |prev_topology: Option<(usize, usize)>| {
        if !auto_layout_enabled.get() {
            return (0, 0);
        }

        let current_graph = graph.get();
        let node_count = current_graph.graph.node_count();
        let edge_count = current_graph.graph.edge_references().count();
        let current_topology = (node_count, edge_count);

        // Check if topology changed (node or edge count changed)
        // Skip on first run (prev_topology is None) to preserve loaded positions
        let topology_changed = prev_topology.is_some() && prev_topology != Some(current_topology);

        if topology_changed && node_count > 0 {
            let mut current_graph = current_graph.clone();

            let has_unpositioned = current_graph
                .graph
                .node_indices()
                .any(|idx| current_graph.get_station_position(idx).is_none());

            let has_positioned_nodes = current_graph.graph.node_indices()
                .any(|idx| {
                    current_graph.get_station_position(idx)
                        .is_some_and(|pos| pos != (0.0, 0.0))
                });

            if has_unpositioned {
                // New nodes without positions - use full layout
                let Some(canvas) = canvas_ref.get() else { return current_topology };
                let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
                let height = f64::from(canvas_elem.client_height());
                auto_layout::apply_layout(&mut current_graph, height);
                set_graph.set(current_graph);
            } else if has_positioned_nodes {
                // Topology changed but all nodes positioned - smart adjustment
                auto_layout::adjust_layout(&mut current_graph);
                set_graph.set(current_graph);
            }
        }

        current_topology
    });
}

fn setup_render_effect(
    graph: ReadSignal<RailwayGraph>,
    zoom_level: ReadSignal<f64>,
    pan_offset_x: ReadSignal<f64>,
    pan_offset_y: ReadSignal<f64>,
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
    edit_mode: ReadSignal<EditMode>,
    selected_station: ReadSignal<Option<NodeIndex>>,
) {
    create_effect(move |_| {
        let current_graph = graph.get();
        let _ = zoom_level.get();
        let _ = pan_offset_x.get();
        let _ = pan_offset_y.get();
        let _ = edit_mode.get();
        let _ = selected_station.get();

        let Some(canvas) = canvas_ref.get() else { return };

        let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
        // Browser dimensions are always non-negative
        #[allow(clippy::cast_sign_loss)]
        let container_width = canvas_elem.client_width() as u32;
        #[allow(clippy::cast_sign_loss)]
        let container_height = canvas_elem.client_height() as u32;

        if container_width > 0 && container_height > 0 {
            canvas_elem.set_width(container_width);
            canvas_elem.set_height(container_height);
        }

        let Some(ctx) = canvas
            .get_context("2d")
            .ok()
            .flatten()
            .and_then(|ctx| ctx.dyn_into::<CanvasRenderingContext2d>().ok())
        else {
            return;
        };

        let zoom = zoom_level.get_untracked();
        let pan_x = pan_offset_x.get_untracked();
        let pan_y = pan_offset_y.get_untracked();

        // Build list of selected stations when in CreatingView mode
        let selected_stations: Vec<NodeIndex> = if matches!(edit_mode.get_untracked(), EditMode::CreatingView) {
            selected_station.get_untracked().into_iter().collect()
        } else {
            Vec::new()
        };

        renderer::draw_infrastructure(&ctx, &current_graph, (f64::from(container_width), f64::from(container_height)), zoom, pan_x, pan_y, &selected_stations);
    });
}

#[allow(clippy::type_complexity, clippy::too_many_arguments, clippy::too_many_lines)]
fn create_event_handlers(
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
    edit_mode: ReadSignal<EditMode>,
    set_edit_mode: WriteSignal<EditMode>,
    selected_station: ReadSignal<Option<NodeIndex>>,
    set_selected_station: WriteSignal<Option<NodeIndex>>,
    set_second_station_clicked: WriteSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    editing_station: ReadSignal<Option<NodeIndex>>,
    set_editing_station: WriteSignal<Option<NodeIndex>>,
    set_editing_junction: WriteSignal<Option<NodeIndex>>,
    set_editing_track: WriteSignal<Option<EdgeIndex>>,
    dragging_station: ReadSignal<Option<NodeIndex>>,
    set_dragging_station: WriteSignal<Option<NodeIndex>>,
    set_is_over_station: WriteSignal<bool>,
    set_is_over_track: WriteSignal<bool>,
    auto_layout_enabled: ReadSignal<bool>,
    space_pressed: ReadSignal<bool>,
    viewport: &canvas_viewport::ViewportSignals,
) -> (impl Fn(MouseEvent), impl Fn(MouseEvent), impl Fn(MouseEvent), impl Fn(MouseEvent), impl Fn(MouseEvent), impl Fn(WheelEvent)) {
    let zoom_level = viewport.zoom_level;
    let pan_offset_x = viewport.pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;
    let is_panning = viewport.is_panning;
    let viewport_copy = *viewport;

    let handle_mouse_down = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let screen_x = f64::from(ev.client_x()) - rect.left();
            let screen_y = f64::from(ev.client_y()) - rect.top();

            let current_mode = edit_mode.get();
            let zoom = zoom_level.get();
            let pan_x = pan_offset_x.get();
            let pan_y = pan_offset_y.get();
            let (world_x, world_y) = screen_to_world(screen_x, screen_y, zoom, pan_x, pan_y);

            // ev.detail() returns click count: 1 for single, 2 for double, 3 for triple
            let is_single_click = ev.detail() == 1;

            match current_mode {
                EditMode::AddingTrack if is_single_click => {
                    let current_graph = graph.get();
                    let Some(clicked_station) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) else {
                        return;
                    };
                    handle_mouse_down_adding_track(clicked_station, selected_station, set_selected_station, graph, set_graph);
                }
                EditMode::AddingJunction if is_single_click => {
                    handle_adding_junction(world_x, world_y, graph, set_graph, lines, set_lines, set_editing_junction, set_edit_mode, auto_layout_enabled);
                }
                EditMode::CreatingView if is_single_click => {
                    let current_graph = graph.get();
                    let Some(clicked_station) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) else {
                        return;
                    };
                    // Only allow selecting stations, not junctions
                    if current_graph.is_junction(clicked_station) {
                        return;
                    }

                    if selected_station.get().is_none() {
                        // First station selected
                        set_selected_station.set(Some(clicked_station));
                    } else if Some(clicked_station) != selected_station.get() {
                        // Second station selected - trigger the dialog opening
                        set_second_station_clicked.set(Some(clicked_station));
                    }
                }
                EditMode::None => {
                    let current_graph = graph.get();
                    let clicked_station = hit_detection::find_station_at_position(&current_graph, world_x, world_y);

                    // Allow dragging any station when a station is being edited
                    if clicked_station.is_some() && editing_station.get().is_some() {
                        set_dragging_station.set(clicked_station);
                    }
                    // Space+move panning is handled in mouse_move, no click needed
                }
                _ => {}
            }
        }
    };

    let handle_mouse_move = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let x = f64::from(ev.client_x()) - rect.left();
            let y = f64::from(ev.client_y()) - rect.top();

            // If space is pressed and not yet panning, start panning
            if space_pressed.get() && !is_panning.get() {
                canvas_viewport::handle_pan_start(x, y, &viewport_copy);
            }

            if is_panning.get() {
                canvas_viewport::handle_pan_move(x, y, &viewport_copy);
            } else if let Some(station_idx) = dragging_station.get() {
                let zoom = zoom_level.get();
                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();
                let (world_x, world_y) = screen_to_world(x, y, zoom, pan_x, pan_y);

                let mut current_graph = graph.get();

                // Snap to grid if autolayout is enabled
                let position = if auto_layout_enabled.get() {
                    auto_layout::snap_to_grid(world_x, world_y)
                } else {
                    (world_x, world_y)
                };

                current_graph.set_station_position(station_idx, position);
                set_graph.set(current_graph);
            } else {
                let viewport_state = ViewportState {
                    zoom_level: zoom_level.get(),
                    zoom_level_x: 1.0, // Infrastructure view doesn't use horizontal zoom
                    pan_offset_x: pan_offset_x.get(),
                    pan_offset_y: pan_offset_y.get(),
                };
                handle_mouse_move_hover_detection(
                    x, y, viewport_state,
                    graph, set_is_over_station, set_is_over_track
                );
            }
        }
    };

    let handle_mouse_up = move |ev: MouseEvent| {
        canvas_viewport::handle_pan_end(&viewport_copy);

        if let Some(station_idx) = dragging_station.get() {
            if let Some(canvas_elem) = canvas_ref.get() {
                let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
                let rect = canvas.get_bounding_client_rect();
                let x = f64::from(ev.client_x()) - rect.left();
                let y = f64::from(ev.client_y()) - rect.top();

                let zoom = zoom_level.get();
                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();
                let (world_x, world_y) = screen_to_world(x, y, zoom, pan_x, pan_y);

                if auto_layout_enabled.get() {
                    apply_drag_snap(graph, set_graph, station_idx, world_x, world_y);
                } else {
                    // When autolayout is off, just snap to grid without branch reorientation
                    let mut current_graph = graph.get();
                    let snapped = auto_layout::snap_to_grid(world_x, world_y);
                    current_graph.set_station_position(station_idx, snapped);
                    set_graph.set(current_graph);
                }
            }
            set_dragging_station.set(None);
        }
    };

    let handle_double_click = move |ev: MouseEvent| {
        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let screen_x = f64::from(ev.client_x()) - rect.left();
            let screen_y = f64::from(ev.client_y()) - rect.top();

            let zoom = zoom_level.get();
            let pan_x = pan_offset_x.get();
            let pan_y = pan_offset_y.get();
            let (world_x, world_y) = screen_to_world(screen_x, screen_y, zoom, pan_x, pan_y);

            let current_graph = graph.get();

            // Check for label click first, then node click
            let clicked_node = hit_detection::find_label_at_position(&current_graph, world_x, world_y, zoom)
                .or_else(|| hit_detection::find_station_at_position(&current_graph, world_x, world_y));

            if let Some(node) = clicked_node {
                if current_graph.is_junction(node) {
                    set_editing_junction.set(Some(node));
                } else {
                    set_editing_station.set(Some(node));
                }
            } else if matches!(edit_mode.get(), EditMode::None) {
                // Only open track editor on double-click when not in a special edit mode
                if let Some(clicked_track) = hit_detection::find_track_at_position(&current_graph, world_x, world_y) {
                    set_editing_track.set(Some(clicked_track));
                }
            }
        }
    };

    let handle_context_menu = move |ev: MouseEvent| {
        ev.prevent_default();

        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let screen_x = f64::from(ev.client_x()) - rect.left();
            let screen_y = f64::from(ev.client_y()) - rect.top();

            let zoom = zoom_level.get();
            let pan_x = pan_offset_x.get();
            let pan_y = pan_offset_y.get();
            let (world_x, world_y) = screen_to_world(screen_x, screen_y, zoom, pan_x, pan_y);

            let current_graph = graph.get();

            // Check for label click first, then node click
            let clicked_node = hit_detection::find_label_at_position(&current_graph, world_x, world_y, zoom)
                .or_else(|| hit_detection::find_station_at_position(&current_graph, world_x, world_y));

            if let Some(node) = clicked_node {
                if current_graph.is_junction(node) {
                    set_editing_junction.set(Some(node));
                } else {
                    set_editing_station.set(Some(node));
                }
            } else if matches!(edit_mode.get(), EditMode::None) {
                // Only open track editor on right-click when not in a special edit mode
                if let Some(clicked_track) = hit_detection::find_track_at_position(&current_graph, world_x, world_y) {
                    set_editing_track.set(Some(clicked_track));
                }
            }
        }
    };

    let handle_wheel = move |ev: WheelEvent| {
        ev.prevent_default();

        if let Some(canvas_elem) = canvas_ref.get() {
            let canvas: &web_sys::HtmlCanvasElement = &canvas_elem;
            let rect = canvas.get_bounding_client_rect();
            let mouse_x = f64::from(ev.client_x()) - rect.left();
            let mouse_y = f64::from(ev.client_y()) - rect.top();

            canvas_viewport::handle_zoom(&ev, mouse_x, mouse_y, &viewport_copy, None, None, false);
        }
    };

    (handle_mouse_down, handle_mouse_move, handle_mouse_up, handle_double_click, handle_context_menu, handle_wheel)
}

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn InfrastructureView(
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    on_create_view: leptos::Callback<crate::models::GraphView>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    #[prop(optional)]
    initial_viewport: Option<crate::models::ViewportState>,
    #[prop(optional)]
    on_viewport_change: Option<leptos::Callback<crate::models::ViewportState>>,
) -> impl IntoView {
    let canvas_ref = create_node_ref::<leptos::html::Canvas>();
    let (auto_layout_enabled, set_auto_layout_enabled) = create_signal(true);
    let (edit_mode, set_edit_mode) = create_signal(EditMode::None);
    let (selected_station, set_selected_station) = create_signal(None::<NodeIndex>);
    let (show_add_station, set_show_add_station) = create_signal(false);
    let (last_added_station, set_last_added_station) = create_signal(None::<NodeIndex>);
    let (editing_station, set_editing_station) = create_signal(None::<NodeIndex>);
    let (editing_junction, set_editing_junction) = create_signal(None::<NodeIndex>);
    let (editing_track, set_editing_track) = create_signal(None::<EdgeIndex>);
    let (show_delete_confirmation, set_show_delete_confirmation) = create_signal(false);
    let (station_to_delete, set_station_to_delete) = create_signal(None::<NodeIndex>);
    let (delete_affected_lines, set_delete_affected_lines) = create_signal(Vec::<String>::new());
    let (delete_station_name, set_delete_station_name) = create_signal(String::new());
    let (is_over_station, set_is_over_station) = create_signal(false);
    let (is_over_track, set_is_over_track) = create_signal(false);
    let (dragging_station, set_dragging_station) = create_signal(None::<NodeIndex>);

    // Panning keyboard state
    let (space_pressed, set_space_pressed) = create_signal(false);
    let (w_pressed, set_w_pressed) = create_signal(false);
    let (a_pressed, set_a_pressed) = create_signal(false);
    let (s_pressed, set_s_pressed) = create_signal(false);
    let (d_pressed, set_d_pressed) = create_signal(false);

    // View creation state
    let (view_start_station, set_view_start_station) = create_signal(None::<NodeIndex>);
    let (view_end_station, set_view_end_station) = create_signal(None::<NodeIndex>);
    let (show_create_view_dialog, set_show_create_view_dialog) = create_signal(false);

    // Separate signal to trigger the dialog opening (to avoid effect loop)
    let (second_station_clicked, set_second_station_clicked) = create_signal(None::<NodeIndex>);

    // Watch for when second station is selected to open the dialog
    create_effect(move |_| {
        if let Some(end) = second_station_clicked.get() {
            if let Some(start) = selected_station.get() {
                if start != end {
                    // Set the dialog's station signals
                    set_view_start_station.set(Some(start));
                    set_view_end_station.set(Some(end));
                    set_show_create_view_dialog.set(true);
                    // Clear the selection state
                    set_selected_station.set(None);
                    set_second_station_clicked.set(None);
                }
            }
        }
    });

    let viewport = if let Some(initial) = initial_viewport {
        canvas_viewport::create_viewport_signals_with_initial(false, initial)
    } else {
        canvas_viewport::create_viewport_signals(false)
    };
    let zoom_level = viewport.zoom_level;
    let pan_offset_x = viewport.pan_offset_x;
    let set_pan_offset_x = viewport.set_pan_offset_x;
    let pan_offset_y = viewport.pan_offset_y;
    let set_pan_offset_y = viewport.set_pan_offset_y;
    let is_panning = viewport.is_panning;

    // Create a signal for canvas dimensions
    let canvas_dimensions = Signal::derive(move || {
        canvas_ref.get().map(|canvas| {
            let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
            (f64::from(canvas_elem.width()), f64::from(canvas_elem.height()))
        })
    });

    // Setup keyboard listeners for Space and WASD
    canvas_viewport::setup_keyboard_listeners(
        set_space_pressed,
        set_w_pressed,
        set_a_pressed,
        set_s_pressed,
        set_d_pressed,
        &viewport,
        canvas_dimensions,
        None, // No min_zoom for infrastructure view
    );

    // WASD continuous panning
    canvas_viewport::setup_wasd_panning(
        w_pressed, a_pressed, s_pressed, d_pressed,
        set_pan_offset_x, set_pan_offset_y,
        pan_offset_x, pan_offset_y,
    );

    // Save viewport state when it changes
    if let Some(on_change) = on_viewport_change {
        create_effect(move |_| {
            let viewport_state = crate::models::ViewportState {
                zoom_level: zoom_level.get(),
                zoom_level_x: None, // Infrastructure view doesn't use horizontal zoom
                pan_offset_x: pan_offset_x.get(),
                pan_offset_y: pan_offset_y.get(),
            };
            on_change.call(viewport_state);
        });
    }

    setup_auto_layout_effect(auto_layout_enabled, graph, set_graph, canvas_ref);

    let toggle_auto_layout = move |()| {
        let new_state = !auto_layout_enabled.get();
        set_auto_layout_enabled.set(new_state);

        if new_state {
            let mut current_graph = graph.get();

            // Always do full layout when toggling autolayout on to fix any junction positioning issues
            if let Some(canvas) = canvas_ref.get() {
                let canvas_elem: &web_sys::HtmlCanvasElement = &canvas;
                let height = f64::from(canvas_elem.client_height());
                auto_layout::apply_layout(&mut current_graph, height);
            }

            set_graph.set(current_graph);
        }
    };

    let (handle_add_station, handle_edit_station, handle_delete_station, confirm_delete_station, handle_edit_track, handle_delete_track, handle_edit_junction, handle_delete_junction) =
        create_handler_callbacks(graph, set_graph, lines, set_lines, set_show_add_station, set_last_added_station, set_editing_station, set_editing_junction, set_editing_track, set_delete_affected_lines, set_station_to_delete, set_delete_station_name, set_show_delete_confirmation, station_to_delete);

    setup_render_effect(graph, zoom_level, pan_offset_x, pan_offset_y, canvas_ref, edit_mode, selected_station);

    let (handle_mouse_down, handle_mouse_move, handle_mouse_up, handle_double_click, handle_context_menu, handle_wheel) = create_event_handlers(
        canvas_ref, edit_mode, set_edit_mode, selected_station, set_selected_station, set_second_station_clicked, graph, set_graph,
        lines, set_lines,
        editing_station, set_editing_station, set_editing_junction, set_editing_track,
        dragging_station, set_dragging_station, set_is_over_station, set_is_over_track,
        auto_layout_enabled, space_pressed, &viewport
    );

    let handle_mouse_leave = move |_: MouseEvent| {
        canvas_viewport::handle_pan_end(&viewport);
        set_dragging_station.set(None);
        set_is_over_station.set(false);
        set_is_over_track.set(false);
    };

    // Callback for creating a view from station range
    let handle_create_view = Rc::new(move |name: String, start: NodeIndex, end: NodeIndex| {
        let current_graph = graph.get();
        match crate::models::GraphView::from_station_range(name, start, end, &current_graph) {
            Ok(new_view) => {
                on_create_view.call(new_view);
                set_show_create_view_dialog.set(false);
                set_edit_mode.set(EditMode::None);
            }
            Err(err) => {
                web_sys::console::error_1(&format!("Failed to create view: {err}").into());
            }
        }
    });

    view! {
        <div class="infrastructure-view">
            <InfrastructureToolbar
                auto_layout_enabled=auto_layout_enabled
                toggle_auto_layout=toggle_auto_layout
                set_show_add_station=set_show_add_station
                edit_mode=edit_mode
                set_edit_mode=set_edit_mode
                set_selected_station=set_selected_station
            />
            <div class="infrastructure-canvas-container">
                <canvas
                    node_ref=canvas_ref
                    class="infrastructure-canvas"
                    on:mousedown=handle_mouse_down
                    on:mousemove=handle_mouse_move
                    on:mouseup=handle_mouse_up
                    on:mouseleave=handle_mouse_leave
                    on:dblclick=handle_double_click
                    on:wheel=handle_wheel
                    on:contextmenu=handle_context_menu
                    style=move || get_canvas_cursor_style(dragging_station, edit_mode, editing_station, is_over_station, is_over_track, is_panning, space_pressed)
                />
            </div>

            <AddStation
                is_open=show_add_station
                on_close=Rc::new(move || set_show_add_station.set(false))
                on_add=handle_add_station
                graph=graph
                last_added_station=last_added_station
            />

            <EditStation
                editing_station=editing_station
                on_close=Rc::new(move || set_editing_station.set(None))
                on_save=handle_edit_station
                on_delete=handle_delete_station
                graph=graph
                on_update_track_defaults=Rc::new(move |edge_idx: EdgeIndex, source_platform: Option<usize>, target_platform: Option<usize>| {
                    let mut current_graph = graph.get();
                    if let Some(track_segment) = current_graph.graph.edge_weight_mut(edge_idx) {
                        if let Some(src) = source_platform {
                            track_segment.default_platform_source = Some(src);
                        }
                        if let Some(tgt) = target_platform {
                            track_segment.default_platform_target = Some(tgt);
                        }
                    }
                    set_graph.set(current_graph);
                })
                on_add_connection=Rc::new(move |from_station: NodeIndex, to_station: NodeIndex| {
                    let mut current_graph = graph.get();
                    current_graph.add_track(from_station, to_station, vec![Track { direction: TrackDirection::Bidirectional }]);
                    set_graph.set(current_graph);
                })
            />

            <EditJunction
                editing_junction=editing_junction
                on_close=Rc::new(move || set_editing_junction.set(None))
                on_save=handle_edit_junction
                on_delete=handle_delete_junction
                graph=graph
                set_graph=set_graph
            />

            <EditTrack
                editing_track=editing_track
                on_close=Rc::new(move || set_editing_track.set(None))
                on_save=handle_edit_track
                on_delete=handle_delete_track
                graph=graph
                lines=lines
                settings=settings
            />

            <DeleteStationConfirmation
                is_open=show_delete_confirmation
                station_name=delete_station_name
                affected_lines=delete_affected_lines
                on_cancel=Rc::new(move || set_show_delete_confirmation.set(false))
                on_confirm=confirm_delete_station
            />

            <CreateViewDialog
                is_open=show_create_view_dialog
                start_station=view_start_station
                end_station=view_end_station
                graph=graph
                on_close=Rc::new(move || {
                    set_show_create_view_dialog.set(false);
                    set_edit_mode.set(EditMode::None);
                })
                on_create=handle_create_view
            />
        </div>
    }
}
