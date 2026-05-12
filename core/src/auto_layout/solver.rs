use crate::models::{ProjectSettings, RailwayGraph};
#[cfg(feature = "solver")]
use crate::models::Stations;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::{HashMap, HashSet};

use super::constants::GRID_SIZE;
use super::constraints::apply_positions_to_graph;
use super::geographic_hints::GeographicHints;
use super::init::initialize_positions;
#[cfg(feature = "solver")]
use super::mip;
use super::scenario::detect_scenario;
use super::types::{LayoutConfig, LayoutState};

/// Minimum edge length in MIP grid units (each unit = `GRID_SIZE` pixels)
#[cfg(feature = "solver")]
const MIP_MIN_EDGE_LENGTH: f64 = 4.0;

/// Main entry point: compute layout and apply to graph
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

    // For fresh imports with geographic data, use MIP solver
    #[cfg(feature = "solver")]
    if scenario == super::types::LayoutScenario::FreshImport {
        if let Some(hints) = geo_hints.filter(|h| !h.is_empty()) {
            if let Some(positions) = mip::run_mip_layout(graph, hints, MIP_MIN_EDGE_LENGTH) {
                // Apply MIP positions and snap to grid
                for (node, (x, y)) in &positions {
                    let snapped = super::snap_to_grid(*x, *y);
                    graph.set_station_position(*node, snapped);
                }
                return;
            }
            log::info!("MIP solver failed, falling back to BFS layout");
        }
    }

    // Fallback: BFS-based layout
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
