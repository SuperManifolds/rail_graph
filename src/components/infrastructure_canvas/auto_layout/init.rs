use crate::geometry::angle_difference;
use crate::models::RailwayGraph;
use petgraph::stable_graph::NodeIndex;
use std::collections::{HashMap, HashSet, VecDeque};

use super::constants::COMPASS_DIRECTIONS;

use super::geographic_hints::GeographicHints;
use super::scenario::get_existing_position;
use super::types::{LayoutConfig, LayoutScenario, Vec2};

/// Initialize positions based on the detected scenario
pub fn initialize_positions(
    graph: &RailwayGraph,
    geo_hints: Option<&GeographicHints>,
    scenario: LayoutScenario,
    config: &LayoutConfig,
) -> HashMap<NodeIndex, Vec2> {
    match scenario {
        LayoutScenario::FreshImport => initialize_from_geography(graph, geo_hints, config),
        LayoutScenario::Incremental => initialize_incremental(graph, geo_hints, config),
        LayoutScenario::ManualNetwork | LayoutScenario::MixedMatched => {
            collect_existing_positions(graph)
        }
    }
}

/// Initialize positions using BFS traversal with geographic direction hints.
/// Each node is placed at `base_spacing` from its parent, in the direction
/// suggested by geography. This ensures even spacing regardless of geographic density.
fn initialize_from_geography(
    graph: &RailwayGraph,
    geo_hints: Option<&GeographicHints>,
    config: &LayoutConfig,
) -> HashMap<NodeIndex, Vec2> {
    let mut positions = HashMap::new();

    if graph.graph.node_count() == 0 {
        return positions;
    }

    let Some(hints) = geo_hints.filter(|h| !h.is_empty()) else {
        // No geography - place in a line
        let mut x = 150.0;
        for node in graph.graph.node_indices() {
            positions.insert(node, Vec2::new(x, config.canvas_height / 2.0));
            x += config.base_spacing;
        }
        return positions;
    };

    // Find the highest-degree node as the starting point
    let start = graph
        .graph
        .node_indices()
        .max_by_key(|&n| graph.graph.neighbors_undirected(n).count())
        .expect("graph is not empty");

    // BFS placement: each node placed at base_spacing from its parent
    let center = Vec2::new(config.canvas_width / 2.0, config.canvas_height / 2.0);
    positions.insert(start, center);

    let mut visited = HashSet::new();
    visited.insert(start);

    let mut queue = VecDeque::new();
    queue.push_back(start);

    while let Some(current) = queue.pop_front() {
        let current_pos = positions[&current];

        // Collect unvisited neighbors
        let neighbors: Vec<NodeIndex> = graph
            .graph
            .neighbors_undirected(current)
            .filter(|n| !visited.contains(n))
            .collect();

        for neighbor in neighbors {
            visited.insert(neighbor);

            // Determine placement direction from geography
            let direction = choose_placement_direction(
                hints, current, neighbor, &current_pos, &positions, config,
            );

            let pos = Vec2::new(
                current_pos.x + direction.cos() * config.base_spacing,
                current_pos.y + direction.sin() * config.base_spacing,
            );

            positions.insert(neighbor, pos);
            queue.push_back(neighbor);
        }
    }

    // Handle disconnected components
    for node in graph.graph.node_indices() {
        positions.entry(node).or_insert(center);
    }

    positions
}

/// Choose the best placement direction for a node using geographic hints
/// and collision avoidance with already-placed nodes
fn choose_placement_direction(
    hints: &GeographicHints,
    from: NodeIndex,
    to: NodeIndex,
    from_pos: &Vec2,
    positions: &HashMap<NodeIndex, Vec2>,
    config: &LayoutConfig,
) -> f64 {
    // Get geographic preferred direction
    let geo_dir = hints.preferred_direction(from, to);

    // Snap geographic direction to nearest compass direction
    let preferred = geo_dir.map_or(0.0, snap_to_compass);

    // Sort compass directions by closeness to preferred
    let mut candidates: Vec<f64> = COMPASS_DIRECTIONS.to_vec();
    candidates.sort_by(|&a, &b| {
        angle_difference(a, preferred)
            .partial_cmp(&angle_difference(b, preferred))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Pick the first direction that doesn't collide
    for &dir in &candidates {
        let test_pos = Vec2::new(
            from_pos.x + dir.cos() * config.base_spacing,
            from_pos.y + dir.sin() * config.base_spacing,
        );

        let has_collision = positions.values().any(|&p| test_pos.dist(p) < config.base_spacing * 0.7);
        if !has_collision {
            return dir;
        }
    }

    // All directions blocked - use preferred with increased spacing (stress will fix it)
    preferred
}

/// Snap angle to nearest 45° compass direction
fn snap_to_compass(angle: f64) -> f64 {
    COMPASS_DIRECTIONS
        .iter()
        .min_by(|&&a, &&b| {
            angle_difference(angle, a)
                .partial_cmp(&angle_difference(angle, b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .unwrap_or(0.0)
}

/// Collect existing positions from graph
fn collect_existing_positions(graph: &RailwayGraph) -> HashMap<NodeIndex, Vec2> {
    let mut positions = HashMap::new();

    for node in graph.graph.node_indices() {
        if let Some((x, y)) = get_existing_position(graph, node) {
            positions.insert(node, Vec2::new(x, y));
        } else {
            // No position - place at origin, will be fixed by stress forces
            positions.insert(node, Vec2::zero());
        }
    }

    positions
}

/// Initialize for incremental layout: keep existing, place new nodes near neighbors
fn initialize_incremental(
    graph: &RailwayGraph,
    geo_hints: Option<&GeographicHints>,
    config: &LayoutConfig,
) -> HashMap<NodeIndex, Vec2> {
    let mut positions = HashMap::new();

    // First pass: collect all existing positions
    for node in graph.graph.node_indices() {
        if let Some((x, y)) = get_existing_position(graph, node) {
            positions.insert(node, Vec2::new(x, y));
        }
    }

    // Second pass: place new nodes using geography and neighbors
    for node in graph.graph.node_indices() {
        if positions.contains_key(&node) {
            continue;
        }

        // Try to place using geographic direction from a positioned neighbor
        if let Some(pos) = place_from_neighbor_with_geo(graph, node, &positions, geo_hints, config)
        {
            positions.insert(node, pos);
            continue;
        }

        // Fallback: interpolate from any positioned neighbors
        if let Some(pos) = interpolate_from_neighbors(graph, node, &positions) {
            positions.insert(node, pos);
            continue;
        }

        // Last resort: canvas center
        positions.insert(
            node,
            Vec2::new(config.canvas_width / 2.0, config.canvas_height / 2.0),
        );
    }

    positions
}

/// Place a node using geographic direction from a positioned neighbor
fn place_from_neighbor_with_geo(
    graph: &RailwayGraph,
    node: NodeIndex,
    positions: &HashMap<NodeIndex, Vec2>,
    geo_hints: Option<&GeographicHints>,
    config: &LayoutConfig,
) -> Option<Vec2> {
    let hints = geo_hints?;
    let node_coords = hints.get_coords(node)?;

    // Find a positioned neighbor that also has geo coords
    for neighbor in graph.graph.neighbors_undirected(node) {
        let Some(neighbor_pos) = positions.get(&neighbor) else {
            continue;
        };
        let Some(neighbor_coords) = hints.get_coords(neighbor) else {
            continue;
        };

        // Compute geographic direction from neighbor to this node
        let dx = node_coords.0 - neighbor_coords.0;
        let dy = neighbor_coords.1 - node_coords.1; // Flip for screen coords
        let angle = dy.atan2(dx);

        return Some(Vec2::new(
            neighbor_pos.x + angle.cos() * config.base_spacing,
            neighbor_pos.y + angle.sin() * config.base_spacing,
        ));
    }

    None
}

/// Interpolate position from positioned neighbors (centroid)
fn interpolate_from_neighbors(
    graph: &RailwayGraph,
    node: NodeIndex,
    positions: &HashMap<NodeIndex, Vec2>,
) -> Option<Vec2> {
    let neighbors: Vec<Vec2> = graph
        .graph
        .neighbors_undirected(node)
        .filter_map(|n| positions.get(&n).copied())
        .filter(|p| p.x != 0.0 || p.y != 0.0)
        .collect();

    if neighbors.is_empty() {
        return None;
    }

    #[allow(clippy::cast_precision_loss)]
    let centroid = Vec2::new(
        neighbors.iter().map(|p| p.x).sum::<f64>() / neighbors.len() as f64,
        neighbors.iter().map(|p| p.y).sum::<f64>() / neighbors.len() as f64,
    );

    Some(centroid)
}

