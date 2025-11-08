use crate::models::{RailwayGraph, Stations, ProjectSettings};
use crate::geometry::{angle_difference, line_segment_distance};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::HashSet;

const GRID_SIZE: f64 = 30.0;

// 8 compass directions (45° increments)
const DIRECTIONS: [f64; 8] = [
    0.0,                                    // E (0°)
    std::f64::consts::FRAC_PI_4,           // SE (45°)
    std::f64::consts::FRAC_PI_2,           // S (90°)
    3.0 * std::f64::consts::FRAC_PI_4,     // SW (135°)
    std::f64::consts::PI,                  // W (180°)
    -3.0 * std::f64::consts::FRAC_PI_4,    // NW (-135°)
    -std::f64::consts::FRAC_PI_2,          // N (-90°)
    -std::f64::consts::FRAC_PI_4,          // NE (-45°)
];

/// Snap coordinates to grid intersections
#[must_use]
pub fn snap_to_grid(x: f64, y: f64) -> (f64, f64) {
    let snapped_x = (x / GRID_SIZE).round() * GRID_SIZE;
    let snapped_y = (y / GRID_SIZE).round() * GRID_SIZE;
    (snapped_x, snapped_y)
}

/// Get all nodes reachable from `start_node`, excluding path back through `exclude_node`
fn get_reachable_nodes(
    graph: &RailwayGraph,
    start_node: NodeIndex,
    exclude_node: Option<NodeIndex>,
) -> HashSet<NodeIndex> {
    let mut reachable = HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    queue.push_back(start_node);
    reachable.insert(start_node);

    while let Some(current) = queue.pop_front() {
        // Get all neighbors (undirected)
        for neighbor in graph.graph.neighbors_undirected(current) {
            // Skip the excluded node
            if Some(neighbor) == exclude_node {
                continue;
            }

            // Skip already visited
            if reachable.contains(&neighbor) {
                continue;
            }

            reachable.insert(neighbor);
            queue.push_back(neighbor);
        }
    }

    reachable
}

/// Calculate how different two node sets are (0.0 = identical, 1.0 = completely different)
#[allow(clippy::cast_precision_loss)]
fn region_difference(set1: &HashSet<NodeIndex>, set2: &HashSet<NodeIndex>) -> f64 {
    if set1.is_empty() && set2.is_empty() {
        return 0.0;
    }

    let intersection_size = set1.intersection(set2).count();
    let union_size = set1.union(set2).count();

    if union_size == 0 {
        return 0.0;
    }

    // Jaccard distance
    1.0 - (intersection_size as f64 / union_size as f64)
}

/// Check if a position has node collision with existing nodes
fn has_node_collision_at(
    graph: &RailwayGraph,
    test_pos: (f64, f64),
    exclude_node: NodeIndex,
    base_station_spacing: f64,
) -> bool {
    for node_idx in graph.graph.node_indices() {
        if node_idx == exclude_node {
            continue;
        }
        if let Some(existing_pos) = graph.get_station_position(node_idx) {
            if existing_pos == (0.0, 0.0) {
                continue;
            }
            let dx = test_pos.0 - existing_pos.0;
            let dy = test_pos.1 - existing_pos.1;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < base_station_spacing * 0.9 {
                return true;
            }
        }
    }
    false
}

/// Check if a line segment would cross or come too close to any existing edges
#[allow(clippy::similar_names)]
fn would_overlap_existing_edges(
    graph: &RailwayGraph,
    pos1: (f64, f64),
    pos2: (f64, f64),
) -> bool {
    const MIN_DISTANCE: f64 = 50.0; // Minimum distance between parallel segments

    // Check all existing edges
    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();

        let Some(source_pos) = graph.get_station_position(source) else { continue };
        let Some(target_pos) = graph.get_station_position(target) else { continue };

        // Skip if either endpoint not placed yet
        if source_pos == (0.0, 0.0) || target_pos == (0.0, 0.0) {
            continue;
        }

        // Check if edges share an endpoint
        let shared_endpoint = if source_pos == pos1 || target_pos == pos1 {
            Some(pos1)
        } else if source_pos == pos2 || target_pos == pos2 {
            Some(pos2)
        } else {
            None
        };

        if let Some(shared_point) = shared_endpoint {
            // Edges share an endpoint - check angle between them
            let new_edge_other = if pos1 == shared_point { pos2 } else { pos1 };
            let existing_edge_other = if source_pos == shared_point { target_pos } else { source_pos };

            // Get direction vectors (normalized)
            let new_dx = new_edge_other.0 - shared_point.0;
            let new_dy = new_edge_other.1 - shared_point.1;
            let new_len = (new_dx * new_dx + new_dy * new_dy).sqrt();

            let exist_dx = existing_edge_other.0 - shared_point.0;
            let exist_dy = existing_edge_other.1 - shared_point.1;
            let exist_len = (exist_dx * exist_dx + exist_dy * exist_dy).sqrt();

            if new_len > 0.1 && exist_len > 0.1 {
                // Normalize direction vectors
                let new_dir_x = new_dx / new_len;
                let new_dir_y = new_dy / new_len;
                let exist_dir_x = exist_dx / exist_len;
                let exist_dir_y = exist_dy / exist_len;

                // Calculate angle between directions using atan2
                let new_angle = new_dir_y.atan2(new_dir_x);
                let exist_angle = exist_dir_y.atan2(exist_dir_x);

                let angle_diff = angle_difference(new_angle, exist_angle);

                // If angle difference is less than 45 degrees (π/4), edges are too close
                if angle_diff < std::f64::consts::FRAC_PI_4 {
                    return true;
                }
            }
        } else {
            // Edges don't share an endpoint - check normal segment distance
            let dist = line_segment_distance(pos1, pos2, source_pos, target_pos);
            if dist < MIN_DISTANCE {
                return true;
            }
        }
    }

    false
}

/// Find best direction and spacing for a branch node
fn find_best_direction_for_branch(
    graph: &RailwayGraph,
    current_pos: (f64, f64),
    neighbor: NodeIndex,
    target_pos: Option<(f64, f64)>,
    neighbor_reachable: &HashSet<NodeIndex>,
    already_used: &[(f64, HashSet<NodeIndex>)],
    incoming_direction: f64,
    base_station_spacing: f64,
) -> (f64, f64, i32) {
    let mut best_direction = DIRECTIONS[0];
    let mut best_score = i32::MIN;
    let mut best_spacing = 1.0;

    // Calculate direction to target if it exists
    let target_direction = target_pos.map(|target| {
        let dx = target.0 - current_pos.0;
        let dy = target.1 - current_pos.1;
        dy.atan2(dx)
    });

    // Try spacing multipliers from 1.0 up to 10.0
    for spacing_mult in [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 7.0, 10.0] {
        for &direction in &DIRECTIONS {
            let test_pos = snap_to_grid(
                current_pos.0 + direction.cos() * base_station_spacing * spacing_mult,
                current_pos.1 + direction.sin() * base_station_spacing * spacing_mult,
            );

            if has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
                continue;
            }

            // CRITICAL: If node has a target, reject directions that move away from it
            if let Some(target_dir) = target_direction {
                let angle_to_target = angle_difference(direction, target_dir);

                // If we're moving away from target (> 90°), reject this direction
                if angle_to_target > std::f64::consts::FRAC_PI_2 {
                    continue;
                }
            }

            let score = score_direction_for_branch(
                graph,
                current_pos,
                direction,
                spacing_mult,
                target_direction,
                neighbor_reachable,
                already_used,
                incoming_direction,
                base_station_spacing,
            );

            if score > best_score {
                best_score = score;
                best_direction = direction;
                best_spacing = spacing_mult;

                // If we found a valid direction, use it
                if score > i32::MIN {
                    return (best_direction, best_spacing, best_score);
                }
            }
        }
    }

    (best_direction, best_spacing, best_score)
}

/// Score a direction for placing a branch node
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::too_many_arguments)]
fn score_direction_for_branch(
    graph: &RailwayGraph,
    current_pos: (f64, f64),
    direction: f64,
    spacing_multiplier: f64,
    target_direction: Option<f64>,
    neighbor_reachable: &HashSet<NodeIndex>,
    already_used: &[(f64, HashSet<NodeIndex>)],
    incoming_direction: f64,
    base_station_spacing: f64,
) -> i32 {
    let mut score = 0;

    // Calculate proposed position
    let neighbor_pos = snap_to_grid(
        current_pos.0 + direction.cos() * base_station_spacing * spacing_multiplier,
        current_pos.1 + direction.sin() * base_station_spacing * spacing_multiplier,
    );

    // CRITICAL: Check for geometric overlap with existing edges
    if would_overlap_existing_edges(graph, current_pos, neighbor_pos) {
        return i32::MIN;
    }

    // Check if this direction goes back where we came from (opposite of incoming)
    let reverse_direction = incoming_direction + std::f64::consts::PI;
    let reverse_angle_diff = angle_difference(direction, reverse_direction);

    // CRITICAL: Never go back in the direction we came from (causes overlap)
    if reverse_angle_diff < std::f64::consts::FRAC_PI_4 {
        return i32::MIN;
    }

    // If we have a target, strongly prefer moving towards it
    if let Some(target_dir) = target_direction {
        let angle_to_target = angle_difference(direction, target_dir);

        // Strong bonus for moving towards target
        score += ((std::f64::consts::PI - angle_to_target) * 2000.0) as i32;
    }

    // For each already-used direction
    for (used_dir, used_reachable) in already_used {
        let angle_diff = angle_difference(direction, *used_dir);
        let region_diff = region_difference(neighbor_reachable, used_reachable);

        // If regions are DIFFERENT but directions are SIMILAR = bad
        if region_diff > 0.5 && angle_diff < std::f64::consts::FRAC_PI_2 {
            score -= ((1.0 - region_diff) * 5000.0) as i32;
        }

        // If regions are SIMILAR and directions are SIMILAR = ok
        if region_diff < 0.3 && angle_diff < std::f64::consts::FRAC_PI_4 {
            score += 200;
        }

        // Prefer larger angular separation
        score += (angle_diff * 500.0) as i32;
    }

    score
}

#[allow(clippy::too_many_lines, clippy::missing_panics_doc, clippy::cast_precision_loss)]
pub fn apply_layout(graph: &mut RailwayGraph, height: f64, settings: &ProjectSettings) {
    let base_station_spacing = settings.default_node_distance_grid_squares * GRID_SIZE;
    let start_x = 150.0;
    let start_y = height / 2.0;

    if graph.graph.node_count() == 0 {
        return; // Empty graph
    }

    // Clear all positions
    let all_nodes: Vec<_> = graph.graph.node_indices().collect();
    for node_idx in all_nodes {
        graph.set_station_position(node_idx, (0.0, 0.0));
    }

    // Phase 1: Find longest path (the main spine)
    let spine = graph.find_longest_path();

    if spine.is_empty() {
        return;
    }

    // Phase 2: Place spine vertically (North-South)
    let mut visited = HashSet::new();
    let spine_direction = -std::f64::consts::FRAC_PI_2; // North (-90°)

    for (i, &node) in spine.iter().enumerate() {
        let offset = i as f64 * base_station_spacing;
        let pos = snap_to_grid(
            start_x + spine_direction.cos() * offset,
            start_y + spine_direction.sin() * offset,
        );
        graph.set_station_position(node, pos);
        visited.insert(node);
    }

    // Phase 3: Place branches from spine nodes
    let mut queue = std::collections::VecDeque::new();

    // Add all spine nodes to queue with their positions and incoming direction
    for &node in &spine {
        if let Some(pos) = graph.get_station_position(node) {
            queue.push_back((node, pos, spine_direction));
        }
    }

    while let Some((current_node, current_pos, incoming_direction)) = queue.pop_front() {
        // Get all unvisited neighbors
        let neighbors: Vec<_> = graph
            .graph
            .neighbors_undirected(current_node)
            .filter(|n| !visited.contains(n))
            .collect();

        if neighbors.is_empty() {
            continue;
        }

        // Track which directions we've assigned from this node
        let mut already_used: Vec<(f64, HashSet<NodeIndex>)> = Vec::new();

        for &neighbor in &neighbors {
            // Check if neighbor has any edges to already-placed nodes (besides current)
            let target_pos = find_placed_target(graph, neighbor, current_node, &visited);

            let reachable = get_reachable_nodes(graph, neighbor, Some(current_node));

            let (best_direction, best_spacing, _best_score) = find_best_direction_for_branch(
                graph,
                current_pos,
                neighbor,
                target_pos,
                &reachable,
                &already_used,
                incoming_direction,
                base_station_spacing,
            );

            let neighbor_pos = snap_to_grid(
                current_pos.0 + best_direction.cos() * base_station_spacing * best_spacing,
                current_pos.1 + best_direction.sin() * base_station_spacing * best_spacing,
            );

            graph.set_station_position(neighbor, neighbor_pos);
            visited.insert(neighbor);

            already_used.push((best_direction, reachable.clone()));

            queue.push_back((neighbor, neighbor_pos, best_direction));
        }
    }

    // Phase 4: Handle disconnected components
    let disconnected: Vec<_> = graph
        .graph
        .node_indices()
        .filter(|idx| !visited.contains(idx))
        .collect();

    if !disconnected.is_empty() {
        let mut offset_x = start_x + 400.0;

        for &node in &disconnected {
            if visited.contains(&node) {
                continue;
            }

            // Find longest path in this disconnected component
            let component_spine = graph.find_longest_path_from(node, &visited);

            for (i, &comp_node) in component_spine.iter().enumerate() {
                let offset = i as f64 * base_station_spacing;
                let pos = snap_to_grid(
                    offset_x,
                    start_y + spine_direction.sin() * offset,
                );
                graph.set_station_position(comp_node, pos);
                visited.insert(comp_node);
            }

            offset_x += 400.0;
        }
    }
}

/// Find if a node has any connections to already-placed nodes (excluding current)
fn find_placed_target(
    graph: &RailwayGraph,
    node: NodeIndex,
    exclude: NodeIndex,
    visited: &HashSet<NodeIndex>,
) -> Option<(f64, f64)> {
    for neighbor in graph.graph.neighbors_undirected(node) {
        if neighbor != exclude && visited.contains(&neighbor) {
            if let Some(pos) = graph.get_station_position(neighbor) {
                if pos != (0.0, 0.0) {
                    return Some(pos);
                }
            }
        }
    }
    None
}

pub fn adjust_layout(_graph: &mut RailwayGraph) {
    // TODO: Implement smart adjustment
}

/// Snap station to grid when manually dragging (with branch reorientation)
pub fn snap_to_angle(graph: &mut RailwayGraph, station_idx: NodeIndex, x: f64, y: f64) {
    let snapped = snap_to_grid(x, y);
    graph.set_station_position(station_idx, snapped);
}

/// Snap station to grid when manually dragging (along branch)
pub fn snap_station_along_branch(graph: &mut RailwayGraph, station_idx: NodeIndex, x: f64, y: f64) {
    let snapped = snap_to_grid(x, y);
    graph.set_station_position(station_idx, snapped);
}
