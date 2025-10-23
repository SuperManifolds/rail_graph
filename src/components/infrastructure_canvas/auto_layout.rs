use crate::models::{RailwayGraph, Stations};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::HashSet;

const STATION_SPACING: f64 = 60.0;
const GRID_SIZE: f64 = 30.0;

/// Snap coordinates to grid intersections
#[must_use]
pub fn snap_to_grid(x: f64, y: f64) -> (f64, f64) {
    let snapped_x = (x / GRID_SIZE).round() * GRID_SIZE;
    let snapped_y = (y / GRID_SIZE).round() * GRID_SIZE;
    (snapped_x, snapped_y)
}

/// Snap a station and its branch to the nearest 45-degree angle from its parent
pub fn snap_to_angle(graph: &mut RailwayGraph, station_idx: NodeIndex, target_x: f64, target_y: f64) {
    // Find the nearest junction (node with 3+ neighbors) by walking from station_idx
    let junction_idx = find_nearest_junction(graph, station_idx);

    let Some(junction_pos) = graph.get_station_position(junction_idx) else {
        return;
    };

    // Calculate target snapped angle from junction to drop position
    let dx = target_x - junction_pos.0;
    let dy = target_y - junction_pos.1;
    let target_angle = dy.atan2(dx);
    let target_angle_deg = target_angle.to_degrees();
    let snapped_angle_deg = (target_angle_deg / 45.0).round() * 45.0;
    let snapped_angle = snapped_angle_deg.to_radians();

    // Get all neighbors of the junction
    let neighbors: Vec<_> = graph.graph.edges(junction_idx)
        .map(|e| e.target())
        .chain(graph.graph.edges_directed(junction_idx, Direction::Incoming)
            .map(|e| e.source()))
        .collect();

    if neighbors.is_empty() {
        return;
    }

    // Find which branch contains station_idx
    let mut branch_to_move: Option<NodeIndex> = None;
    for &neighbor in &neighbors {
        let mut visited = HashSet::new();
        visited.insert(junction_idx);
        if contains_node(graph, neighbor, station_idx, &mut visited) {
            branch_to_move = Some(neighbor);
            break;
        }
    }

    let Some(branch_to_move) = branch_to_move else {
        return;
    };

    // Position the first node of the branch at the snapped angle from junction
    let new_x = junction_pos.0 + snapped_angle.cos() * STATION_SPACING;
    let new_y = junction_pos.1 + snapped_angle.sin() * STATION_SPACING;
    let (new_x, new_y) = snap_to_grid(new_x, new_y);

    // Save the branch angle for future auto layout runs
    graph.branch_angles.insert((junction_idx.index(), branch_to_move.index()), snapped_angle);

    // Realign the entire branch
    let mut visited = HashSet::new();
    visited.insert(junction_idx);

    // Mark all other branches as visited so we don't move them
    for &neighbor in &neighbors {
        if neighbor != branch_to_move {
            visited.insert(neighbor);
        }
    }

    realign_branch(graph, branch_to_move, (new_x, new_y), snapped_angle, &mut visited);
}

/// Snap a station to a grid-aligned position along its current branch direction
/// Does not move other stations or rotate the branch
#[allow(clippy::similar_names)]
pub fn snap_station_along_branch(graph: &mut RailwayGraph, station_idx: NodeIndex, target_x: f64, target_y: f64) {
    // Get current position
    let Some(current_pos) = graph.get_station_position(station_idx) else {
        return;
    };

    // Get neighbors to determine branch direction
    let neighbors: Vec<_> = graph.graph.edges(station_idx)
        .map(|e| (e.target(), graph.get_station_position(e.target())))
        .chain(graph.graph.edges_directed(station_idx, Direction::Incoming)
            .map(|e| (e.source(), graph.get_station_position(e.source()))))
        .collect();

    if neighbors.is_empty() {
        // No neighbors, just snap to grid
        let snapped = snap_to_grid(target_x, target_y);
        graph.set_station_position(station_idx, snapped);
        return;
    }

    // Calculate the average branch direction from positioned neighbors
    let mut total_dx = 0.0;
    let mut total_dy = 0.0;
    let mut count = 0;

    for (_, neighbor_pos_opt) in &neighbors {
        if let Some(neighbor_pos) = neighbor_pos_opt {
            let dx = neighbor_pos.0 - current_pos.0;
            let dy = neighbor_pos.1 - current_pos.1;
            if dx.abs() > 0.01 || dy.abs() > 0.01 {
                // Normalize
                let len = (dx * dx + dy * dy).sqrt();
                total_dx += dx / len;
                total_dy += dy / len;
                count += 1;
            }
        }
    }

    if count == 0 {
        // No positioned neighbors with meaningful distance, just snap to grid
        let snapped = snap_to_grid(target_x, target_y);
        graph.set_station_position(station_idx, snapped);
        return;
    }

    // Average direction
    let avg_dx = total_dx / f64::from(count);
    let avg_dy = total_dy / f64::from(count);

    // Normalize to get unit direction vector
    let dir_len = (avg_dx * avg_dx + avg_dy * avg_dy).sqrt();

    if dir_len < 0.01 {
        // Direction vectors cancelled out (e.g., node between two others on a line)
        // Just snap to grid without projection
        let snapped = snap_to_grid(target_x, target_y);
        graph.set_station_position(station_idx, snapped);
        return;
    }

    let dir_x = avg_dx / dir_len;
    let dir_y = avg_dy / dir_len;

    // Project target position onto the line through current_pos in direction (dir_x, dir_y)
    // Line: P = current_pos + t * direction
    // Find t where the projected point is closest to (target_x, target_y)
    let to_target_x = target_x - current_pos.0;
    let to_target_y = target_y - current_pos.1;
    let t = to_target_x * dir_x + to_target_y * dir_y;

    let projected_x = current_pos.0 + t * dir_x;
    let projected_y = current_pos.1 + t * dir_y;

    // Snap to grid
    let snapped = snap_to_grid(projected_x, projected_y);
    graph.set_station_position(station_idx, snapped);
}

/// Find the nearest junction (node with 3+ neighbors) starting from `start_node`
/// If no junction found, returns `start_node`
fn find_nearest_junction(graph: &RailwayGraph, start_node: NodeIndex) -> NodeIndex {
    let mut queue = std::collections::VecDeque::new();
    let mut visited = HashSet::new();

    queue.push_back(start_node);
    visited.insert(start_node);

    while let Some(node) = queue.pop_front() {
        let neighbor_count = graph.graph.edges(node).count()
            + graph.graph.edges_directed(node, Direction::Incoming).count();

        if neighbor_count >= 3 {
            return node;
        }

        // Add neighbors to queue
        for edge in graph.graph.edges(node) {
            let target = edge.target();
            if visited.insert(target) {
                queue.push_back(target);
            }
        }
        for edge in graph.graph.edges_directed(node, Direction::Incoming) {
            let source = edge.source();
            if visited.insert(source) {
                queue.push_back(source);
            }
        }
    }

    start_node
}

/// Check if a branch starting from `start_node` contains `target_node`
fn contains_node(
    graph: &RailwayGraph,
    start_node: NodeIndex,
    target_node: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
) -> bool {
    if start_node == target_node {
        return true;
    }

    if visited.contains(&start_node) {
        return false;
    }

    visited.insert(start_node);

    let neighbors: Vec<_> = graph.graph.edges(start_node)
        .map(|e| e.target())
        .chain(graph.graph.edges_directed(start_node, Direction::Incoming)
            .map(|e| e.source()))
        .filter(|&n| !visited.contains(&n))
        .collect();

    for neighbor in neighbors {
        if contains_node(graph, neighbor, target_node, visited) {
            return true;
        }
    }

    false
}

/// Recursively realign a branch in the given direction
fn realign_branch(
    graph: &mut RailwayGraph,
    current_node: NodeIndex,
    position: (f64, f64),
    direction: f64,
    visited: &mut HashSet<NodeIndex>,
) {
    if visited.contains(&current_node) {
        return;
    }

    let snapped_position = snap_to_grid(position.0, position.1);
    graph.set_station_position(current_node, snapped_position);
    visited.insert(current_node);

    // Get all unvisited neighbors
    let neighbors: Vec<_> = graph.graph.edges(current_node)
        .map(|e| e.target())
        .chain(graph.graph.edges_directed(current_node, Direction::Incoming)
            .map(|e| e.source()))
        .filter(|&n| !visited.contains(&n))
        .collect();

    // Continue in the same direction for all neighbors
    for neighbor in neighbors {
        let next_pos = (
            position.0 + direction.cos() * STATION_SPACING,
            position.1 + direction.sin() * STATION_SPACING,
        );
        realign_branch(graph, neighbor, next_pos, direction, visited);
    }
}

pub fn apply_layout(graph: &mut RailwayGraph, height: f64) {
    use crate::models::Junctions;

    let start_x = 150.0;
    let start_y = height / 2.0;

    // Find a starting node - prefer endpoints (nodes with only 1 connection)
    let Some(start_node) = graph
        .graph
        .node_indices()
        .min_by_key(|&idx| {
            let outgoing = graph.graph.edges(idx).count();
            let incoming = graph.graph.edges_directed(idx, Direction::Incoming).count();
            let total = outgoing + incoming;
            // Prefer endpoints (1 connection), then nodes with fewer connections
            if total == 1 { 0 } else { total }
        }) else {
            return; // No nodes in graph
        };

    let mut visited = HashSet::new();
    let mut available_directions = vec![
        0.0,                                    // Right
        std::f64::consts::PI / 4.0,            // Down-right
        -std::f64::consts::PI / 4.0,           // Up-right
        std::f64::consts::PI / 2.0,            // Down
        3.0 * std::f64::consts::PI / 4.0,      // Down-left
        -3.0 * std::f64::consts::PI / 4.0,     // Up-left
    ];

    // Layout the main line and branches
    layout_line(
        graph,
        start_node,
        (start_x, start_y),
        -std::f64::consts::PI / 2.0, // Start going up/north
        STATION_SPACING,
        &mut visited,
        &mut available_directions,
    );

    // Handle any disconnected nodes - collect first to avoid borrow checker issues
    let disconnected_nodes: Vec<_> = graph.graph.node_indices()
        .filter(|idx| !visited.contains(idx))
        .collect();

    let mut offset_y = start_y + 100.0;
    for idx in disconnected_nodes {
        let position = snap_to_grid(start_x + 200.0, offset_y);
        graph.set_station_position(idx, position);
        offset_y += STATION_SPACING;
    }

    // Interpolate positions for junctions without explicit positions
    let junction_indices: Vec<_> = graph.graph.node_indices()
        .filter(|&idx| graph.is_junction(idx))
        .collect();

    for junction_idx in junction_indices {
        graph.interpolate_junction_position(junction_idx, false);
    }
}

/// Smart adjustment: preserves layout structure but fixes spacing, angles, and snaps to grid
///
/// This function adjusts an existing layout without resetting positions. It:
/// - Snaps all positions to grid
/// - Fixes spacing between stations to maintain `STATION_SPACING`
/// - Preserves branch angles from user adjustments
/// - Snaps angles to 45-degree increments
///
/// # Panics
///
/// Panics if a positioned start node exists but has no position (which should be impossible).
pub fn adjust_layout(graph: &mut RailwayGraph) {
    use crate::models::Junctions;
    use std::collections::HashMap;

    // First pass: snap all existing positions to grid
    let positioned_nodes: Vec<_> = graph.graph.node_indices()
        .filter_map(|idx| {
            graph.get_station_position(idx).map(|pos| (idx, pos))
        })
        .collect();

    for (idx, pos) in positioned_nodes {
        let snapped = snap_to_grid(pos.0, pos.1);
        graph.set_station_position(idx, snapped);
    }

    // Infer branch angles from current layout for nodes that have positions
    let mut inferred_angles: HashMap<(usize, usize), f64> = HashMap::new();

    for idx in graph.graph.node_indices() {
        if let Some(parent_pos) = graph.get_station_position(idx) {
            // Get all neighbors
            let neighbors: Vec<_> = graph.graph.edges(idx)
                .map(|e| e.target())
                .chain(graph.graph.edges_directed(idx, Direction::Incoming).map(|e| e.source()))
                .collect();

            for neighbor in neighbors {
                // Skip if we already have a saved branch angle
                if graph.branch_angles.contains_key(&(idx.index(), neighbor.index())) {
                    continue;
                }

                if let Some(neighbor_pos) = graph.get_station_position(neighbor) {
                    let dx = neighbor_pos.0 - parent_pos.0;
                    let dy = neighbor_pos.1 - parent_pos.1;
                    let angle = dy.atan2(dx);

                    // Snap to nearest 45-degree angle
                    let angle_deg = angle.to_degrees();
                    let snapped_angle_deg = (angle_deg / 45.0).round() * 45.0;
                    let snapped_angle = snapped_angle_deg.to_radians();

                    inferred_angles.insert((idx.index(), neighbor.index()), snapped_angle);
                }
            }
        }
    }

    // Merge inferred angles with saved branch angles (saved angles take precedence)
    for (key, angle) in inferred_angles {
        graph.branch_angles.entry(key).or_insert(angle);
    }

    // Find starting node (prefer endpoints with positions)
    let start_node = graph.graph.node_indices()
        .find(|&idx| {
            let neighbor_count = graph.graph.edges(idx).count()
                + graph.graph.edges_directed(idx, Direction::Incoming).count();
            neighbor_count == 1 && graph.get_station_position(idx).is_some()
        })
        .or_else(|| {
            // Fall back to any positioned node
            graph.graph.node_indices()
                .find(|&idx| graph.get_station_position(idx).is_some())
        });

    let Some(start_node) = start_node else {
        return; // No positioned nodes, nothing to adjust
    };

    let start_pos = graph.get_station_position(start_node).expect("Start node should have position");

    // Recursively adjust positions starting from the start node
    let mut visited = HashSet::new();
    adjust_from_node(graph, start_node, start_pos, &mut visited);

    // Handle any disconnected positioned components
    let unvisited_positioned: Vec<_> = graph.graph.node_indices()
        .filter(|&idx| !visited.contains(&idx) && graph.get_station_position(idx).is_some())
        .collect();

    for idx in unvisited_positioned {
        let pos = graph.get_station_position(idx).expect("Node was filtered to have position");
        visited.clear(); // Reset visited for this component
        adjust_from_node(graph, idx, pos, &mut visited);
    }

    // Interpolate positions for junctions
    let junction_indices: Vec<_> = graph.graph.node_indices()
        .filter(|&idx| graph.is_junction(idx))
        .collect();

    for junction_idx in junction_indices {
        graph.interpolate_junction_position(junction_idx, false);
    }
}

/// Recursively adjust node positions from a starting node
fn adjust_from_node(
    graph: &mut RailwayGraph,
    current_node: NodeIndex,
    position: (f64, f64),
    visited: &mut HashSet<NodeIndex>,
) {
    if visited.contains(&current_node) {
        return;
    }

    let snapped_position = snap_to_grid(position.0, position.1);
    graph.set_station_position(current_node, snapped_position);
    visited.insert(current_node);

    // Get all unvisited neighbors with their track counts
    let mut neighbors_with_tracks: Vec<(NodeIndex, usize)> = Vec::new();

    for edge in graph.graph.edges(current_node) {
        let target = edge.target();
        if !visited.contains(&target) {
            let track_count = edge.weight().tracks.len();
            neighbors_with_tracks.push((target, track_count));
        }
    }

    for edge in graph.graph.edges_directed(current_node, Direction::Incoming) {
        let source = edge.source();
        if !visited.contains(&source) {
            let track_count = edge.weight().tracks.len();
            neighbors_with_tracks.push((source, track_count));
        }
    }

    if neighbors_with_tracks.is_empty() {
        return;
    }

    // Sort by track count (descending)
    neighbors_with_tracks.sort_by(|a, b| b.1.cmp(&a.1));

    // For each neighbor, determine its angle from saved branch_angles or current position
    for (neighbor, _) in neighbors_with_tracks {
        let angle = if let Some(&saved_angle) = graph.branch_angles.get(&(current_node.index(), neighbor.index())) {
            saved_angle
        } else if let Some(neighbor_pos) = graph.get_station_position(neighbor) {
            // Infer from current position and snap to 45 degrees
            let dx = neighbor_pos.0 - snapped_position.0;
            let dy = neighbor_pos.1 - snapped_position.1;
            let current_angle = dy.atan2(dx);
            let angle_deg = current_angle.to_degrees();
            let snapped_angle_deg = (angle_deg / 45.0).round() * 45.0;
            snapped_angle_deg.to_radians()
        } else {
            // No position, skip this neighbor (will be positioned by apply_layout if needed)
            continue;
        };

        // Calculate new position at proper distance and angle
        let new_pos = (
            snapped_position.0 + angle.cos() * STATION_SPACING,
            snapped_position.1 + angle.sin() * STATION_SPACING,
        );

        adjust_from_node(graph, neighbor, new_pos, visited);
    }
}

fn layout_line(
    graph: &mut RailwayGraph,
    current_node: NodeIndex,
    position: (f64, f64),
    direction: f64,
    spacing: f64,
    visited: &mut HashSet<NodeIndex>,
    available_directions: &mut Vec<f64>,
) {
    if visited.contains(&current_node) {
        return;
    }

    // Set position for current node (snapped to grid)
    let snapped_position = snap_to_grid(position.0, position.1);
    graph.set_station_position(current_node, snapped_position);
    visited.insert(current_node);

    // Get all unvisited neighbors with their track counts (both incoming and outgoing edges)
    let mut neighbors_with_tracks: Vec<(NodeIndex, usize)> = Vec::new();

    // Outgoing edges
    for edge in graph.graph.edges(current_node) {
        let target = edge.target();
        if !visited.contains(&target) {
            let track_count = edge.weight().tracks.len();
            neighbors_with_tracks.push((target, track_count));
        }
    }

    // Incoming edges (treat graph as undirected for layout purposes)
    for edge in graph.graph.edges_directed(current_node, Direction::Incoming) {
        let source = edge.source();
        if !visited.contains(&source) {
            let track_count = edge.weight().tracks.len();
            neighbors_with_tracks.push((source, track_count));
        }
    }

    if neighbors_with_tracks.is_empty() {
        return;
    }

    // Sort neighbors by track count (descending) - edges with more tracks should continue straight
    neighbors_with_tracks.sort_by(|a, b| b.1.cmp(&a.1));

    // Separate neighbors into those with saved angles and those without
    let mut neighbors_with_angles: Vec<(NodeIndex, f64)> = Vec::new();
    let mut neighbors_without_angles: Vec<(NodeIndex, usize)> = Vec::new();

    for &(neighbor, track_count) in &neighbors_with_tracks {
        if let Some(&saved_angle) = graph.branch_angles.get(&(current_node.index(), neighbor.index())) {
            neighbors_with_angles.push((neighbor, saved_angle));
        } else {
            neighbors_without_angles.push((neighbor, track_count));
        }
    }

    // Layout neighbors with saved angles first using their saved directions
    for (neighbor, saved_angle) in neighbors_with_angles {
        let neighbor_pos = (
            position.0 + saved_angle.cos() * spacing,
            position.1 + saved_angle.sin() * spacing,
        );
        layout_line(
            graph,
            neighbor,
            neighbor_pos,
            saved_angle,
            spacing,
            visited,
            available_directions,
        );
    }

    // For neighbors without saved angles, first (highest track count) continues in same direction (main line), rest are branches
    if !neighbors_without_angles.is_empty() {
        let (main_neighbor, _) = neighbors_without_angles[0];
        let next_pos = (
            position.0 + direction.cos() * spacing,
            position.1 + direction.sin() * spacing,
        );
        layout_line(
            graph,
            main_neighbor,
            next_pos,
            direction,
            spacing,
            visited,
            available_directions,
        );

        // Additional neighbors are branches - pick from available directions
        for &(branch_neighbor, _) in neighbors_without_angles.iter().skip(1) {
            if let Some(branch_dir) = available_directions.pop() {
                let branch_pos = (
                    position.0 + branch_dir.cos() * spacing,
                    position.1 + branch_dir.sin() * spacing,
                );

                layout_line(
                    graph,
                    branch_neighbor,
                    branch_pos,
                    branch_dir,
                    spacing,
                    visited,
                    available_directions,
                );
            }
        }
    }
}
