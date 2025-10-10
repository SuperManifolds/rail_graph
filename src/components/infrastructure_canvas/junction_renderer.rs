use crate::models::{RailwayGraph, Junctions, Stations};
use web_sys::CanvasRenderingContext2d;
use petgraph::graph::{NodeIndex, EdgeIndex};
use petgraph::Direction;
use petgraph::visit::EdgeRef;
use std::collections::HashSet;

const JUNCTION_TRACK_DISTANCE: f64 = 14.0; // Match JUNCTION_STOP_DISTANCE from track_renderer
const TRACK_SPACING: f64 = 3.0; // Match track_renderer
const TRACK_COLOR: &str = "#444";
const TRACK_LINE_WIDTH: f64 = 2.0;

fn draw_junction_track_connections(
    ctx: &CanvasRenderingContext2d,
    from_track_count: usize,
    to_track_count: usize,
    entry_base: (f64, f64),
    exit_base: (f64, f64),
    from_perp: (f64, f64),
    to_perp: (f64, f64),
    zoom: f64,
) {
    // Calculate all entry track positions
    #[allow(clippy::cast_precision_loss)]
    let from_total_width = (from_track_count - 1) as f64 * TRACK_SPACING;
    let from_start_offset = -from_total_width / 2.0;

    let mut entry_points = Vec::new();
    for track_idx in 0..from_track_count {
        #[allow(clippy::cast_precision_loss)]
        let offset = from_start_offset + (track_idx as f64 * TRACK_SPACING);
        entry_points.push((
            entry_base.0 + from_perp.0 * offset,
            entry_base.1 + from_perp.1 * offset
        ));
    }

    // Calculate all exit track positions
    #[allow(clippy::cast_precision_loss)]
    let to_total_width = (to_track_count - 1) as f64 * TRACK_SPACING;
    let to_start_offset = -to_total_width / 2.0;

    let mut exit_points = Vec::new();
    for track_idx in 0..to_track_count {
        #[allow(clippy::cast_precision_loss)]
        let offset = to_start_offset + (track_idx as f64 * TRACK_SPACING);
        exit_points.push((
            exit_base.0 + to_perp.0 * offset,
            exit_base.1 + to_perp.1 * offset
        ));
    }

    // Draw connections from every entry track to every exit track
    ctx.set_stroke_style_str(TRACK_COLOR);
    ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

    for entry_point in &entry_points {
        for exit_point in &exit_points {
            ctx.begin_path();
            ctx.move_to(entry_point.0, entry_point.1);
            ctx.line_to(exit_point.0, exit_point.1);
            ctx.stroke();
        }
    }
}

pub fn draw_junction(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    idx: NodeIndex,
    pos: (f64, f64),
    zoom: f64,
) {
    // Collect all connected edges with their angles, positions, and edge IDs
    let mut connections: Vec<(f64, EdgeIndex, (f64, f64))> = Vec::new();

    // Incoming edges
    for edge in graph.graph.edges_directed(idx, Direction::Incoming) {
        if let Some(source_pos) = graph.get_station_position(edge.source()) {
            let angle = (source_pos.1 - pos.1).atan2(source_pos.0 - pos.0);
            connections.push((angle, edge.id(), source_pos));
        }
    }

    // Outgoing edges
    for edge in graph.graph.edges(idx) {
        if let Some(target_pos) = graph.get_station_position(edge.target()) {
            let angle = (target_pos.1 - pos.1).atan2(target_pos.0 - pos.0);
            connections.push((angle, edge.id(), target_pos));
        }
    }

    if connections.is_empty() {
        return;
    }

    let junction = graph.get_junction(idx);
    let Some(j) = junction else { return };

    ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

    // Track which edge pairs we've already drawn to avoid duplicates
    let mut drawn_pairs: HashSet<(EdgeIndex, EdgeIndex)> = HashSet::new();

    // Draw connections between allowed routing pairs
    for (i, (_from_angle, from_edge, from_node_pos)) in connections.iter().enumerate() {
        // Get track count for the incoming edge
        let from_track_count = graph.graph.edge_weight(*from_edge)
            .map_or(0, |edge| edge.tracks.len());

        if from_track_count == 0 {
            continue;
        }

        for (j_idx, (_to_angle, to_edge, to_node_pos)) in connections.iter().enumerate() {
            if i == j_idx {
                continue; // Skip same edge
            }

            // Create a canonical pair (smaller index first) to avoid drawing duplicates
            let pair = if from_edge.index() < to_edge.index() {
                (*from_edge, *to_edge)
            } else {
                (*to_edge, *from_edge)
            };

            if drawn_pairs.contains(&pair) {
                continue; // Already drawn this pair
            }

            let is_allowed = j.is_routing_allowed(*from_edge, *to_edge);

            if !is_allowed {
                continue; // Don't draw blocked routes
            }

            drawn_pairs.insert(pair);

            // Get track count for the outgoing edge
            let to_track_count = graph.graph.edge_weight(*to_edge)
                .map_or(0, |edge| edge.tracks.len());

            if to_track_count == 0 {
                continue;
            }

            // Get the from edge details to calculate proper perpendicular
            let from_edge_ref = graph.graph.edge_references().find(|e| e.id() == *from_edge);
            let (from_source, from_target) = if let Some(e) = from_edge_ref {
                (e.source(), e.target())
            } else {
                continue;
            };

            let from_source_pos = graph.get_station_position(from_source).unwrap_or(pos);
            let from_target_pos = graph.get_station_position(from_target).unwrap_or(pos);

            // Edge direction vector (source -> target, matching track_renderer)
            let from_edge_vec = (from_target_pos.0 - from_source_pos.0, from_target_pos.1 - from_source_pos.1);
            let from_edge_len = (from_edge_vec.0 * from_edge_vec.0 + from_edge_vec.1 * from_edge_vec.1).sqrt();

            // Perpendicular to from edge (same calculation as track_renderer)
            let from_perp = (-from_edge_vec.1 / from_edge_len, from_edge_vec.0 / from_edge_len);

            // Calculate entry base point
            let entry_delta = (from_node_pos.0 - pos.0, from_node_pos.1 - pos.1);
            let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();
            let entry_base = (
                pos.0 + (entry_delta.0 / entry_distance) * JUNCTION_TRACK_DISTANCE,
                pos.1 + (entry_delta.1 / entry_distance) * JUNCTION_TRACK_DISTANCE,
            );

            // Get the to edge details to calculate proper perpendicular
            let to_edge_ref = graph.graph.edge_references().find(|e| e.id() == *to_edge);
            let (to_source, to_target) = if let Some(e) = to_edge_ref {
                (e.source(), e.target())
            } else {
                continue;
            };

            let to_source_pos = graph.get_station_position(to_source).unwrap_or(pos);
            let to_target_pos = graph.get_station_position(to_target).unwrap_or(pos);

            // Edge direction vector (source -> target, matching track_renderer)
            let to_edge_vec = (to_target_pos.0 - to_source_pos.0, to_target_pos.1 - to_source_pos.1);
            let to_edge_len = (to_edge_vec.0 * to_edge_vec.0 + to_edge_vec.1 * to_edge_vec.1).sqrt();

            // Perpendicular to to edge (same calculation as track_renderer)
            let to_perp = (-to_edge_vec.1 / to_edge_len, to_edge_vec.0 / to_edge_len);

            // Calculate exit base point
            let exit_delta = (to_node_pos.0 - pos.0, to_node_pos.1 - pos.1);
            let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();
            let exit_base = (
                pos.0 + (exit_delta.0 / exit_distance) * JUNCTION_TRACK_DISTANCE,
                pos.1 + (exit_delta.1 / exit_distance) * JUNCTION_TRACK_DISTANCE,
            );

            if from_track_count == 1 && to_track_count == 1 {
                // Single track to single track - draw simple connection
                ctx.set_stroke_style_str(TRACK_COLOR);
                ctx.begin_path();
                ctx.move_to(entry_base.0, entry_base.1);
                ctx.line_to(exit_base.0, exit_base.1);
                ctx.stroke();
            } else {
                // Multiple tracks - connect every track to every other track
                draw_junction_track_connections(
                    ctx,
                    from_track_count,
                    to_track_count,
                    entry_base,
                    exit_base,
                    from_perp,
                    to_perp,
                    zoom,
                );
            }
        }
    }
}
