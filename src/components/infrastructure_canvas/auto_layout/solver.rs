use crate::models::{ProjectSettings, RailwayGraph};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::{HashMap, HashSet};

use super::constants::GRID_SIZE;
use super::constraints::apply_positions_to_graph;
use super::geographic_hints::GeographicHints;
use super::init::initialize_positions;
use super::scenario::detect_scenario;
use super::types::{LayoutConfig, LayoutState};

/// Main entry point: compute layout and apply to graph.
/// MIP solver runs on the Tauri backend; this only handles the BFS fallback.
pub fn compute_and_apply_layout(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &HashSet<NodeIndex>,
    _edge_weights: Option<&HashMap<EdgeIndex, usize>>,
) {
    if graph.graph.node_count() <= 1 {
        return;
    }

    let scenario = detect_scenario(graph, geo_hints, pinned_nodes);

    // BFS-based layout
    #[allow(clippy::cast_precision_loss)]
    let node_count_f = graph.graph.node_count() as f64;
    let spacing = (settings.default_node_distance_grid_squares * GRID_SIZE)
        .max(GRID_SIZE * 3.0)
        .max(node_count_f.sqrt() * GRID_SIZE * 0.6);
    let spacing = (spacing / GRID_SIZE).round() * GRID_SIZE;
    let canvas_size = node_count_f.sqrt() * spacing * 2.5;

    let config = LayoutConfig {
        base_spacing: spacing,
        canvas_width: canvas_size.max(2000.0),
        canvas_height: canvas_size.max(height),
    };

    let state = LayoutState {
        positions: initialize_positions(graph, geo_hints, scenario, &config),
    };

    apply_positions_to_graph(graph, &state);
}
