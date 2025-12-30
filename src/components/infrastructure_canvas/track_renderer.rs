use crate::models::{RailwayGraph, Stations};
use crate::theme::Theme;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::{HashSet, HashMap};
use web_sys::CanvasRenderingContext2d;

const TRACK_SPACING: f64 = 3.0;
const TRACK_LINE_WIDTH: f64 = 2.0;
const JUNCTION_STOP_DISTANCE: f64 = 14.0;

struct Palette {
    track: &'static str,
    highlighted_track: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    track: "#444",
    highlighted_track: "#ffaa00",
};

const LIGHT_PALETTE: Palette = Palette {
    track: "#999",
    highlighted_track: "#ff8800",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

/// Get segments for a specific edge (used for both rendering and click detection)
#[must_use]
pub fn get_segments_for_edge(
    _graph: &RailwayGraph,
    _source: petgraph::graph::NodeIndex,
    _target: petgraph::graph::NodeIndex,
    pos1: (f64, f64),
    pos2: (f64, f64),
) -> Vec<((f64, f64), (f64, f64))> {
    // Simple straight line
    vec![(pos1, pos2)]
}

/// Get all track segments
/// Returns a list of line segments (start, end) that represent the actual drawn tracks
#[must_use]
pub fn get_track_segments(graph: &RailwayGraph) -> Vec<((f64, f64), (f64, f64))> {
    let mut segments = Vec::new();

    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();
        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        segments.push((pos1, pos2));
    }

    segments
}

/// Calculate track endpoints considering crossover intersections for orphaned tracks
/// Returns (endpoint1, endpoint2, `use_offset1`, `use_offset2`)
#[allow(clippy::too_many_arguments)]
fn calculate_track_endpoints_for_edge(
    track_idx: usize,
    pos1: (f64, f64),
    pos2: (f64, f64),
    dx: f64,
    dy: f64,
    len: f64,
    junction_distance: f64,
    source_is_junction: bool,
    target_is_junction: bool,
    source: NodeIndex,
    target: NodeIndex,
    edge_id: EdgeIndex,
    orphaned_tracks: &HashMap<(EdgeIndex, NodeIndex), HashSet<usize>>,
    crossover_intersections: &HashMap<(EdgeIndex, NodeIndex, usize), (f64, f64)>,
) -> ((f64, f64), (f64, f64), bool, bool) {
    let mut actual_pos1 = pos1;
    let mut actual_pos2 = pos2;
    let mut use_offset1 = true;
    let mut use_offset2 = true;

    // Check if this track is orphaned at source junction
    if source_is_junction {
        let is_orphaned = orphaned_tracks
            .get(&(edge_id, source))
            .is_some_and(|set| set.contains(&track_idx));

        if is_orphaned && crossover_intersections.contains_key(&(edge_id, source, track_idx)) {
            actual_pos1 = crossover_intersections[&(edge_id, source, track_idx)];
            use_offset1 = false;
        } else if len > junction_distance {
            let t = junction_distance / len;
            actual_pos1 = (pos1.0 + dx * t, pos1.1 + dy * t);
        }
    }

    // Check if this track is orphaned at target junction
    if target_is_junction {
        let is_orphaned = orphaned_tracks
            .get(&(edge_id, target))
            .is_some_and(|set| set.contains(&track_idx));

        if is_orphaned && crossover_intersections.contains_key(&(edge_id, target, track_idx)) {
            actual_pos2 = crossover_intersections[&(edge_id, target, track_idx)];
            use_offset2 = false;
        } else if len > junction_distance {
            let t = junction_distance / len;
            actual_pos2 = (pos2.0 - dx * t, pos2.1 - dy * t);
        }
    }

    (actual_pos1, actual_pos2, use_offset1, use_offset2)
}

#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn draw_tracks(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
    highlighted_edges: &HashSet<petgraph::stable_graph::EdgeIndex>,
    viewport_bounds: (f64, f64, f64, f64),
    junctions: &HashSet<NodeIndex>,
    theme: Theme,
    orphaned_tracks: &HashMap<(EdgeIndex, NodeIndex), HashSet<usize>>,
    crossover_intersections: &HashMap<(EdgeIndex, NodeIndex, usize), (f64, f64)>,
) {
    let palette = get_palette(theme);
    let (left, top, right, bottom) = viewport_bounds;
    let margin = 200.0; // Buffer to include tracks slightly outside viewport

    for edge in graph.graph.edge_references() {
        let edge_id = edge.id();
        let source = edge.source();
        let target = edge.target();
        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        // Viewport culling: skip tracks completely outside visible area
        let min_x = pos1.0.min(pos2.0);
        let max_x = pos1.0.max(pos2.0);
        let min_y = pos1.1.min(pos2.1);
        let max_y = pos1.1.max(pos2.1);

        if max_x < left - margin || min_x > right + margin ||
           max_y < top - margin || min_y > bottom + margin {
            continue;
        }

        let track_count = edge.weight().tracks.len();

        if track_count == 0 {
            continue;
        }

        // Check if this edge is highlighted (part of preview path)
        let is_highlighted = highlighted_edges.contains(&edge_id);
        let track_color = if is_highlighted { palette.highlighted_track } else { palette.track };

        // Check if source or target is a junction (use cached set)
        let source_is_junction = junctions.contains(&source);
        let target_is_junction = junctions.contains(&target);

        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();

        // Calculate perpendicular offset for parallel tracks
        let nx = -dy / len;
        let ny = dx / len;

        ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

        if track_count == 1 {
            // Single track - draw straight line
            let (actual_pos1, actual_pos2, _, _) = calculate_track_endpoints_for_edge(
                0, pos1, pos2, dx, dy, len, JUNCTION_STOP_DISTANCE,
                source_is_junction, target_is_junction, source, target, edge_id,
                orphaned_tracks, crossover_intersections,
            );

            ctx.set_stroke_style_str(track_color);
            ctx.begin_path();
            ctx.move_to(actual_pos1.0, actual_pos1.1);
            ctx.line_to(actual_pos2.0, actual_pos2.1);
            ctx.stroke();
        } else {
            // Multiple tracks - distribute evenly
            let total_width = (track_count - 1) as f64 * TRACK_SPACING;
            let start_offset = -total_width / 2.0;

            for (i, _track) in edge.weight().tracks.iter().enumerate() {
                let offset = start_offset + (i as f64 * TRACK_SPACING);
                let ox = nx * offset;
                let oy = ny * offset;

                // Calculate endpoints for this specific track (may differ if orphaned)
                let (actual_pos1, actual_pos2, use_offset1, use_offset2) = calculate_track_endpoints_for_edge(
                    i, pos1, pos2, dx, dy, len, JUNCTION_STOP_DISTANCE,
                    source_is_junction, target_is_junction, source, target, edge_id,
                    orphaned_tracks, crossover_intersections,
                );

                ctx.set_stroke_style_str(track_color);
                ctx.begin_path();

                // Apply offset only if not already in the endpoint coordinates
                let offset1 = if use_offset1 { (ox, oy) } else { (0.0, 0.0) };
                let offset2 = if use_offset2 { (ox, oy) } else { (0.0, 0.0) };

                ctx.move_to(actual_pos1.0 + offset1.0, actual_pos1.1 + offset1.1);
                ctx.line_to(actual_pos2.0 + offset2.0, actual_pos2.1 + offset2.1);

                ctx.stroke();
            }
        }
    }
}

/// Draw tracks, excluding edges that have scheduled lines
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn draw_tracks_filtered(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
    highlighted_edges: &HashSet<petgraph::stable_graph::EdgeIndex>,
    viewport_bounds: (f64, f64, f64, f64),
    junctions: &HashSet<NodeIndex>,
    theme: Theme,
    orphaned_tracks: &HashMap<(EdgeIndex, NodeIndex), HashSet<usize>>,
    crossover_intersections: &HashMap<(EdgeIndex, NodeIndex, usize), (f64, f64)>,
    excluded_edges: &HashSet<EdgeIndex>,
) {
    let palette = get_palette(theme);
    let (left, top, right, bottom) = viewport_bounds;
    let margin = 200.0; // Buffer to include tracks slightly outside viewport

    for edge in graph.graph.edge_references() {
        let edge_id = edge.id();

        // Skip edges that have scheduled lines (they'll be drawn as lines instead)
        if excluded_edges.contains(&edge_id) {
            continue;
        }

        let source = edge.source();
        let target = edge.target();
        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        // Viewport culling: skip tracks completely outside visible area
        let min_x = pos1.0.min(pos2.0);
        let max_x = pos1.0.max(pos2.0);
        let min_y = pos1.1.min(pos2.1);
        let max_y = pos1.1.max(pos2.1);

        if max_x < left - margin || min_x > right + margin ||
           max_y < top - margin || min_y > bottom + margin {
            continue;
        }

        let track_count = edge.weight().tracks.len();

        if track_count == 0 {
            continue;
        }

        // Check if this edge is highlighted (part of preview path)
        let is_highlighted = highlighted_edges.contains(&edge_id);
        let track_color = if is_highlighted { palette.highlighted_track } else { palette.track };

        // Check if source or target is a junction (use cached set)
        let source_is_junction = junctions.contains(&source);
        let target_is_junction = junctions.contains(&target);

        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();

        // Calculate perpendicular offset for parallel tracks
        let nx = -dy / len;
        let ny = dx / len;

        ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

        if track_count == 1 {
            // Single track - draw straight line
            let (actual_pos1, actual_pos2, _, _) = calculate_track_endpoints_for_edge(
                0, pos1, pos2, dx, dy, len, JUNCTION_STOP_DISTANCE,
                source_is_junction, target_is_junction, source, target, edge_id,
                orphaned_tracks, crossover_intersections,
            );

            ctx.set_stroke_style_str(track_color);
            ctx.begin_path();
            ctx.move_to(actual_pos1.0, actual_pos1.1);
            ctx.line_to(actual_pos2.0, actual_pos2.1);
            ctx.stroke();
        } else {
            // Multiple tracks - distribute evenly
            let total_width = (track_count - 1) as f64 * TRACK_SPACING;
            let start_offset = -total_width / 2.0;

            for (i, _track) in edge.weight().tracks.iter().enumerate() {
                let offset = start_offset + (i as f64 * TRACK_SPACING);
                let ox = nx * offset;
                let oy = ny * offset;

                // Calculate endpoints for this specific track (may differ if orphaned)
                let (actual_pos1, actual_pos2, use_offset1, use_offset2) = calculate_track_endpoints_for_edge(
                    i, pos1, pos2, dx, dy, len, JUNCTION_STOP_DISTANCE,
                    source_is_junction, target_is_junction, source, target, edge_id,
                    orphaned_tracks, crossover_intersections,
                );

                ctx.set_stroke_style_str(track_color);
                ctx.begin_path();

                // Apply offset only if not already in the endpoint coordinates
                let offset1 = if use_offset1 { (ox, oy) } else { (0.0, 0.0) };
                let offset2 = if use_offset2 { (ox, oy) } else { (0.0, 0.0) };

                ctx.move_to(actual_pos1.0 + offset1.0, actual_pos1.1 + offset1.1);
                ctx.line_to(actual_pos2.0 + offset2.0, actual_pos2.1 + offset2.1);

                ctx.stroke();
            }
        }
    }
}
