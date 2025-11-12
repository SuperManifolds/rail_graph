use leptos::{component, view, IntoView, ReadSignal, WriteSignal, Callback, SignalGet, SignalSet, SignalWith, Callable, use_context, create_signal, Signal};
use petgraph::stable_graph::NodeIndex;
use crate::models::{RailwayGraph, Line, Stations, ProjectSettings, UserSettings};
use crate::components::label_position_grid::LabelPositionGrid;

const SELECTION_PADDING: f64 = 20.0;

pub fn delete_selected_stations(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    set_selected_stations: WriteSignal<Vec<NodeIndex>>,
) {
    let stations = selected_stations.get();
    if stations.is_empty() {
        return;
    }

    let mut current_graph = graph.get();
    let mut current_lines = lines.get();

    for &station_idx in &stations {
        let (removed_edges, bypass_mapping) = current_graph.delete_station(station_idx);

        for line in &mut current_lines {
            line.update_route_after_deletion(&removed_edges, &bypass_mapping);
        }
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
    set_selected_stations.set(Vec::new());
}

pub fn add_platform_to_selected(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) {
    let stations = selected_stations.get();
    if stations.is_empty() {
        return;
    }

    let mut current_graph = graph.get();

    for &station_idx in &stations {
        if let Some(node) = current_graph.graph.node_weight_mut(station_idx) {
            if let Some(station) = node.as_station_mut() {
                let next_num = station.platforms.len() + 1;
                station.platforms.push(crate::models::Platform {
                    name: next_num.to_string(),
                });
            }
        }
    }

    set_graph.set(current_graph);
}

pub fn remove_platform_from_selected(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) {
    let stations = selected_stations.get();
    if stations.is_empty() {
        return;
    }

    let mut current_graph = graph.get();

    for &station_idx in &stations {
        if let Some(node) = current_graph.graph.node_weight_mut(station_idx) {
            if let Some(station) = node.as_station_mut() {
                if station.platforms.len() > 1 {
                    station.platforms.pop();
                }
            }
        }
    }

    set_graph.set(current_graph);
}

fn add_track_to_edge(
    graph: &mut RailwayGraph,
    lines: &mut [Line],
    from: NodeIndex,
    to: NodeIndex,
    handedness: crate::models::TrackHandedness,
) {
    if let Some(edge_idx) = graph.graph.find_edge(from, to) {
        if let Some(segment) = graph.graph.edge_weight_mut(edge_idx) {
            let new_count = segment.tracks.len() + 1;
            segment.tracks = crate::import::create_tracks_with_count(new_count, handedness);

            // Fix track indices in affected lines
            let edge_index = edge_idx.index();
            for line in lines {
                line.fix_track_indices_after_change(edge_index, new_count, graph);
            }
        }
    }
}

pub fn add_tracks_between_selected(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    settings: ReadSignal<ProjectSettings>,
) {
    let stations = selected_stations.get();
    if stations.len() < 2 {
        return;
    }

    let mut current_graph = graph.get();
    let mut current_lines = lines.get();
    let handedness = settings.get().track_handedness;

    // Add a track to existing edges between all pairs of selected stations
    for i in 0..stations.len() {
        for j in i + 1..stations.len() {
            let from = stations[i];
            let to = stations[j];

            // Check both directions for existing edges
            add_track_to_edge(&mut current_graph, &mut current_lines, from, to, handedness);
            add_track_to_edge(&mut current_graph, &mut current_lines, to, from, handedness);
        }
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
}

fn remove_last_track_from_edge(
    graph: &mut RailwayGraph,
    lines: &mut [Line],
    from: NodeIndex,
    to: NodeIndex,
    handedness: crate::models::TrackHandedness,
) {
    if let Some(edge_idx) = graph.graph.find_edge(from, to) {
        if let Some(segment) = graph.graph.edge_weight_mut(edge_idx) {
            if segment.tracks.len() > 1 {
                let new_count = segment.tracks.len() - 1;
                segment.tracks = crate::import::create_tracks_with_count(new_count, handedness);

                // Fix track indices in affected lines
                let edge_index = edge_idx.index();
                for line in lines {
                    line.fix_track_indices_after_change(edge_index, new_count, graph);
                }
            }
        }
    }
}

pub fn remove_tracks_between_selected(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    set_lines: WriteSignal<Vec<Line>>,
    settings: ReadSignal<ProjectSettings>,
) {
    let stations = selected_stations.get();
    if stations.len() < 2 {
        return;
    }

    let mut current_graph = graph.get();
    let mut current_lines = lines.get();
    let handedness = settings.get().track_handedness;

    // Remove last track from existing edges between all pairs of selected stations
    for i in 0..stations.len() {
        for j in i + 1..stations.len() {
            let from = stations[i];
            let to = stations[j];

            // Check both directions for existing edges
            remove_last_track_from_edge(&mut current_graph, &mut current_lines, from, to, handedness);
            remove_last_track_from_edge(&mut current_graph, &mut current_lines, to, from, handedness);
        }
    }

    set_graph.set(current_graph);
    set_lines.set(current_lines);
}

/// Recalculate selection bounds based on current station positions
pub fn update_selection_bounds(
    graph: &RailwayGraph,
    stations: &[NodeIndex],
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
) {
    if stations.is_empty() {
        set_selection_bounds.set(None);
        return;
    }

    let positions: Vec<(f64, f64)> = stations.iter()
        .filter_map(|&idx| graph.get_station_position(idx))
        .collect();

    if positions.is_empty() {
        set_selection_bounds.set(None);
        return;
    }

    let (first_x, first_y) = positions[0];
    let mut min_x = first_x;
    let mut max_x = first_x;
    let mut min_y = first_y;
    let mut max_y = first_y;

    for &(x, y) in &positions[1..] {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }

    // Add padding to ensure bounds are always clickable, especially for linear selections
    min_x -= SELECTION_PADDING;
    max_x += SELECTION_PADDING;
    min_y -= SELECTION_PADDING;
    max_y += SELECTION_PADDING;

    set_selection_bounds.set(Some((min_x, max_x, min_y, max_y)));
}

#[allow(clippy::cast_precision_loss)]
pub fn align_selected_stations(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
    settings: ReadSignal<crate::models::ProjectSettings>,
) {
    use std::collections::HashSet;

    let stations = selected_stations.get();
    if stations.len() < 2 {
        return;
    }

    let mut current_graph = graph.get();
    let station_set: HashSet<NodeIndex> = stations.iter().copied().collect();

    // Find a starting node - prefer nodes with exactly one selected neighbor (true endpoint)
    let start = stations.iter()
        .find(|&&s| {
            current_graph.graph.neighbors(s)
                .filter(|n| station_set.contains(n))
                .count() == 1
        })
        .or_else(|| {
            // Fallback: pick node with fewest selected neighbors
            stations.iter()
                .min_by_key(|&&s| {
                    current_graph.graph.neighbors(s)
                        .filter(|n| station_set.contains(n))
                        .count()
                })
        })
        .copied();

    let Some(start) = start else {
        return;
    };

    // DFS traversal from the endpoint following the path
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();
    let mut current = start;

    ordered.push(current);
    seen.insert(current);

    // Follow the path by always picking the next unvisited neighbor
    loop {
        let next = current_graph.graph.neighbors(current)
            .find(|n| station_set.contains(n) && !seen.contains(n));

        let Some(next_node) = next else {
            break;
        };

        ordered.push(next_node);
        seen.insert(next_node);
        current = next_node;
    }

    // Add any disconnected stations at the end
    for &station in &stations {
        if !seen.contains(&station) {
            ordered.push(station);
        }
    }

    // Get positions in order
    let positions: Vec<(f64, f64)> = ordered.iter()
        .filter_map(|&idx| current_graph.get_station_position(idx))
        .collect();

    if positions.is_empty() {
        return;
    }

    if positions.len() < 2 {
        return;
    }

    // Calculate the current angle of the line (from first to last station)
    let first_pos = positions[0];
    let last_pos = positions[positions.len() - 1];
    let dx = last_pos.0 - first_pos.0;
    let dy = last_pos.1 - first_pos.1;
    let current_angle = dy.atan2(dx);

    // Determine if current alignment is close to a 45° increment
    let angle_deg = current_angle.to_degrees();
    let normalized_angle = ((angle_deg % 360.0) + 360.0) % 360.0;

    // Check if close to 0°, 45°, 90°, 135°, 180°, 225°, 270°, or 315°
    let target_angles = [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0];
    let angle_tolerance = 5.0; // degrees

    let aligned_angle = target_angles.iter()
        .find(|&&target| (normalized_angle - target).abs() < angle_tolerance || (normalized_angle - target - 360.0).abs() < angle_tolerance)
        .copied();

    let use_angle = aligned_angle.unwrap_or_else(|| {
        // Not aligned to 45° increment - choose horizontal or vertical based on variance
        let count = positions.len();
        let mean_x: f64 = positions.iter().map(|(x, _)| x).sum::<f64>() / count as f64;
        let mean_y: f64 = positions.iter().map(|(_, y)| y).sum::<f64>() / count as f64;

        let variance_x: f64 = positions.iter()
            .map(|(x, _)| (x - mean_x).powi(2))
            .sum::<f64>() / count as f64;
        let variance_y: f64 = positions.iter()
            .map(|(_, y)| (y - mean_y).powi(2))
            .sum::<f64>() / count as f64;

        if variance_x < variance_y { 90.0 } else { 0.0 }
    });

    // Calculate line direction vector
    let line_angle_rad = use_angle.to_radians();
    let dir_x = line_angle_rad.cos();
    let dir_y = line_angle_rad.sin();

    // Snap the first position to grid
    let snapped_first = crate::components::infrastructure_canvas::auto_layout::snap_to_grid(first_pos.0, first_pos.1);

    // Calculate required spacing based on project settings
    // For 0°/180° (horizontal) or 90°/270° (vertical): grid_squares * 30
    // For 45° angles: grid_squares * 30 * sqrt(2)
    let grid_size = 30.0;
    let grid_squares = settings.get().default_node_distance_grid_squares;
    let spacing = if (use_angle % 90.0).abs() < 0.1 {
        // Horizontal or vertical
        grid_size * grid_squares
    } else {
        // 45° diagonal
        grid_size * grid_squares * 2.0_f64.sqrt()
    };

    // Position stations evenly spaced along the line (skip passing loops - they auto-position)
    let mut non_passing_count = 0;
    for &station_idx in &ordered {
        // Check if this is a passing loop
        let is_passing_loop = current_graph.graph.node_weight(station_idx)
            .and_then(|n| n.as_station())
            .is_some_and(|s| s.passing_loop);

        if !is_passing_loop {
            let distance_along_line = f64::from(non_passing_count) * spacing;
            let new_x = snapped_first.0 + distance_along_line * dir_x;
            let new_y = snapped_first.1 + distance_along_line * dir_y;
            current_graph.set_station_position(station_idx, (new_x, new_y));
            non_passing_count += 1;
        }
    }

    set_graph.set(current_graph.clone());

    // Recalculate bounds after alignment
    update_selection_bounds(&current_graph, &stations, set_selection_bounds);
}

#[allow(clippy::cast_precision_loss)]
fn rotate_stations_by_angle(
    stations: &[NodeIndex],
    graph: &mut RailwayGraph,
    angle_degrees: f64,
) {
    if stations.len() < 2 {
        return;
    }

    let mut positions = Vec::new();

    // Collect all positions
    for &station_idx in stations {
        if let Some((x, y)) = graph.get_station_position(station_idx) {
            positions.push((station_idx, x, y));
        }
    }

    if positions.len() < 2 {
        return;
    }

    // Calculate centroid
    let count = positions.len();
    let center_x: f64 = positions.iter().map(|(_, x, _)| x).sum::<f64>() / count as f64;
    let center_y: f64 = positions.iter().map(|(_, _, y)| y).sum::<f64>() / count as f64;

    // Check if stations are aligned (collinear within tolerance)
    let collinearity_threshold: f64 = 0.1;
    let is_aligned = if positions.len() == 2 {
        true // Two points are always collinear
    } else {
        // Check if all points lie on the same line
        let (_, x1, y1) = positions[0];
        let (_, x2, y2) = positions[1];

        positions[2..].iter().all(|(_, x, y)| {
            // Calculate perpendicular distance from point to line
            let dx = x2 - x1;
            let dy = y2 - y1;
            let line_length_sq = dx * dx + dy * dy;

            if line_length_sq < 0.001 {
                return true; // Points are too close
            }

            let distance = ((dy * (x - x1) - dx * (y - y1)).abs()) / line_length_sq.sqrt();
            distance < collinearity_threshold
        })
    };

    if is_aligned {
        // Stations are aligned - maintain alignment during rotation
        // Find the two endpoints (farthest apart)
        let mut max_dist_sq = 0.0;
        let mut endpoint1_idx = 0;
        let mut endpoint2_idx = 1;

        for i in 0..positions.len() {
            for j in (i + 1)..positions.len() {
                let dx = positions[i].1 - positions[j].1;
                let dy = positions[i].2 - positions[j].2;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > max_dist_sq {
                    max_dist_sq = dist_sq;
                    endpoint1_idx = i;
                    endpoint2_idx = j;
                }
            }
        }

        let (_idx1, x1, y1) = positions[endpoint1_idx];
        let (_idx2, x2, y2) = positions[endpoint2_idx];

        // Rotate the two endpoints
        let angle = angle_degrees.to_radians();
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        let dx1 = x1 - center_x;
        let dy1 = y1 - center_y;
        let endpoint1_rotated_x = center_x + dx1 * cos_angle - dy1 * sin_angle;
        let endpoint1_rotated_y = center_y + dx1 * sin_angle + dy1 * cos_angle;

        let dx2 = x2 - center_x;
        let dy2 = y2 - center_y;
        let endpoint2_rotated_x = center_x + dx2 * cos_angle - dy2 * sin_angle;
        let endpoint2_rotated_y = center_y + dx2 * sin_angle + dy2 * cos_angle;

        // Snap the first endpoint to grid
        let (snapped_endpoint1_x, snapped_endpoint1_y) = crate::components::infrastructure_canvas::auto_layout::snap_to_grid(
            endpoint1_rotated_x, endpoint1_rotated_y
        );

        // Calculate the angle and distance from endpoint1 to endpoint2
        let dx = endpoint2_rotated_x - endpoint1_rotated_x;
        let dy = endpoint2_rotated_y - endpoint1_rotated_y;
        let line_length = (dx * dx + dy * dy).sqrt();
        let line_angle = dy.atan2(dx);

        // Snap angle to nearest 45° increment (0°, 45°, 90°, 135°, 180°, -45°, -90°, -135°)
        let angle_increment = std::f64::consts::PI / 4.0; // 45 degrees in radians
        let snapped_angle = (line_angle / angle_increment).round() * angle_increment;

        // Calculate endpoint2 position at the snapped angle, then snap it to grid
        let rotated_ep2_x = snapped_endpoint1_x + line_length * snapped_angle.cos();
        let rotated_ep2_y = snapped_endpoint1_y + line_length * snapped_angle.sin();
        let (snapped_endpoint2_x, snapped_endpoint2_y) = crate::components::infrastructure_canvas::auto_layout::snap_to_grid(
            rotated_ep2_x, rotated_ep2_y
        );

        // Calculate each station's position along the original line (0.0 to 1.0)
        let mut station_positions: Vec<(NodeIndex, f64)> = Vec::new();

        for &(idx, x, y) in &positions {
            let dx = x - x1;
            let dy = y - y1;
            let t = ((dx * (x2 - x1) + dy * (y2 - y1)) / max_dist_sq).clamp(0.0, 1.0);
            station_positions.push((idx, t));
        }

        // Position stations along the snapped rotated line
        for (idx, t) in station_positions {
            let new_x = snapped_endpoint1_x + t * (snapped_endpoint2_x - snapped_endpoint1_x);
            let new_y = snapped_endpoint1_y + t * (snapped_endpoint2_y - snapped_endpoint1_y);
            graph.set_station_position(idx, (new_x, new_y));
        }
    } else {
        // Stations are not aligned - rotate and snap to maintain grid alignment
        let angle = angle_degrees.to_radians();
        let cos_angle = angle.cos();
        let sin_angle = angle.sin();

        // First pass: calculate all rotated positions
        let mut rotated_positions = Vec::new();
        for (station_idx, x, y) in &positions {
            let dx = x - center_x;
            let dy = y - center_y;

            // Rotation matrix (accounting for canvas Y-axis pointing down):
            // x' = x*cos(θ) - y*sin(θ)
            // y' = x*sin(θ) + y*cos(θ)
            let new_x = center_x + dx * cos_angle - dy * sin_angle;
            let new_y = center_y + dx * sin_angle + dy * cos_angle;

            rotated_positions.push((*station_idx, new_x, new_y));
        }

        // Second pass: snap first node to grid and calculate offset
        if let Some((_first_idx, first_x, first_y)) = rotated_positions.first() {
            let (snapped_first_x, snapped_first_y) =
                crate::components::infrastructure_canvas::auto_layout::snap_to_grid(*first_x, *first_y);

            // Calculate the offset from snapping
            let snap_offset_x = snapped_first_x - first_x;
            let snap_offset_y = snapped_first_y - first_y;

            // Apply rotation with snap offset to all nodes
            for (station_idx, x, y) in rotated_positions {
                graph.set_station_position(station_idx, (x + snap_offset_x, y + snap_offset_y));
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn rotate_selected_stations_clockwise(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
) {
    let stations = selected_stations.get();
    let mut current_graph = graph.get();
    rotate_stations_by_angle(&stations, &mut current_graph, 45.0);

    // Recalculate bounds after rotation
    update_selection_bounds(&current_graph, &stations, set_selection_bounds);

    set_graph.set(current_graph);
}

#[allow(clippy::cast_precision_loss)]
pub fn rotate_selected_stations_counterclockwise(
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    set_selection_bounds: WriteSignal<Option<(f64, f64, f64, f64)>>,
) {
    let stations = selected_stations.get();
    let mut current_graph = graph.get();
    rotate_stations_by_angle(&stations, &mut current_graph, -45.0);

    // Recalculate bounds after rotation
    update_selection_bounds(&current_graph, &stations, set_selection_bounds);

    set_graph.set(current_graph);
}

pub fn set_label_position_for_selected(
    selected_nodes: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
    label_position: Option<crate::components::infrastructure_canvas::station_renderer::LabelPosition>,
) {
    let nodes = selected_nodes.get();
    if nodes.is_empty() {
        return;
    }

    let mut current_graph = graph.get();

    for &node_idx in &nodes {
        if let Some(node) = current_graph.graph.node_weight_mut(node_idx) {
            match node {
                crate::models::Node::Station(station) => {
                    station.label_position = label_position;
                }
                crate::models::Node::Junction(junction) => {
                    junction.label_position = label_position;
                }
            }
        }
    }

    set_graph.set(current_graph);
}

#[component]
#[must_use]
#[allow(clippy::similar_names)]
#[allow(clippy::too_many_lines)]
pub fn MultiSelectToolbar(
    /// Selected stations
    selected_stations: ReadSignal<Vec<NodeIndex>>,
    /// Selection box start (if Some, user is currently selecting)
    selection_box_start: ReadSignal<Option<(f64, f64)>>,
    /// Graph to calculate centroid position
    graph: ReadSignal<RailwayGraph>,
    /// Zoom level for positioning
    zoom: ReadSignal<f64>,
    /// Pan offset X
    pan_x: ReadSignal<f64>,
    /// Pan offset Y
    pan_y: ReadSignal<f64>,
    /// Callback for Rotate Clockwise operation
    #[prop(optional)]
    on_rotate_cw: Option<Callback<()>>,
    /// Callback for Rotate Counter-Clockwise operation
    #[prop(optional)]
    on_rotate_ccw: Option<Callback<()>>,
    /// Callback for Align operation
    #[prop(optional)]
    on_align: Option<Callback<()>>,
    /// Callback for Add Platform operation
    #[prop(optional)]
    on_add_platform: Option<Callback<()>>,
    /// Callback for Remove Platform operation
    #[prop(optional)]
    on_remove_platform: Option<Callback<()>>,
    /// Callback for Add Track operation
    #[prop(optional)]
    on_add_track: Option<Callback<()>>,
    /// Callback for Remove Track operation
    #[prop(optional)]
    on_remove_track: Option<Callback<()>>,
    /// Callback for Delete operation
    #[prop(optional)]
    on_delete: Option<Callback<()>>,
    /// Callback for Set Label Position operation
    #[prop(optional)]
    on_set_label_position: Option<Callback<Option<crate::components::infrastructure_canvas::station_renderer::LabelPosition>>>,
) -> impl IntoView {
    // Calculate toolbar position based on selected stations centroid
    let toolbar_position = move || {
        let stations = selected_stations.get();
        if stations.is_empty() {
            return (0.0, 0.0);
        }

        let current_graph = graph.get();
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut count = 0;

        for &idx in &stations {
            if let Some((x, y)) = current_graph.get_station_position(idx) {
                sum_x += x;
                sum_y += y;
                count += 1;
            }
        }

        if count == 0 {
            return (0.0, 0.0);
        }

        // Use centroid for stable positioning during rotation
        let center_x = sum_x / f64::from(count);
        let center_y = sum_y / f64::from(count);

        // Convert world coordinates to screen coordinates
        let zoom_val = zoom.get();
        let pan_x_offset = pan_x.get();
        let pan_y_offset = pan_y.get();
        let screen_x = center_x * zoom_val + pan_x_offset;
        let screen_y = center_y * zoom_val + pan_y_offset;

        (screen_x, screen_y)
    };

    // Get user settings to format shortcuts
    let (user_settings, _) = use_context::<(ReadSignal<UserSettings>, WriteSignal<UserSettings>)>()
        .expect("UserSettings context not found");

    // Helper to format title with shortcut hint
    let format_title_with_shortcut = move |base_title: String, shortcut_id: &str| -> String {
        user_settings.with(|settings| {
            if let Some(Some(shortcut)) = settings.keyboard_shortcuts.shortcuts.get(shortcut_id) {
                let is_mac = crate::models::is_mac_platform();
                let is_windows = cfg!(target_os = "windows");
                let shortcut_text = shortcut.format(is_mac, is_windows);
                if !shortcut_text.is_empty() {
                    return format!("{base_title} ({shortcut_text})");
                }
            }
            base_title
        })
    };

    // State for label position grid
    let (label_grid_open, set_label_grid_open) = create_signal(false);

    // Calculate current label position state for selected nodes
    let label_position_state = move || {
        use crate::components::label_position_grid::LabelPositionState;

        let stations = selected_stations.get();
        if stations.is_empty() {
            return LabelPositionState::Auto;
        }

        let current_graph = graph.get();
        let mut positions: Vec<Option<crate::components::infrastructure_canvas::station_renderer::LabelPosition>> = Vec::new();

        for &idx in &stations {
            if let Some(node) = current_graph.graph.node_weight(idx) {
                let pos = match node {
                    crate::models::Node::Station(station) => station.label_position,
                    crate::models::Node::Junction(junction) => junction.label_position,
                };
                positions.push(pos);
            }
        }

        if positions.is_empty() {
            return LabelPositionState::Auto;
        }

        // Check if all positions are the same
        let first = positions[0];
        let all_same = positions.iter().all(|&p| p == first);

        if all_same {
            match first {
                Some(pos) => LabelPositionState::Single(pos),
                None => LabelPositionState::Auto,
            }
        } else {
            LabelPositionState::Mixed
        }
    };

    view! {
        {move || {
            let stations = selected_stations.get();
            // Don't show toolbar if empty or currently selecting
            if stations.is_empty() || selection_box_start.get().is_some() {
                view! { <></> };
                return ().into_view();
            }

            let (x, y) = toolbar_position();
            let count = stations.len();

            view! {
                <div
                    class="multi-select-toolbar"
                    style:left=format!("{}px", x)
                    style:top=format!("{}px", y - 5.0)
                >
                    <button
                        class="toolbar-button"
                        title=format_title_with_shortcut(
                            format!("Rotate {} station{} counter-clockwise 45°", count, if count == 1 { "" } else { "s" }),
                            "multi_select_rotate_ccw"
                        )
                        on:click=move |_| {
                            if let Some(callback) = on_rotate_ccw {
                                callback.call(());
                            }
                        }
                    >
                        <i class="fa-solid fa-rotate-left"></i>
                    </button>
                    <button
                        class="toolbar-button"
                        title=format_title_with_shortcut(
                            format!("Rotate {} station{} clockwise 45°", count, if count == 1 { "" } else { "s" }),
                            "multi_select_rotate_cw"
                        )
                        on:click=move |_| {
                            if let Some(callback) = on_rotate_cw {
                                callback.call(());
                            }
                        }
                    >
                        <i class="fa-solid fa-rotate-right"></i>
                    </button>
                    <button
                        class="toolbar-button"
                        title=format_title_with_shortcut(
                            format!("Align {} station{} horizontally or vertically", count, if count == 1 { "" } else { "s" }),
                            "multi_select_align"
                        )
                        on:click=move |_| {
                            if let Some(callback) = on_align {
                                callback.call(());
                            }
                        }
                    >
                        <i class="fa-solid fa-align-center"></i>
                    </button>
                    <div class="dropdown-wrapper">
                        <button
                            class="toolbar-button"
                            title=format_title_with_shortcut(
                                format!("Set label position for {} node{}", count, if count == 1 { "" } else { "s" }),
                                "multi_select_label_position"
                            )
                            on:click=move |_| {
                                set_label_grid_open.set(!label_grid_open.get());
                            }
                        >
                            {move || {
                                use crate::components::label_position_grid::LabelPositionState;
                                match label_position_state() {
                                    LabelPositionState::Single(pos) => {
                                        use crate::components::infrastructure_canvas::station_renderer::LabelPosition;
                                        match pos {
                                            LabelPosition::TopLeft => view! { <span>"↖"</span> }.into_view(),
                                            LabelPosition::Top => view! { <span>"↑"</span> }.into_view(),
                                            LabelPosition::TopRight => view! { <span>"↗"</span> }.into_view(),
                                            LabelPosition::Left => view! { <span>"←"</span> }.into_view(),
                                            LabelPosition::Right => view! { <span>"→"</span> }.into_view(),
                                            LabelPosition::BottomLeft => view! { <span>"↙"</span> }.into_view(),
                                            LabelPosition::Bottom => view! { <span>"↓"</span> }.into_view(),
                                            LabelPosition::BottomRight => view! { <span>"↘"</span> }.into_view(),
                                        }
                                    }
                                    LabelPositionState::Auto => view! { <span>"⊙"</span> }.into_view(),
                                    LabelPositionState::Mixed => view! { <i class="fa-solid fa-tag"></i> }.into_view(),
                                }
                            }}
                        </button>

                        <LabelPositionGrid
                            is_open=Signal::derive(move || label_grid_open.get())
                            on_select=Callback::new(move |pos: Option<crate::components::infrastructure_canvas::station_renderer::LabelPosition>| {
                                if let Some(callback) = on_set_label_position {
                                    callback.call(pos);
                                }
                            })
                            current_state=Signal::derive(label_position_state)
                        />
                    </div>

                    <div class="toolbar-divider"></div>

                    <div class="toolbar-section">
                        <div class="toolbar-section-icon">
                            <i class="fa-solid fa-person-walking-luggage"></i>
                        </div>
                        <button
                            class="toolbar-button"
                            title=format_title_with_shortcut(
                                format!("Add platform to {} station{}", count, if count == 1 { "" } else { "s" }),
                                "multi_select_add_platform"
                            )
                            on:click=move |_| {
                                if let Some(callback) = on_add_platform {
                                    callback.call(());
                                }
                            }
                        >
                            <i class="fa-solid fa-plus"></i>
                        </button>
                        <button
                            class="toolbar-button"
                            title=format_title_with_shortcut(
                                format!("Remove last platform from {} station{}", count, if count == 1 { "" } else { "s" }),
                                "multi_select_remove_platform"
                            )
                            on:click=move |_| {
                                if let Some(callback) = on_remove_platform {
                                    callback.call(());
                                }
                            }
                        >
                            <i class="fa-solid fa-minus"></i>
                        </button>
                    </div>

                    <div class="toolbar-divider"></div>

                    <div class="toolbar-section">
                        <div class="toolbar-section-icon rail-icon"></div>
                        <button
                            class="toolbar-button"
                            title=format_title_with_shortcut(
                                "Add track between selected stations".to_string(),
                                "multi_select_add_track"
                            )
                            on:click=move |_| {
                                if let Some(callback) = on_add_track {
                                    callback.call(());
                                }
                            }
                        >
                            <i class="fa-solid fa-plus"></i>
                        </button>
                        <button
                            class="toolbar-button"
                            title=format_title_with_shortcut(
                                "Remove track between selected stations".to_string(),
                                "multi_select_remove_track"
                            )
                            on:click=move |_| {
                                if let Some(callback) = on_remove_track {
                                    callback.call(());
                                }
                            }
                        >
                            <i class="fa-solid fa-minus"></i>
                        </button>
                    </div>

                    <div class="toolbar-divider"></div>

                    <button
                        class="toolbar-button toolbar-button-danger"
                        title=format_title_with_shortcut(
                            format!("Delete {} station{}", count, if count == 1 { "" } else { "s" }),
                            "multi_select_delete"
                        )
                        on:click=move |_| {
                            if let Some(callback) = on_delete {
                                callback.call(());
                            }
                        }
                    >
                        <i class="fa-solid fa-trash"></i>
                    </button>
                </div>
            }.into_view()
        }}
    }
}
