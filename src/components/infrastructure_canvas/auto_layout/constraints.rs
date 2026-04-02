use crate::models::{RailwayGraph, Stations};

use super::types::LayoutState;

/// Apply final positions to the graph
pub fn apply_positions_to_graph(graph: &mut RailwayGraph, state: &LayoutState) {
    for (&node, &pos) in &state.positions {
        graph.set_station_position(node, pos.to_tuple());
    }
}
