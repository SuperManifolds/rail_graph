use crate::models::{RailwayGraph, Line, Track, TrackDirection, Stations, Tracks, Junctions};
use crate::components::infrastructure_canvas::{auto_layout, renderer, hit_detection};
use crate::components::infrastructure_toolbar::{InfrastructureToolbar, EditMode};
use crate::components::canvas_viewport;
use crate::components::canvas_controls_hint::CanvasControlsHint;
use crate::components::multi_select_toolbar::MultiSelectToolbar;
use crate::components::graph_canvas::types::ViewportState;
use crate::components::add_station::{AddStation, AddStationsBatchCallback};
use crate::components::add_station_quick::QuickEntryStation;
use crate::components::confirmation_dialog::ConfirmationDialog;
use crate::components::create_view_dialog::CreateViewDialog;
use crate::components::delete_station_confirmation::DeleteStationConfirmation;
use crate::components::edit_junction::EditJunction;
use crate::components::edit_station::EditStation;
use crate::components::edit_track::EditTrack;
use leptos::{wasm_bindgen, web_sys, component, view, ReadSignal, WriteSignal, IntoView, create_node_ref, create_signal, create_effect, SignalGet, SignalSet, SignalGetUntracked, Callable, Signal, use_context, StoredValue, store_value};
use wasm_bindgen::closure::Closure;
use crate::models::UserSettings;
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::HashSet;
use std::rc::Rc;
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, MouseEvent, WheelEvent};

// Use the TopologyCache from renderer module
type TopologyCache = renderer::TopologyCache;

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
    topology_cache: StoredValue<RefCell<TopologyCache>>,
) {
    let world_x = (x - viewport.pan_offset_x) / viewport.zoom_level;
    let world_y = (y - viewport.pan_offset_y) / viewport.zoom_level;

    let current_graph = graph.get();

    // Check for label or station (use cached labels if available)
    let hovered_node = topology_cache.with_value(|cache| {
        let cache_borrow = cache.borrow();
        if let Some((_, ref label_cache)) = cache_borrow.label_cache {
            hit_detection::find_label_at_position_cached(label_cache, world_x, world_y)
        } else {
            None
        }
    }).or_else(|| hit_detection::find_station_at_position(&current_graph, world_x, world_y));

    if hovered_node.is_some() {
        set_is_over_station.set(true);
        set_is_over_track.set(false);
    } else {
        // Use cached edge segments for hit detection
        let track_hit = topology_cache.with_value(|cache| {
            hit_detection::find_track_at_position_cached(&cache.borrow().edge_segments, world_x, world_y)
        });

        if track_hit.is_some() {
            set_is_over_station.set(false);
            set_is_over_track.set(true);
        } else {
            set_is_over_station.set(false);
            set_is_over_track.set(false);
        }
    }
}

fn screen_to_world(screen_x: f64, screen_y: f64, zoom: f64, pan_x: f64, pan_y: f64) -> (f64, f64) {
    ((screen_x - pan_x) / zoom, (screen_y - pan_y) / zoom)
}

/// Check if right-clicking on preview station and clear if so
fn handle_preview_station_right_click(
    world_x: f64,
    world_y: f64,
    show_add_station: ReadSignal<bool>,
    station_dialog_clicked_position: ReadSignal<Option<(f64, f64)>>,
    set_station_dialog_clicked_position: WriteSignal<Option<(f64, f64)>>,
    set_station_dialog_clicked_segment: WriteSignal<Option<EdgeIndex>>,
) -> bool {
    const PREVIEW_CLICK_RADIUS: f64 = 15.0;

    if !show_add_station.get() {
        return false;
    }

    if let Some((preview_x, preview_y)) = station_dialog_clicked_position.get() {
        let dx = world_x - preview_x;
        let dy = world_y - preview_y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance <= PREVIEW_CLICK_RADIUS {
            set_station_dialog_clicked_position.set(None);
            set_station_dialog_clicked_segment.set(None);
            return true;
        }
    }

    false
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
/// Split a track segment by inserting a node in the middle
/// Returns (`edge1_index`, `edge2_index`) where edge1 is `from_node` -> `new_node` and edge2 is `new_node` -> `to_node`
fn split_segment_and_insert_node(
    clicked_edge: EdgeIndex,
    new_node_idx: NodeIndex,
    updated_graph: &mut RailwayGraph,
    current_lines: &mut [Line],
    should_set_routing_rules: bool,
    handedness: crate::models::TrackHandedness,
) -> (EdgeIndex, EdgeIndex) {
    use crate::models::Junctions;

    // Get edge details before we modify the graph
    let Some(edge_ref) = updated_graph.graph.edge_references().find(|e| e.id() == clicked_edge) else {
        panic!("Clicked edge not found in graph");
    };
    let from_node = edge_ref.source();
    let to_node = edge_ref.target();
    let tracks = edge_ref.weight().tracks.clone();
    let distance = edge_ref.weight().distance;
    let old_edge_index = clicked_edge.index();
    let track_count = tracks.len();

    // Get platform count from the new node
    let platform_count = updated_graph.graph.node_weight(new_node_idx)
        .and_then(|node| node.as_station())
        .map_or(1, |station| station.platforms.len());

    // Remove the old edge
    updated_graph.graph.remove_edge(clicked_edge);

    // Create two new edges: from_node -> new_node and new_node -> to_node
    // If distance is set, split it in half
    let edge1 = updated_graph.add_track(from_node, new_node_idx, tracks.clone());
    let edge2 = updated_graph.add_track(new_node_idx, to_node, tracks);

    if let Some(dist) = distance {
        if let Some(edge1_weight) = updated_graph.graph.edge_weight_mut(edge1) {
            edge1_weight.distance = Some(dist / 2.0);
        }
        if let Some(edge2_weight) = updated_graph.graph.edge_weight_mut(edge2) {
            edge2_weight.distance = Some(dist / 2.0);
        }
    }

    // Set default routing rules to allow through traffic (only for junctions)
    if should_set_routing_rules {
        if let Some(j) = updated_graph.get_junction_mut(new_node_idx) {
            j.set_routing_rule(edge1, edge2, true);
            j.set_routing_rule(edge2, edge1, true);
        }
    }

    // Update all lines that used the old edge to now use the two new edges
    for line in current_lines {
        line.replace_split_edge(old_edge_index, edge1.index(), edge2.index(), track_count, updated_graph, platform_count, handedness);
    }

    (edge1, edge2)
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
    handedness: crate::models::TrackHandedness,
) {
    use crate::models::{Junction, Junctions};

    let current_graph = graph.get();
    let Some(clicked_edge) = hit_detection::find_track_at_position(&current_graph, world_x, world_y) else {
        return;
    };

    let mut updated_graph = current_graph;
    let mut current_lines = lines.get();

    // Create junction without initial position - autolayout will position it
    let junction = Junction {
        name: None,
        position: None,
        routing_rules: vec![],
    };
    let junction_idx = updated_graph.add_junction(junction);

    // Split the segment and insert the junction
    split_segment_and_insert_node(clicked_edge, junction_idx, &mut updated_graph, &mut current_lines, true, handedness);

    set_graph.set(updated_graph);
    set_lines.set(current_lines);

    // Open the edit dialog for the newly created junction
    set_editing_junction.set(Some(junction_idx));

    // Exit junction placement mode
    set_edit_mode.set(EditMode::None);
}

#[allow(clippy::too_many_arguments)]
fn add_station_handler(
    name: String,
    passing_loop: bool,
    connect_to: Option<NodeIndex>,
    platforms: Vec<crate::models::Platform>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_show_add_station: WriteSignal<bool>,
    set_last_added_station: WriteSignal<Option<NodeIndex>>,
    clicked_position: ReadSignal<Option<(f64, f64)>>,
    clicked_segment: ReadSignal<Option<EdgeIndex>>,
    set_clicked_position: WriteSignal<Option<(f64, f64)>>,
    set_clicked_segment: WriteSignal<Option<EdgeIndex>>,
    handedness: crate::models::TrackHandedness,
) {
    use crate::models::{Track, TrackDirection, Stations};

    let mut current_graph = graph.get();
    let node_idx = current_graph.add_or_get_station(name.clone());

    if let Some(node) = current_graph.graph.node_weight_mut(node_idx) {
        if let Some(station) = node.as_station_mut() {
            station.passing_loop = passing_loop;
            station.platforms = platforms;
        }
    }

    // Check if we're placing on a track segment
    if let Some(segment_edge) = clicked_segment.get_untracked() {
        let mut current_lines = lines.get();

        // Get endpoints for positioning the station at midpoint
        if let Some((from_node, to_node)) = current_graph.get_track_endpoints(segment_edge) {
            let from_pos = current_graph.get_station_position(from_node).unwrap_or((0.0, 0.0));
            let to_pos = current_graph.get_station_position(to_node).unwrap_or((0.0, 0.0));
            let midpoint = ((from_pos.0 + to_pos.0) / 2.0, (from_pos.1 + to_pos.1) / 2.0);
            current_graph.set_station_position(node_idx, midpoint);
        }

        // Split the segment and insert the station
        split_segment_and_insert_node(segment_edge, node_idx, &mut current_graph, &mut current_lines, false, handedness);

        set_lines.set(current_lines);
    }
    // Check if we have a clicked position (but not on a segment)
    else if let Some((x, y)) = clicked_position.get_untracked() {
        current_graph.set_station_position(node_idx, (x, y));

        // Still honor the connect_to dropdown if selected
        if let Some(connect_idx) = connect_to {
            current_graph.add_track(connect_idx, node_idx, vec![Track { direction: TrackDirection::Bidirectional }]);
        }
    }
    // Default behavior: use connect_to logic
    else if let Some(connect_idx) = connect_to {
        if let Some(connect_pos) = current_graph.get_station_position(connect_idx) {
            current_graph.set_station_position(node_idx, (connect_pos.0 + 80.0, connect_pos.1 + 40.0));
        }
        current_graph.add_track(connect_idx, node_idx, vec![Track { direction: TrackDirection::Bidirectional }]);
    }

    set_graph.set(current_graph);
    set_last_added_station.set(Some(node_idx));
    set_show_add_station.set(false);

    // Clear clicked position/segment
    set_clicked_position.set(None);
    set_clicked_segment.set(None);
}

#[allow(clippy::too_many_arguments)]
fn add_stations_batch_handler(
    station_entries: Vec<QuickEntryStation>,
    connect_to: Option<NodeIndex>,
    platforms: Vec<crate::models::Platform>,
    tracks: Vec<Track>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    _lines: ReadSignal<Vec<Line>>,
    _set_lines: WriteSignal<Vec<Line>>,
    set_show_add_station: WriteSignal<bool>,
    clicked_position: ReadSignal<Option<(f64, f64)>>,
    _clicked_segment: ReadSignal<Option<EdgeIndex>>,
    set_clicked_position: WriteSignal<Option<(f64, f64)>>,
    set_clicked_segment: WriteSignal<Option<EdgeIndex>>,
    set_selected_stations: WriteSignal<Vec<NodeIndex>>,
    set_last_added_station: WriteSignal<Option<NodeIndex>>,
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
) {
    let mut current_graph = graph.get();
    let mut added_stations = Vec::new();
    let mut prev_station_idx: Option<NodeIndex> = connect_to;

    for entry in station_entries {
        // Create the station
        let node_idx = current_graph.add_or_get_station(entry.name.clone());

        if let Some(node) = current_graph.graph.node_weight_mut(node_idx) {
            if let Some(station) = node.as_station_mut() {
                station.passing_loop = entry.is_passing_loop;
                station.platforms.clone_from(&platforms);
            }
        }

        // Position the station using the same logic as add_station_handler
        if let Some((x, y)) = clicked_position.get_untracked() {
            // For first station, use clicked position if available
            if prev_station_idx == connect_to {
                current_graph.set_station_position(node_idx, (x, y));
            } else if let Some(prev_idx) = prev_station_idx {
                // For subsequent stations, use autolayout offset from previous
                if let Some(prev_pos) = current_graph.get_station_position(prev_idx) {
                    current_graph.set_station_position(node_idx, (prev_pos.0 + 80.0, prev_pos.1 + 40.0));
                }
            }
        } else if let Some(prev_idx) = prev_station_idx {
            // Use autolayout offset from previous station
            if let Some(prev_pos) = current_graph.get_station_position(prev_idx) {
                current_graph.set_station_position(node_idx, (prev_pos.0 + 80.0, prev_pos.1 + 40.0));
            }
        } else {
            // First station with no clicked position and no connect_to - use default position
            current_graph.set_station_position(node_idx, (0.0, 0.0));
        }

        // Connect to previous station if we have one
        if let Some(prev_idx) = prev_station_idx {
            let edge_idx = current_graph.add_track(prev_idx, node_idx, tracks.clone());

            // Set the distance on the edge
            if let Some(segment) = current_graph.graph.edge_weight_mut(edge_idx) {
                segment.distance = Some(entry.distance_from_previous);
            }
        }

        added_stations.push(node_idx);
        prev_station_idx = Some(node_idx);
    }

    set_graph.set(current_graph.clone());

    // Get last station before moving added_stations
    let last_station = added_stations.last().copied();
    set_selected_stations.set(added_stations.clone());
    if let Some(last_station) = last_station {
        set_last_added_station.set(Some(last_station));
    }

    // Calculate and set selection bounds for the newly added stations
    crate::components::multi_select_toolbar::update_selection_bounds(&current_graph, &added_stations, set_selection_bounds);

    set_show_add_station.set(false);
    set_clicked_position.set(None);
    set_clicked_segment.set(None);
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

#[allow(clippy::too_many_arguments)]
fn delete_station_handler(
    station_idx: NodeIndex,
    graph: ReadSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_delete_affected_lines: WriteSignal<Vec<String>>,
    set_station_to_delete: WriteSignal<Option<NodeIndex>>,
    set_delete_station_name: WriteSignal<String>,
    set_delete_bypass_info: WriteSignal<Option<(String, String)>>,
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

    // Check if a bypass connection will be created
    let connections = current_graph.find_connections_through_station(station_idx);
    let bypass_info = if connections.len() == 1 {
        let (from_idx, to_idx, _, _) = &connections[0];
        let from_name = current_graph.graph.node_weight(*from_idx)
            .map_or_else(|| "Unknown".to_string(), crate::models::Node::display_name);
        let to_name = current_graph.graph.node_weight(*to_idx)
            .map_or_else(|| "Unknown".to_string(), crate::models::Node::display_name);
        Some((from_name, to_name))
    } else {
        None
    };

    set_delete_affected_lines.set(affected);
    set_station_to_delete.set(Some(station_idx));
    set_delete_bypass_info.set(bypass_info);
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
    set_delete_bypass_info: WriteSignal<Option<(String, String)>>,
    set_show_delete_confirmation: WriteSignal<bool>,
    station_to_delete: ReadSignal<Option<NodeIndex>>,
    clicked_position: ReadSignal<Option<(f64, f64)>>,
    clicked_segment: ReadSignal<Option<EdgeIndex>>,
    set_clicked_position: WriteSignal<Option<(f64, f64)>>,
    set_clicked_segment: WriteSignal<Option<EdgeIndex>>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_selected_stations: WriteSignal<Vec<NodeIndex>>,
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
) -> (
    Rc<dyn Fn(String, bool, Option<NodeIndex>, Vec<crate::models::Platform>)>,
    AddStationsBatchCallback,
    Rc<dyn Fn(NodeIndex, String, bool, Vec<crate::models::Platform>)>,
    Rc<dyn Fn(NodeIndex)>,
    Rc<dyn Fn()>,
    Rc<dyn Fn(EdgeIndex, Vec<Track>, Option<f64>)>,
    Rc<dyn Fn(EdgeIndex)>,
    Rc<dyn Fn(NodeIndex, Option<String>)>,
    Rc<dyn Fn(NodeIndex)>,
) {
    let handle_add_station = Rc::new(move |name: String, passing_loop: bool, connect_to: Option<NodeIndex>, platforms: Vec<crate::models::Platform>| {
        let handedness = settings.get().track_handedness;
        add_station_handler(name, passing_loop, connect_to, platforms, graph, set_graph, lines, set_lines, set_show_add_station, set_last_added_station, clicked_position, clicked_segment, set_clicked_position, set_clicked_segment, handedness);
    });

    let handle_add_stations_batch: AddStationsBatchCallback = Rc::new(move |station_entries: Vec<QuickEntryStation>, connect_to: Option<NodeIndex>, platforms: Vec<crate::models::Platform>, tracks: Vec<Track>| {
        add_stations_batch_handler(station_entries, connect_to, platforms, tracks, graph, set_graph, lines, set_lines, set_show_add_station, clicked_position, clicked_segment, set_clicked_position, set_clicked_segment, set_selected_stations, set_last_added_station, set_selection_bounds);
    });

    let handle_edit_station = Rc::new(move |station_idx: NodeIndex, new_name: String, passing_loop: bool, platforms: Vec<crate::models::Platform>| {
        edit_station_handler(station_idx, new_name, passing_loop, platforms, graph, set_graph, set_editing_station);
    });

    let handle_delete_station = Rc::new(move |station_idx: NodeIndex| {
        delete_station_handler(station_idx, graph, lines, set_delete_affected_lines, set_station_to_delete, set_delete_station_name, set_delete_bypass_info, set_show_delete_confirmation, set_editing_station);
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

    (handle_add_station, handle_add_stations_batch, handle_edit_station, handle_delete_station, confirm_delete_station, handle_edit_track, handle_delete_track, handle_edit_junction, handle_delete_junction)
}

#[allow(clippy::too_many_arguments)]
fn get_canvas_cursor_style(
    dragging_station: ReadSignal<Option<NodeIndex>>,
    edit_mode: ReadSignal<EditMode>,
    editing_station: ReadSignal<Option<NodeIndex>>,
    is_over_station: ReadSignal<bool>,
    is_over_track: ReadSignal<bool>,
    is_panning: ReadSignal<bool>,
    space_pressed: ReadSignal<bool>,
    dragging_selection: ReadSignal<bool>,
    is_over_selection: ReadSignal<bool>,
) -> &'static str {
    if dragging_station.get().is_some() || dragging_selection.get() || is_panning.get() {
        "cursor: grabbing;"
    } else if space_pressed.get() {
        "cursor: grab;"
    } else if is_over_selection.get() {
        "cursor: move;"
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

fn update_cache_if_needed(topology_cache: StoredValue<RefCell<TopologyCache>>, current_graph: &RailwayGraph) {
    topology_cache.with_value(|cache| {
        let mut cache = cache.borrow_mut();
        let current_topology = (current_graph.graph.node_count(), current_graph.graph.edge_count());
        if cache.topology != current_topology {
            *cache = renderer::build_topology_cache(current_graph);
        }
    });
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

        // Check if we should apply layout
        let has_unpositioned = current_graph
            .graph
            .node_indices()
            .any(|idx| current_graph.get_station_position(idx).is_none());

        let topology_changed = prev_topology.is_some() && prev_topology != Some(current_topology);

        // Apply layout if:
        // - First run and there are unpositioned nodes (e.g., CSV import to new project)
        // - Topology changed (nodes/edges added or removed)
        let should_layout = if prev_topology.is_none() {
            // First run: only layout if there are unpositioned nodes
            // This preserves positions from loaded projects while handling new imports
            has_unpositioned && node_count > 0
        } else {
            // Subsequent runs: layout if topology changed
            topology_changed && node_count > 0
        };

        if should_layout {
            let mut current_graph = current_graph.clone();

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

#[allow(clippy::too_many_arguments)]
fn setup_render_effect(
    graph: ReadSignal<RailwayGraph>,
    zoom_level: ReadSignal<f64>,
    pan_offset_x: ReadSignal<f64>,
    pan_offset_y: ReadSignal<f64>,
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
    edit_mode: ReadSignal<EditMode>,
    selected_station: ReadSignal<Option<NodeIndex>>,
    waypoints: ReadSignal<Vec<NodeIndex>>,
    preview_path: ReadSignal<Option<Vec<EdgeIndex>>>,
    topology_cache: StoredValue<RefCell<TopologyCache>>,
    is_zooming: ReadSignal<bool>,
    render_requested: ReadSignal<bool>,
    set_render_requested: WriteSignal<bool>,
    station_dialog_clicked_position: ReadSignal<Option<(f64, f64)>>,
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    selection_box_start: ReadSignal<Option<(f64, f64)>>,
    selection_box_end: ReadSignal<Option<(f64, f64)>>,
) {
    create_effect(move |_| {
        // Track all dependencies
        let _ = graph.get();
        let _ = zoom_level.get();
        let _ = pan_offset_x.get();
        let _ = pan_offset_y.get();
        let _ = edit_mode.get();
        let _ = selected_station.get();
        let _ = waypoints.get();
        let _ = preview_path.get();
        let _ = station_dialog_clicked_position.get();
        let _ = selected_stations.get();
        let _ = selection_box_start.get();
        let _ = selection_box_end.get();

        // Throttle renders using requestAnimationFrame
        if !render_requested.get_untracked() {
            set_render_requested.set(true);

            let Some(window) = web_sys::window() else { return };
            let callback = Closure::once(move || {
                set_render_requested.set(false);

                let Some(canvas) = canvas_ref.get_untracked() else { return };

                let current_graph = graph.get_untracked();
                let zoom = zoom_level.get_untracked();
                let pan_x = pan_offset_x.get_untracked();
                let pan_y = pan_offset_y.get_untracked();
                let current_edit_mode = edit_mode.get_untracked();
                let current_waypoints = waypoints.get_untracked();
                let current_preview = preview_path.get_untracked();
                let zooming = is_zooming.get_untracked();
                let preview_station_pos = station_dialog_clicked_position.get_untracked();
                let current_selected_stations = selected_stations.get_untracked();
                let current_selection_box = if let (Some(start), Some(end)) = (selection_box_start.get_untracked(), selection_box_end.get_untracked()) {
                    Some((start, end))
                } else {
                    None
                };

                // Update topology cache if needed
                update_cache_if_needed(topology_cache, &current_graph);

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

                // Build list of selected stations (from CreatingView mode or multi-select)
                let selected_stations: Vec<NodeIndex> = if matches!(current_edit_mode, EditMode::CreatingView) {
                    current_waypoints
                } else {
                    current_selected_stations
                };

                // Get preview path edges if in CreatingView mode
                let highlighted_edges: HashSet<EdgeIndex> = if matches!(current_edit_mode, EditMode::CreatingView) {
                    current_preview.unwrap_or_default().into_iter().collect()
                } else {
                    HashSet::new()
                };

                // Pass cache to renderer (mutable to update label cache)
                topology_cache.with_value(|cache| {
                    let mut cache_mut = cache.borrow_mut();
                    renderer::draw_infrastructure(&ctx, &current_graph, (f64::from(container_width), f64::from(container_height)), zoom, pan_x, pan_y, &selected_stations, &highlighted_edges, &mut cache_mut, zooming, preview_station_pos, current_selection_box);
                });
            });

            let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
            callback.forget();
        }
    });
}

/// Toggle station selection - add if not selected, remove if already selected
fn toggle_station_selection(
    station_idx: NodeIndex,
    current_selection: &mut Vec<NodeIndex>,
) {
    match current_selection.iter().position(|&idx| idx == station_idx) {
        Some(pos) => { current_selection.remove(pos); }
        None => { current_selection.push(station_idx); }
    }
}

/// Apply position updates to all selected stations during drag
fn update_dragged_stations(
    graph: &mut RailwayGraph,
    stations: &[NodeIndex],
    dx: f64,
    dy: f64,
    snap_to_grid: bool,
) -> (f64, f64) {
    // Snap the delta to grid increments so all stations move together
    let (offset_x, offset_y) = if snap_to_grid {
        const GRID_SIZE: f64 = 30.0;
        (
            (dx / GRID_SIZE).round() * GRID_SIZE,
            (dy / GRID_SIZE).round() * GRID_SIZE,
        )
    } else {
        (dx, dy)
    };

    for &station_idx in stations {
        let Some((old_x, old_y)) = graph.get_station_position(station_idx) else {
            continue;
        };

        let new_x = old_x + offset_x;
        let new_y = old_y + offset_y;

        graph.set_station_position(station_idx, (new_x, new_y));
    }

    // Return the actual offset applied
    (offset_x, offset_y)
}

/// Handle mouse down in multi-select mode
#[allow(clippy::too_many_arguments)]
fn handle_multi_select_mouse_down(
    world_x: f64,
    world_y: f64,
    selection_bounds: ReadSignal<Option<(f64, f64, f64, f64)>>,
    graph: ReadSignal<RailwayGraph>,
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    set_selected_stations: WriteSignal<Vec<NodeIndex>>,
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
    set_dragging_selection: WriteSignal<bool>,
    set_drag_start_pos: WriteSignal<Option<(f64, f64)>>,
    set_selection_box_start: WriteSignal<Option<(f64, f64)>>,
    set_selection_box_end: WriteSignal<Option<(f64, f64)>>,
) {
    if let Some((min_x, max_x, min_y, max_y)) = selection_bounds.get() {
        // We have bounds - check if click is inside
        let inside = world_x >= min_x && world_x <= max_x && world_y >= min_y && world_y <= max_y;
        if inside {
            // Start dragging selection
            set_dragging_selection.set(true);
            set_drag_start_pos.set(Some((world_x, world_y)));
        } else {
            // Clicked outside - clear and start new selection
            set_selected_stations.set(Vec::new());
            set_selection_bounds.set(None);
            set_selection_box_start.set(Some((world_x, world_y)));
            set_selection_box_end.set(Some((world_x, world_y)));
        }
    } else {
        // No bounds - check if clicking station or empty space
        let current_graph = graph.get();
        if let Some(station_idx) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) {
            // Toggle station selection
            let mut current_selection = selected_stations.get();
            toggle_station_selection(station_idx, &mut current_selection);
            set_selected_stations.set(current_selection);
        } else {
            // Empty space - start new selection box
            set_selected_stations.set(Vec::new());
            set_selection_box_start.set(Some((world_x, world_y)));
            set_selection_box_end.set(Some((world_x, world_y)));
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments, clippy::too_many_lines)]
fn create_event_handlers(
    canvas_ref: leptos::NodeRef<leptos::html::Canvas>,
    edit_mode: ReadSignal<EditMode>,
    set_edit_mode: WriteSignal<EditMode>,
    selected_station: ReadSignal<Option<NodeIndex>>,
    set_selected_station: WriteSignal<Option<NodeIndex>>,
    view_creation_callbacks: Rc<dyn Fn(NodeIndex)>,
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
    topology_cache: StoredValue<RefCell<TopologyCache>>,
    set_is_zooming: WriteSignal<bool>,
    show_add_station: ReadSignal<bool>,
    station_dialog_clicked_position: ReadSignal<Option<(f64, f64)>>,
    set_station_dialog_clicked_position: WriteSignal<Option<(f64, f64)>>,
    set_station_dialog_clicked_segment: WriteSignal<Option<EdgeIndex>>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    set_show_hint: WriteSignal<bool>,
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    set_selected_stations: WriteSignal<Vec<NodeIndex>>,
    selection_box_start: ReadSignal<Option<(f64, f64)>>,
    set_selection_box_start: WriteSignal<Option<(f64, f64)>>,
    selection_box_end: ReadSignal<Option<(f64, f64)>>,
    set_selection_box_end: WriteSignal<Option<(f64, f64)>>,
    selection_bounds: ReadSignal<Option<(f64, f64, f64, f64)>>,
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
    dragging_selection: ReadSignal<bool>,
    set_dragging_selection: WriteSignal<bool>,
    drag_start_pos: ReadSignal<Option<(f64, f64)>>,
    set_drag_start_pos: WriteSignal<Option<(f64, f64)>>,
    set_is_over_selection: WriteSignal<bool>,
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

            // Handle clicks while Add Station dialog is open
            if show_add_station.get() && is_single_click {
                let snapped_position = auto_layout::snap_to_grid(world_x, world_y);
                set_station_dialog_clicked_position.set(Some(snapped_position));

                // Check if clicking on a track segment
                let current_graph = graph.get();
                let clicked_segment = hit_detection::find_track_at_position(&current_graph, world_x, world_y);
                set_station_dialog_clicked_segment.set(clicked_segment);
                return;
            }

            match current_mode {
                EditMode::AddingTrack if is_single_click => {
                    let current_graph = graph.get();
                    let Some(clicked_station) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) else {
                        return;
                    };
                    handle_mouse_down_adding_track(clicked_station, selected_station, set_selected_station, graph, set_graph);
                }
                EditMode::AddingJunction if is_single_click => {
                    let handedness = settings.get().track_handedness;
                    handle_adding_junction(world_x, world_y, graph, set_graph, lines, set_lines, set_editing_junction, set_edit_mode, auto_layout_enabled, handedness);
                }
                EditMode::CreatingView if is_single_click => {
                    let current_graph = graph.get();
                    // Allow clicking any node (station or junction)
                    if let Some(clicked_node) = hit_detection::find_station_at_position(&current_graph, world_x, world_y) {
                        view_creation_callbacks(clicked_node);
                    }
                }
                EditMode::None => {
                    // Don't start selection box or drag if space is pressed (panning mode)
                    if space_pressed.get() {
                        return;
                    }

                    // If editing a station, only allow single station drag (no multi-select)
                    if editing_station.get().is_some() {
                        let current_graph = graph.get();
                        let clicked_station = hit_detection::find_station_at_position(&current_graph, world_x, world_y);
                        set_dragging_station.set(clicked_station);
                        return;
                    }

                    // Multi-select mode (only when NOT editing a station)
                    handle_multi_select_mouse_down(
                        world_x, world_y,
                        selection_bounds, graph, selected_stations, set_selected_stations,
                        set_selection_bounds, set_dragging_selection, set_drag_start_pos,
                        set_selection_box_start, set_selection_box_end
                    );
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
                // Dismiss hint when starting to pan
                set_show_hint.set(false);
            }

            if is_panning.get() {
                canvas_viewport::handle_pan_move(x, y, &viewport_copy);
            } else if dragging_selection.get() {
                // Dragging multiple selected stations
                let Some(drag_start) = drag_start_pos.get() else {
                    return;
                };

                let zoom = zoom_level.get();
                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();
                let (world_x, world_y) = screen_to_world(x, y, zoom, pan_x, pan_y);

                let dx = world_x - drag_start.0;
                let dy = world_y - drag_start.1;

                let mut current_graph = graph.get();
                let stations = selected_stations.get();

                // Get the actual snapped offset that was applied
                let (applied_offset_x, applied_offset_y) = update_dragged_stations(&mut current_graph, &stations, dx, dy, true);

                set_graph.set(current_graph.clone());
                // Only advance drag_start by the actual amount moved (snapped)
                set_drag_start_pos.set(Some((drag_start.0 + applied_offset_x, drag_start.1 + applied_offset_y)));

                // Update selection bounds with the actual offset applied
                if let Some((min_x, max_x, min_y, max_y)) = selection_bounds.get() {
                    set_selection_bounds.set(Some((
                        min_x + applied_offset_x,
                        max_x + applied_offset_x,
                        min_y + applied_offset_y,
                        max_y + applied_offset_y,
                    )));
                }
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
            } else if let Some(start) = selection_box_start.get() {
                // Update selection box while dragging
                let zoom = zoom_level.get();
                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();
                let (world_x, world_y) = screen_to_world(x, y, zoom, pan_x, pan_y);
                set_selection_box_end.set(Some((world_x, world_y)));

                // Update selection in real-time while dragging
                let current_graph = graph.get();
                let min_x = start.0.min(world_x);
                let max_x = start.0.max(world_x);
                let min_y = start.1.min(world_y);
                let max_y = start.1.max(world_y);

                let new_selection: Vec<NodeIndex> = current_graph.graph.node_indices()
                    .filter(|&idx| {
                        current_graph.get_station_position(idx)
                            .is_some_and(|(x, y)| x >= min_x && x <= max_x && y >= min_y && y <= max_y)
                    })
                    .collect();

                set_selected_stations.set(new_selection);
            } else {
                // Check if hovering over selection bounds
                let zoom = zoom_level.get();
                let pan_x = pan_offset_x.get();
                let pan_y = pan_offset_y.get();
                let (world_x, world_y) = screen_to_world(x, y, zoom, pan_x, pan_y);

                if let Some((min_x, max_x, min_y, max_y)) = selection_bounds.get() {
                    let inside = world_x >= min_x && world_x <= max_x && world_y >= min_y && world_y <= max_y;
                    set_is_over_selection.set(inside);
                } else {
                    set_is_over_selection.set(false);
                }

                let viewport_state = ViewportState {
                    zoom_level: zoom_level.get(),
                    zoom_level_x: 1.0, // Infrastructure view doesn't use horizontal zoom
                    pan_offset_x: pan_offset_x.get(),
                    pan_offset_y: pan_offset_y.get(),
                };
                handle_mouse_move_hover_detection(
                    x, y, viewport_state,
                    graph, set_is_over_station, set_is_over_track, topology_cache
                );
            }
        }
    };

    let handle_mouse_up = move |ev: MouseEvent| {
        canvas_viewport::handle_pan_end(&viewport_copy);

        // Clear multi-select drag state
        if dragging_selection.get() {
            set_dragging_selection.set(false);
            set_drag_start_pos.set(None);
        }

        // Finalize selection box (selection already updated during drag)
        if let (Some(start), Some(end)) = (selection_box_start.get(), selection_box_end.get()) {
            // Save the selection bounds for future drag detection
            let min_x = start.0.min(end.0);
            let max_x = start.0.max(end.0);
            let min_y = start.1.min(end.1);
            let max_y = start.1.max(end.1);
            set_selection_bounds.set(Some((min_x, max_x, min_y, max_y)));

            // Clear selection box
            set_selection_box_start.set(None);
            set_selection_box_end.set(None);
        }

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

            // Check if right-clicking on preview station - clear position if so
            if handle_preview_station_right_click(world_x, world_y, show_add_station, station_dialog_clicked_position, set_station_dialog_clicked_position, set_station_dialog_clicked_segment) {
                return;
            }

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

        // Mark as zooming when wheel event occurs
        set_is_zooming.set(true);

        // Dismiss hint on zoom
        set_show_hint.set(false);

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
    #[allow(unused_variables)]
    folders: ReadSignal<Vec<crate::models::LineFolder>>,
    #[allow(unused_variables)]
    set_folders: WriteSignal<Vec<crate::models::LineFolder>>,
    on_create_view: leptos::Callback<crate::models::GraphView>,
    settings: ReadSignal<crate::models::ProjectSettings>,
    #[prop(optional)]
    initial_viewport: Option<crate::models::ViewportState>,
    #[prop(optional)]
    on_viewport_change: Option<leptos::Callback<crate::models::ViewportState>>,
) -> impl IntoView {
    // Get user settings from context
    let (user_settings, _) = use_context::<(ReadSignal<UserSettings>, WriteSignal<UserSettings>)>()
        .expect("UserSettings context not found");

    // Get capturing shortcut state from context
    let (is_capturing_shortcut, _) = use_context::<(ReadSignal<bool>, WriteSignal<bool>)>()
        .expect("is_capturing_shortcut context not found");

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
    let (delete_bypass_info, set_delete_bypass_info) = create_signal(None::<(String, String)>);
    let (show_multi_delete_confirmation, set_show_multi_delete_confirmation) = create_signal(false);
    let (is_over_station, set_is_over_station) = create_signal(false);
    let (is_over_track, set_is_over_track) = create_signal(false);
    let (dragging_station, set_dragging_station) = create_signal(None::<NodeIndex>);
    let (station_dialog_clicked_position, set_station_dialog_clicked_position) = create_signal(None::<(f64, f64)>);
    let (station_dialog_clicked_segment, set_station_dialog_clicked_segment) = create_signal(None::<EdgeIndex>);

    // Multi-select state
    let (selected_stations, set_selected_stations) = create_signal(Vec::<NodeIndex>::new());
    let (selection_box_start, set_selection_box_start) = create_signal(None::<(f64, f64)>);
    let (selection_box_end, set_selection_box_end) = create_signal(None::<(f64, f64)>);
    let (selection_bounds, set_selection_bounds) = create_signal(None::<(f64, f64, f64, f64)>); // (min_x, max_x, min_y, max_y)
    let (dragging_selection, set_dragging_selection) = create_signal(false);
    let (drag_start_pos, set_drag_start_pos) = create_signal(None::<(f64, f64)>);
    let (is_over_selection, set_is_over_selection) = create_signal(false);

    // Performance cache for topology-dependent data
    let topology_cache: StoredValue<RefCell<TopologyCache>> = store_value(RefCell::new(TopologyCache::default()));

    // Track zooming state to skip expensive operations during zoom
    let (is_zooming, set_is_zooming) = create_signal(false);

    // Throttle rendering using requestAnimationFrame (similar to graph_canvas)
    let (render_requested, set_render_requested) = create_signal(false);

    // Panning keyboard state
    let (space_pressed, set_space_pressed) = create_signal(false);
    let (w_pressed, set_w_pressed) = create_signal(false);
    let (a_pressed, set_a_pressed) = create_signal(false);
    let (s_pressed, set_s_pressed) = create_signal(false);
    let (d_pressed, set_d_pressed) = create_signal(false);

    // Canvas controls hint visibility
    let (show_hint, set_show_hint) = create_signal(true);

    // View creation state - multi-point waypoint approach
    let view_creation = crate::components::view_creation::ViewCreationState::new(edit_mode);
    let view_creation_callbacks = view_creation.create_callbacks(graph, set_edit_mode, on_create_view);

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

    // Debounce is_zooming flag: clear it after 150ms of no zoom activity
    let zoom_timeout_handle: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));
    create_effect({
        let zoom_timeout_handle = zoom_timeout_handle.clone();
        move |_| {
            let _ = zoom_level.get();

            // Clear any existing timeout
            if let Some(handle) = *zoom_timeout_handle.borrow() {
                if let Some(window) = web_sys::window() {
                    window.clear_timeout_with_handle(handle);
                }
            }

            // Set new timeout to clear is_zooming after 150ms
            if let Some(window) = web_sys::window() {
                let closure = Closure::once(move || {
                    set_is_zooming.set(false);
                });

                let handle = window
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        closure.as_ref().unchecked_ref(),
                        150,
                    )
                    .ok();

                *zoom_timeout_handle.borrow_mut() = handle;
                closure.forget();
            }
        }
    });

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
        user_settings,
        is_capturing_shortcut,
    );

    // WASD continuous panning
    canvas_viewport::setup_wasd_panning(
        w_pressed, a_pressed, s_pressed, d_pressed,
        set_pan_offset_x, set_pan_offset_y,
        pan_offset_x, pan_offset_y,
    );

    // Dismiss hint when any WASD key is pressed
    create_effect(move |_| {
        if w_pressed.get() || a_pressed.get() || s_pressed.get() || d_pressed.get() {
            set_show_hint.set(false);
        }
    });

    // Dismiss hint when zoom level changes (from +/- keys)
    create_effect(move |prev_zoom: Option<f64>| {
        let current_zoom = zoom_level.get();
        if let Some(prev) = prev_zoom {
            if (current_zoom - prev).abs() > f64::EPSILON {
                set_show_hint.set(false);
            }
        }
        current_zoom
    });

    // Save viewport state when it changes
    if let Some(on_change) = on_viewport_change {
        create_effect(move |_| {
            let viewport_state = crate::models::ViewportState {
                zoom_level: zoom_level.get(),
                zoom_level_x: None, // Infrastructure view doesn't use horizontal zoom
                pan_offset_x: pan_offset_x.get(),
                pan_offset_y: pan_offset_y.get(),
                station_label_width: 120.0, // Infrastructure view uses default width
                sidebar_width: 320.0, // Infrastructure view uses default width (no sidebar)
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

    let (handle_add_station, handle_add_stations_batch, handle_edit_station, handle_delete_station, confirm_delete_station, handle_edit_track, handle_delete_track, handle_edit_junction, handle_delete_junction) =
        create_handler_callbacks(graph, set_graph, lines, set_lines, set_show_add_station, set_last_added_station, set_editing_station, set_editing_junction, set_editing_track, set_delete_affected_lines, set_station_to_delete, set_delete_station_name, set_delete_bypass_info, set_show_delete_confirmation, station_to_delete, station_dialog_clicked_position, station_dialog_clicked_segment, set_station_dialog_clicked_position, set_station_dialog_clicked_segment, settings, set_selected_stations, set_selection_bounds);

    setup_render_effect(graph, zoom_level, pan_offset_x, pan_offset_y, canvas_ref, edit_mode, selected_station, view_creation.waypoints, view_creation.preview_path, topology_cache, is_zooming, render_requested, set_render_requested, station_dialog_clicked_position, selected_stations, selection_box_start, selection_box_end);

    let (handle_mouse_down, handle_mouse_move, handle_mouse_up, handle_double_click, handle_context_menu, handle_wheel) = create_event_handlers(
        canvas_ref, edit_mode, set_edit_mode, selected_station, set_selected_station, view_creation_callbacks.on_add_waypoint.clone(), graph, set_graph,
        lines, set_lines,
        editing_station, set_editing_station, set_editing_junction, set_editing_track,
        dragging_station, set_dragging_station, set_is_over_station, set_is_over_track,
        auto_layout_enabled, space_pressed, &viewport, topology_cache, set_is_zooming,
        show_add_station, station_dialog_clicked_position, set_station_dialog_clicked_position, set_station_dialog_clicked_segment,
        settings,
        set_show_hint,
        selected_stations, set_selected_stations,
        selection_box_start, set_selection_box_start,
        selection_box_end, set_selection_box_end,
        selection_bounds, set_selection_bounds,
        dragging_selection, set_dragging_selection,
        drag_start_pos, set_drag_start_pos,
        set_is_over_selection
    );

    // Setup keyboard shortcuts for multi-select operations
    let shortcuts = leptos::create_memo(move |_| user_settings.get().keyboard_shortcuts);
    crate::models::setup_shortcut_handler(is_capturing_shortcut, shortcuts, move |action_id, _ev| {
        // Only handle multi-select shortcuts when stations are selected
        if selected_stations.get().is_empty() {
            return;
        }

        match action_id {
            "multi_select_rotate_cw" => {
                crate::components::multi_select_toolbar::rotate_selected_stations_clockwise(
                    selected_stations,
                    graph,
                    set_graph,
                    set_selection_bounds,
                );
            }
            "multi_select_rotate_ccw" => {
                crate::components::multi_select_toolbar::rotate_selected_stations_counterclockwise(
                    selected_stations,
                    graph,
                    set_graph,
                    set_selection_bounds,
                );
            }
            "multi_select_align" => {
                crate::components::multi_select_toolbar::align_selected_stations(
                    selected_stations,
                    graph,
                    set_graph,
                    set_selection_bounds,
                );
            }
            "multi_select_delete" => {
                if !selected_stations.get().is_empty() {
                    set_show_multi_delete_confirmation.set(true);
                }
            }
            "multi_select_add_platform" => {
                crate::components::multi_select_toolbar::add_platform_to_selected(
                    selected_stations,
                    graph,
                    set_graph,
                );
            }
            "multi_select_remove_platform" => {
                crate::components::multi_select_toolbar::remove_platform_from_selected(
                    selected_stations,
                    graph,
                    set_graph,
                );
            }
            "multi_select_add_track" => {
                crate::components::multi_select_toolbar::add_tracks_between_selected(
                    selected_stations,
                    graph,
                    set_graph,
                    lines,
                    set_lines,
                    settings,
                );
            }
            "multi_select_remove_track" => {
                crate::components::multi_select_toolbar::remove_tracks_between_selected(
                    selected_stations,
                    graph,
                    set_graph,
                    lines,
                    set_lines,
                    settings,
                );
            }
            _ => {}
        }
    });

    let handle_mouse_leave = move |_: MouseEvent| {
        canvas_viewport::handle_pan_end(&viewport);
        set_dragging_station.set(None);
        set_is_over_station.set(false);
        set_is_over_track.set(false);
    };

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
                    style=move || get_canvas_cursor_style(dragging_station, edit_mode, editing_station, is_over_station, is_over_track, is_panning, space_pressed, dragging_selection, is_over_selection)
                />
                <CanvasControlsHint visible=show_hint />
                <MultiSelectToolbar
                    selected_stations=selected_stations
                    selection_box_start=selection_box_start
                    graph=graph
                    zoom=zoom_level
                    pan_x=pan_offset_x
                    pan_y=pan_offset_y
                    on_rotate_cw=leptos::Callback::new(move |()| {
                        crate::components::multi_select_toolbar::rotate_selected_stations_clockwise(
                            selected_stations,
                            graph,
                            set_graph,
                            set_selection_bounds,
                        );
                    })
                    on_rotate_ccw=leptos::Callback::new(move |()| {
                        crate::components::multi_select_toolbar::rotate_selected_stations_counterclockwise(
                            selected_stations,
                            graph,
                            set_graph,
                            set_selection_bounds,
                        );
                    })
                    on_align=leptos::Callback::new(move |()| {
                        crate::components::multi_select_toolbar::align_selected_stations(
                            selected_stations,
                            graph,
                            set_graph,
                            set_selection_bounds,
                        );
                    })
                    on_add_platform=leptos::Callback::new(move |()| {
                        crate::components::multi_select_toolbar::add_platform_to_selected(
                            selected_stations,
                            graph,
                            set_graph,
                        );
                    })
                    on_remove_platform=leptos::Callback::new(move |()| {
                        crate::components::multi_select_toolbar::remove_platform_from_selected(
                            selected_stations,
                            graph,
                            set_graph,
                        );
                    })
                    on_add_track=leptos::Callback::new(move |()| {
                        crate::components::multi_select_toolbar::add_tracks_between_selected(
                            selected_stations,
                            graph,
                            set_graph,
                            lines,
                            set_lines,
                            settings,
                        );
                    })
                    on_remove_track=leptos::Callback::new(move |()| {
                        crate::components::multi_select_toolbar::remove_tracks_between_selected(
                            selected_stations,
                            graph,
                            set_graph,
                            lines,
                            set_lines,
                            settings,
                        );
                    })
                    on_delete=leptos::Callback::new(move |()| {
                        if !selected_stations.get().is_empty() {
                            set_show_multi_delete_confirmation.set(true);
                        }
                    })
                />
            </div>

            <AddStation
                is_open=show_add_station
                on_close=Rc::new(move || {
                    set_show_add_station.set(false);
                    set_station_dialog_clicked_position.set(None);
                    set_station_dialog_clicked_segment.set(None);
                })
                on_add=handle_add_station
                on_add_batch=handle_add_stations_batch
                graph=graph
                last_added_station=last_added_station
                clicked_segment=station_dialog_clicked_segment
                settings=settings
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
                bypass_info=delete_bypass_info
                on_cancel=Rc::new(move || set_show_delete_confirmation.set(false))
                on_confirm=confirm_delete_station
            />

            <ConfirmationDialog
                is_open=Signal::derive(move || show_multi_delete_confirmation.get())
                title=Signal::derive(|| "Delete Stations".to_string())
                message=Signal::derive(move || {
                    let count = selected_stations.get().len();
                    format!("Are you sure you want to delete {} selected station{}?", count, if count == 1 { "" } else { "s" })
                })
                on_cancel=Rc::new(move || set_show_multi_delete_confirmation.set(false))
                on_confirm=Rc::new(move || {
                    crate::components::multi_select_toolbar::delete_selected_stations(
                        selected_stations,
                        graph,
                        set_graph,
                        lines,
                        set_lines,
                        set_selected_stations,
                    );
                    set_show_multi_delete_confirmation.set(false);
                })
                confirm_text="Delete".to_string()
                cancel_text="Cancel".to_string()
            />

            <CreateViewDialog
                is_open=view_creation.show_dialog
                waypoints=view_creation.waypoints
                graph=graph
                validation_error=view_creation.validation_error
                on_close=view_creation_callbacks.on_close.clone()
                on_create=view_creation_callbacks.on_create.clone()
                on_add_waypoint=view_creation_callbacks.on_add_waypoint.clone()
                on_remove_waypoint=view_creation_callbacks.on_remove_waypoint.clone()
            />
        </div>
    }
}
