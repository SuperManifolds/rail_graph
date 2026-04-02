//! Auto-layout module for railway graph visualization.
//!
//! This module provides automatic layout algorithms for positioning stations
//! and junctions in the infrastructure view. The main algorithm uses stress
//! majorization with geographic and octilinear constraints.

mod constants;
mod constraints;
mod geographic_hints;
mod init;
mod mip;
mod scenario;
mod solver;
mod types;

// Re-export public API
pub use constants::GRID_SIZE;
pub use geographic_hints::GeographicHints;

use crate::models::{ProjectSettings, RailwayGraph, Stations};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::{HashMap, HashSet};

/// Snap coordinates to grid intersections
#[must_use]
pub fn snap_to_grid(x: f64, y: f64) -> (f64, f64) {
    let snapped_x = (x / GRID_SIZE).round() * GRID_SIZE;
    let snapped_y = (y / GRID_SIZE).round() * GRID_SIZE;
    (snapped_x, snapped_y)
}

/// Apply automatic layout to the graph
pub fn apply_layout(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
) {
    solver::compute_and_apply_layout(graph, height, settings, geo_hints, &HashSet::new(), None);
}

/// Apply automatic layout, preserving positions of pinned nodes
pub fn apply_layout_with_pinned(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &HashSet<NodeIndex>,
) {
    solver::compute_and_apply_layout(graph, height, settings, geo_hints, pinned_nodes, None);
}

/// Apply automatic layout with edge weights for importance-based spacing
///
/// Edge weights (typically from line usage counts) make high-traffic corridors
/// have stronger attraction forces, keeping those stations closer together.
pub fn apply_layout_with_edge_weights(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &HashSet<NodeIndex>,
    edge_weights: &HashMap<EdgeIndex, usize>,
) {
    solver::compute_and_apply_layout(
        graph,
        height,
        settings,
        geo_hints,
        pinned_nodes,
        Some(edge_weights),
    );
}

/// Placeholder for smart layout adjustment (not yet implemented)
pub fn adjust_layout(_graph: &mut RailwayGraph) {
    // TODO: Implement smart adjustment
}

/// Snap station to grid when manually dragging
pub fn snap_to_angle(graph: &mut RailwayGraph, station_idx: NodeIndex, x: f64, y: f64) {
    let snapped = snap_to_grid(x, y);
    graph.set_station_position(station_idx, snapped);
}

/// Snap station to grid when manually dragging along branch
pub fn snap_station_along_branch(graph: &mut RailwayGraph, station_idx: NodeIndex, x: f64, y: f64) {
    let snapped = snap_to_grid(x, y);
    graph.set_station_position(station_idx, snapped);
}
