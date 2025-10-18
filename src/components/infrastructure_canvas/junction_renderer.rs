use crate::models::{RailwayGraph, Junctions, Stations, TrackDirection};
use web_sys::CanvasRenderingContext2d;
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::Direction;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use super::track_renderer;

const JUNCTION_TRACK_DISTANCE: f64 = 14.0; // Match JUNCTION_STOP_DISTANCE from track_renderer
const TRACK_SPACING: f64 = 3.0; // Match track_renderer
const TRACK_COLOR: &str = "#444";
const TRACK_LINE_WIDTH: f64 = 2.0;

/// Check if a specific track allows arrival at the junction
fn track_allows_arrival(
    track: &crate::models::Track,
    edge_source: NodeIndex,
    edge_target: NodeIndex,
    junction_idx: NodeIndex,
) -> bool {
    match track.direction {
        TrackDirection::Bidirectional => true,
        // Forward track allows source→target travel
        TrackDirection::Forward => edge_target == junction_idx,
        // Backward track allows target→source travel
        TrackDirection::Backward => edge_source == junction_idx,
    }
}

/// Check if a specific track allows departure from the junction
fn track_allows_departure(
    track: &crate::models::Track,
    edge_source: NodeIndex,
    edge_target: NodeIndex,
    junction_idx: NodeIndex,
) -> bool {
    match track.direction {
        TrackDirection::Bidirectional => true,
        // Forward track allows source→target travel
        TrackDirection::Forward => edge_source == junction_idx,
        // Backward track allows target→source travel
        TrackDirection::Backward => edge_target == junction_idx,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_junction_track_connections(
    ctx: &CanvasRenderingContext2d,
    from_track_indices: &[usize],
    to_track_indices: &[usize],
    from_total_tracks: usize,
    to_total_tracks: usize,
    entry_base: (f64, f64),
    exit_base: (f64, f64),
    from_perp: (f64, f64),
    to_perp: (f64, f64),
    zoom: f64,
) {
    // Calculate position offsets for all tracks on from edge
    #[allow(clippy::cast_precision_loss)]
    let from_total_width = (from_total_tracks - 1) as f64 * TRACK_SPACING;
    let from_start_offset = -from_total_width / 2.0;

    // Get positions only for the tracks that allow arrival
    let mut entry_points = Vec::new();
    for &track_idx in from_track_indices {
        #[allow(clippy::cast_precision_loss)]
        let offset = from_start_offset + (track_idx as f64 * TRACK_SPACING);
        entry_points.push((
            entry_base.0 + from_perp.0 * offset,
            entry_base.1 + from_perp.1 * offset
        ));
    }

    // Calculate position offsets for all tracks on to edge
    #[allow(clippy::cast_precision_loss)]
    let to_total_width = (to_total_tracks - 1) as f64 * TRACK_SPACING;
    let to_start_offset = -to_total_width / 2.0;

    // Get positions only for the tracks that allow departure
    let mut exit_points = Vec::new();
    for &track_idx in to_track_indices {
        #[allow(clippy::cast_precision_loss)]
        let offset = to_start_offset + (track_idx as f64 * TRACK_SPACING);
        exit_points.push((
            exit_base.0 + to_perp.0 * offset,
            exit_base.1 + to_perp.1 * offset
        ));
    }

    // Draw connections from every valid entry track to every valid exit track
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
    // Collect all connected edges - we need ALL edges connected to the junction
    // because an edge can have tracks going in either direction
    let mut all_edges: Vec<(EdgeIndex, (f64, f64))> = Vec::new();
    let mut seen_edges = std::collections::HashSet::new();

    // Incoming edges (where junction is target)
    for edge in graph.graph.edges_directed(idx, Direction::Incoming) {
        if seen_edges.insert(edge.id()) {
            if let Some(source_pos) = graph.get_station_position(edge.source()) {
                all_edges.push((edge.id(), source_pos));
            }
        }
    }

    // Outgoing edges (where junction is source)
    for edge in graph.graph.edges(idx) {
        if seen_edges.insert(edge.id()) {
            if let Some(target_pos) = graph.get_station_position(edge.target()) {
                all_edges.push((edge.id(), target_pos));
            }
        }
    }

    if all_edges.is_empty() {
        return;
    }

    let junction = graph.get_junction(idx);
    let Some(j) = junction else { return };

    ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

    // Draw connections between edges, checking track-by-track directionality
    for (i, (from_edge, from_node_pos)) in all_edges.iter().enumerate() {
        let Some(from_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *from_edge) else {
            continue;
        };
        let from_source = from_edge_ref.source();
        let from_target = from_edge_ref.target();
        let from_tracks = &from_edge_ref.weight().tracks;

        if from_tracks.is_empty() {
            continue;
        }

        // Check which tracks on this edge allow arrival at junction
        let arriving_tracks: Vec<usize> = from_tracks.iter()
            .enumerate()
            .filter(|(_, track)| track_allows_arrival(track, from_source, from_target, idx))
            .map(|(i, _)| i)
            .collect();

        if arriving_tracks.is_empty() {
            continue;
        }

        for (j_idx, (to_edge, to_node_pos)) in all_edges.iter().enumerate() {
            if i == j_idx {
                continue; // Skip same edge
            }

            // Check if this routing is allowed by junction
            let is_allowed = j.is_routing_allowed(*from_edge, *to_edge);
            if !is_allowed {
                continue;
            }

            let Some(to_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *to_edge) else {
                continue;
            };
            let to_source = to_edge_ref.source();
            let to_target = to_edge_ref.target();
            let to_tracks = &to_edge_ref.weight().tracks;

            if to_tracks.is_empty() {
                continue;
            }

            // Check which tracks on this edge allow departure from junction
            let departing_tracks: Vec<usize> = to_tracks.iter()
                .enumerate()
                .filter(|(_, track)| track_allows_departure(track, to_source, to_target, idx))
                .map(|(i, _)| i)
                .collect();

            if departing_tracks.is_empty() {
                continue;
            }

            // Now we have: arriving_tracks and departing_tracks indices
            if arriving_tracks.is_empty() || departing_tracks.is_empty() {
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

            // Calculate avoidance offset for this edge
            let (avoid_from_x, avoid_from_y) = track_renderer::calculate_avoidance_offset(
                graph, from_source_pos, from_target_pos, from_source, from_target
            );

            let entry_base = (
                pos.0 + (entry_delta.0 / entry_distance) * JUNCTION_TRACK_DISTANCE + avoid_from_x,
                pos.1 + (entry_delta.1 / entry_distance) * JUNCTION_TRACK_DISTANCE + avoid_from_y,
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

            // Calculate avoidance offset for this edge
            let (avoid_to_x, avoid_to_y) = track_renderer::calculate_avoidance_offset(
                graph, to_source_pos, to_target_pos, to_source, to_target
            );

            let exit_base = (
                pos.0 + (exit_delta.0 / exit_distance) * JUNCTION_TRACK_DISTANCE + avoid_to_x,
                pos.1 + (exit_delta.1 / exit_distance) * JUNCTION_TRACK_DISTANCE + avoid_to_y,
            );

            if arriving_tracks.len() == 1 && departing_tracks.len() == 1 {
                // Single track to single track - draw simple connection
                // Calculate the actual positions of these specific tracks
                #[allow(clippy::cast_precision_loss)]
                let from_total_width = (from_tracks.len() - 1) as f64 * TRACK_SPACING;
                let from_start_offset = -from_total_width / 2.0;
                #[allow(clippy::cast_precision_loss)]
                let from_offset = from_start_offset + (arriving_tracks[0] as f64 * TRACK_SPACING);

                #[allow(clippy::cast_precision_loss)]
                let to_total_width = (to_tracks.len() - 1) as f64 * TRACK_SPACING;
                let to_start_offset = -to_total_width / 2.0;
                #[allow(clippy::cast_precision_loss)]
                let to_offset = to_start_offset + (departing_tracks[0] as f64 * TRACK_SPACING);

                let entry_point = (
                    entry_base.0 + from_perp.0 * from_offset,
                    entry_base.1 + from_perp.1 * from_offset
                );
                let exit_point = (
                    exit_base.0 + to_perp.0 * to_offset,
                    exit_base.1 + to_perp.1 * to_offset
                );

                ctx.set_stroke_style_str(TRACK_COLOR);
                ctx.begin_path();
                ctx.move_to(entry_point.0, entry_point.1);
                ctx.line_to(exit_point.0, exit_point.1);
                ctx.stroke();
            } else {
                // Multiple tracks - connect every valid track to every other valid track
                draw_junction_track_connections(
                    ctx,
                    &arriving_tracks,
                    &departing_tracks,
                    from_tracks.len(),
                    to_tracks.len(),
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
