use crate::models::RailwayGraph;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::collections::HashSet;

pub fn apply_layout(graph: &mut RailwayGraph, height: f64) {
    let node_count = graph.graph.node_count();
    if node_count == 0 {
        return;
    }

    let station_spacing = 60.0;
    let start_x = 150.0;
    let start_y = height / 2.0;

    // Find a starting node (node with fewest connections)
    let start_node = graph
        .graph
        .node_indices()
        .min_by_key(|&idx| {
            let outgoing = graph.graph.edges(idx).count();
            let incoming = graph.graph.edges_directed(idx, Direction::Incoming).count();
            outgoing + incoming
        })
        .unwrap();

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
        station_spacing,
        &mut visited,
        &mut available_directions,
    );
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

    // First neighbor continues in the same direction (main line)
    let main_neighbor = neighbors[0];
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
    for &branch_neighbor in neighbors.iter().skip(1) {
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
