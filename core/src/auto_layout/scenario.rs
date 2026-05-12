use crate::models::RailwayGraph;
use petgraph::stable_graph::NodeIndex;
use std::collections::HashSet;

use super::geographic_hints::GeographicHints;
use super::types::LayoutScenario;

/// Detect which layout scenario we're in based on available data
#[allow(clippy::cast_precision_loss)]
pub fn detect_scenario(
    graph: &RailwayGraph,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &HashSet<NodeIndex>,
) -> LayoutScenario {
    let nodes: Vec<NodeIndex> = graph.graph.node_indices().collect();
    let total = nodes.len();

    if total == 0 {
        return LayoutScenario::ManualNetwork;
    }

    // Count nodes with existing positions (non-zero, non-pinned)
    let positioned_count = nodes
        .iter()
        .filter(|&&node| {
            if pinned_nodes.contains(&node) {
                return true; // Pinned nodes count as positioned
            }
            has_existing_position(graph, node)
        })
        .count();

    // Count nodes with geographic coordinates
    let geo_count = geo_hints.map_or(0, |hints| {
        nodes.iter().filter(|&&node| hints.has_coords(node)).count()
    });

    let pos_ratio = positioned_count as f64 / total as f64;
    let geo_ratio = geo_count as f64 / total as f64;

    // Classification logic
    match (pos_ratio > 0.5, geo_ratio > 0.5) {
        // Most nodes have geo, few have positions -> fresh import
        (false, true) => LayoutScenario::FreshImport,

        // Most nodes have positions, few have geo -> manual network
        (true, false) => LayoutScenario::ManualNetwork,

        // Both high -> either re-layout or matched import
        (true, true) => {
            // If nearly all positioned nodes also have geo, it's mixed/matched
            // (existing stations were matched with import data)
            LayoutScenario::MixedMatched
        }

        // Neither high -> incremental (some positioned, some new with geo)
        (false, false) => {
            if positioned_count > 0 && geo_count > 0 {
                LayoutScenario::Incremental
            } else if positioned_count > 0 {
                LayoutScenario::ManualNetwork
            } else {
                LayoutScenario::FreshImport
            }
        }
    }
}

/// Check if a node has an existing non-zero position
fn has_existing_position(graph: &RailwayGraph, node: NodeIndex) -> bool {
    graph
        .graph
        .node_weight(node)
        .and_then(|n| n.as_station())
        .and_then(|s| s.position)
        .is_some_and(|(x, y)| x != 0.0 || y != 0.0)
}

/// Get existing position for a node if it has one
pub fn get_existing_position(graph: &RailwayGraph, node: NodeIndex) -> Option<(f64, f64)> {
    graph
        .graph
        .node_weight(node)
        .and_then(|n| n.as_station())
        .and_then(|s| s.position)
        .filter(|&(x, y)| x != 0.0 || y != 0.0)
}

