use crate::models::RailwayGraph;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::HashSet;

const STATION_SPACING: f64 = 60.0;

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

    graph.set_station_position(current_node, position);
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

    // Handle any disconnected nodes
    let mut offset_y = start_y + 100.0;
    for idx in graph.graph.node_indices() {
        if !visited.contains(&idx) {
            graph.set_station_position(idx, (start_x + 200.0, offset_y));
            offset_y += STATION_SPACING;
        }
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

    // Set position for current node
    graph.set_station_position(current_node, position);
    visited.insert(current_node);

    // Get all unvisited neighbors (both incoming and outgoing edges)
    let mut neighbors = Vec::new();

    // Outgoing edges
    for edge in graph.graph.edges(current_node) {
        let target = edge.target();
        if !visited.contains(&target) {
            neighbors.push(target);
        }
    }

    // Incoming edges (treat graph as undirected for layout purposes)
    for edge in graph.graph.edges_directed(current_node, Direction::Incoming) {
        let source = edge.source();
        if !visited.contains(&source) {
            neighbors.push(source);
        }
    }

    if neighbors.is_empty() {
        return;
    }

    // Separate neighbors into those with saved angles and those without
    let mut neighbors_with_angles: Vec<(NodeIndex, f64)> = Vec::new();
    let mut neighbors_without_angles: Vec<NodeIndex> = Vec::new();

    for &neighbor in &neighbors {
        if let Some(&saved_angle) = graph.branch_angles.get(&(current_node.index(), neighbor.index())) {
            neighbors_with_angles.push((neighbor, saved_angle));
        } else {
            neighbors_without_angles.push(neighbor);
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

    // For neighbors without saved angles, first continues in same direction (main line), rest are branches
    if !neighbors_without_angles.is_empty() {
        let main_neighbor = neighbors_without_angles[0];
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
        for &branch_neighbor in neighbors_without_angles.iter().skip(1) {
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
