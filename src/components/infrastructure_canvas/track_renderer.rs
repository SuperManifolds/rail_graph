use crate::models::{RailwayGraph, Stations};
use crate::theme::Theme;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::{HashSet, HashMap};
use web_sys::CanvasRenderingContext2d;

const TRACK_SPACING: f64 = 3.0;
const STATION_AVOIDANCE_THRESHOLD: f64 = 20.0;
const STATION_AVOIDANCE_OFFSET: f64 = 25.0;
const TRANSITION_LENGTH: f64 = 30.0;
const AVOIDANCE_OFFSET_THRESHOLD: f64 = 0.1;
const PROJECTION_MIN: f64 = 0.1;
const PROJECTION_MAX: f64 = 0.9;

const TRACK_LINE_WIDTH: f64 = 2.0;
const JUNCTION_STOP_DISTANCE: f64 = 14.0;

struct Palette {
    track: &'static str,
    highlighted_track: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    track: "#444",
    highlighted_track: "#4a9eff",
};

const LIGHT_PALETTE: Palette = Palette {
    track: "#999",
    highlighted_track: "#1976d2",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

/// Draw a track segment with optional avoidance transitions
fn draw_track_segment_with_avoidance(
    ctx: &CanvasRenderingContext2d,
    pos1: (f64, f64),
    pos2: (f64, f64),
    segment_length: f64,
    track_offset: (f64, f64),
    avoidance_offset: (f64, f64),
    transitions: (bool, bool),
) {
    let (ox, oy) = track_offset;
    let (avoid_x, avoid_y) = avoidance_offset;
    let (start_needs_transition, end_needs_transition) = transitions;

    if start_needs_transition {
        ctx.move_to(pos1.0 + ox, pos1.1 + oy);
        let t1 = TRANSITION_LENGTH / segment_length;
        let mid1_x = pos1.0 + (pos2.0 - pos1.0) * t1;
        let mid1_y = pos1.1 + (pos2.1 - pos1.1) * t1;
        ctx.line_to(mid1_x + ox + avoid_x, mid1_y + oy + avoid_y);
    } else {
        ctx.move_to(pos1.0 + ox + avoid_x, pos1.1 + oy + avoid_y);
    }

    if end_needs_transition {
        let t2 = (segment_length - TRANSITION_LENGTH) / segment_length;
        let mid2_x = pos1.0 + (pos2.0 - pos1.0) * t2;
        let mid2_y = pos1.1 + (pos2.1 - pos1.1) * t2;
        ctx.line_to(mid2_x + ox + avoid_x, mid2_y + oy + avoid_y);
        ctx.line_to(pos2.0 + ox, pos2.1 + oy);
    } else {
        ctx.line_to(pos2.0 + ox + avoid_x, pos2.1 + oy + avoid_y);
    }
}

/// Check if applying an offset would cause overlap with other track segments
fn would_overlap_other_tracks(
    graph: &RailwayGraph,
    pos1: (f64, f64),
    pos2: (f64, f64),
    offset: (f64, f64),
    source: petgraph::graph::NodeIndex,
    target: petgraph::graph::NodeIndex,
) -> bool {
    const MIN_TRACK_DISTANCE: f64 = 15.0;

    // Check multiple points along the offset segment
    let sample_points = 5;
    for i in 0..=sample_points {
        let t = f64::from(i) / f64::from(sample_points);
        let sample_x = pos1.0 + (pos2.0 - pos1.0) * t + offset.0;
        let sample_y = pos1.1 + (pos2.1 - pos1.1) * t + offset.1;

        // Check all other edges
        for edge in graph.graph.edge_references() {
            let other_source = edge.source();
            let other_target = edge.target();

            // Skip if this is the same edge
            if (other_source == source && other_target == target) ||
               (other_source == target && other_target == source) {
                continue;
            }

            let Some(other_pos1) = graph.get_station_position(other_source) else { continue };
            let Some(other_pos2) = graph.get_station_position(other_target) else { continue };

            // Quick bounding box check - skip if edges are far apart
            let margin = MIN_TRACK_DISTANCE + 10.0; // Add margin for offset
            let min_x = pos1.0.min(pos2.0) - margin;
            let max_x = pos1.0.max(pos2.0) + margin;
            let min_y = pos1.1.min(pos2.1) - margin;
            let max_y = pos1.1.max(pos2.1) + margin;

            let other_min_x = other_pos1.0.min(other_pos2.0);
            let other_max_x = other_pos1.0.max(other_pos2.0);
            let other_min_y = other_pos1.1.min(other_pos2.1);
            let other_max_y = other_pos1.1.max(other_pos2.1);

            // Skip if bounding boxes don't overlap
            if max_x < other_min_x || min_x > other_max_x ||
               max_y < other_min_y || min_y > other_max_y {
                continue;
            }

            // Also check if the other track has avoidance offset
            let other_offset = calculate_avoidance_offset_internal(graph, other_pos1, other_pos2, other_source, other_target, false);

            let dx = other_pos2.0 - other_pos1.0;
            let dy = other_pos2.1 - other_pos1.1;
            let len_sq = dx * dx + dy * dy;

            if len_sq < 0.01 {
                continue;
            }

            // Project sample point onto other segment
            let proj_t = ((sample_x - other_pos1.0) * dx + (sample_y - other_pos1.1) * dy) / len_sq;
            let proj_t_clamped = proj_t.clamp(0.0, 1.0);

            // Calculate closest point on other segment (with its offset)
            let closest_x = other_pos1.0 + proj_t_clamped * dx + other_offset.0;
            let closest_y = other_pos1.1 + proj_t_clamped * dy + other_offset.1;

            let dist_x = sample_x - closest_x;
            let dist_y = sample_y - closest_y;
            let dist = (dist_x * dist_x + dist_y * dist_y).sqrt();

            if dist < MIN_TRACK_DISTANCE {
                return true;
            }
        }
    }

    false
}

/// Internal version of `calculate_avoidance_offset` that can skip overlap checking
fn calculate_avoidance_offset_internal(
    graph: &RailwayGraph,
    pos1: (f64, f64),
    pos2: (f64, f64),
    source: petgraph::graph::NodeIndex,
    target: petgraph::graph::NodeIndex,
    check_overlaps: bool,
) -> (f64, f64) {
    // Check all stations
    for node_idx in graph.graph.node_indices() {
        // Skip source and target stations
        if node_idx == source || node_idx == target {
            continue;
        }

        let Some(station_pos) = graph.get_station_position(node_idx) else { continue };

        // Calculate distance from station to line segment
        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len_sq = dx * dx + dy * dy;

        if len_sq == 0.0 {
            continue;
        }

        // Calculate projection parameter t
        let t = ((station_pos.0 - pos1.0) * dx + (station_pos.1 - pos1.1) * dy) / len_sq;

        // Only check if station is between the two endpoints (not beyond them)
        if !(PROJECTION_MIN..=PROJECTION_MAX).contains(&t) {
            continue;
        }

        // Find closest point on line segment
        let closest_x = pos1.0 + t * dx;
        let closest_y = pos1.1 + t * dy;

        // Calculate distance to station
        let dist_x = station_pos.0 - closest_x;
        let dist_y = station_pos.1 - closest_y;
        let dist = (dist_x * dist_x + dist_y * dist_y).sqrt();

        // If too close, calculate perpendicular offset to push track away
        if dist < STATION_AVOIDANCE_THRESHOLD {
            // Calculate perpendicular direction
            let len = len_sq.sqrt();
            let perp_x = -dy / len;
            let perp_y = dx / len;

            // Determine which side the station is on
            let cross = dx * (station_pos.1 - pos1.1) - dy * (station_pos.0 - pos1.0);
            let side = if cross > 0.0 { -1.0 } else { 1.0 };

            // Calculate proposed offset
            let proposed_offset = (perp_x * side * STATION_AVOIDANCE_OFFSET, perp_y * side * STATION_AVOIDANCE_OFFSET);

            // Check if this offset would cause overlap with other tracks (if requested)
            if check_overlaps && would_overlap_other_tracks(graph, pos1, pos2, proposed_offset, source, target) {
                // Don't apply avoidance if it would cause overlap
                return (0.0, 0.0);
            }

            // Return perpendicular offset to shift entire track away from station
            return proposed_offset;
        }
    }

    (0.0, 0.0)
}

/// Check if a line segment from pos1 to pos2 passes near any stations (excluding source and target)
/// Returns a perpendicular offset to shift the track away from the station
#[must_use]
pub fn calculate_avoidance_offset(
    graph: &RailwayGraph,
    pos1: (f64, f64),
    pos2: (f64, f64),
    source: petgraph::graph::NodeIndex,
    target: petgraph::graph::NodeIndex,
) -> (f64, f64) {
    calculate_avoidance_offset_internal(graph, pos1, pos2, source, target, true)
}

/// Get segments for a specific edge (used for both rendering and click detection)
#[must_use]
pub fn get_segments_for_edge(
    graph: &RailwayGraph,
    source: petgraph::graph::NodeIndex,
    target: petgraph::graph::NodeIndex,
    pos1: (f64, f64),
    pos2: (f64, f64),
) -> Vec<((f64, f64), (f64, f64))> {
    let mut segments = Vec::new();

    // Check if we need to offset to avoid any stations
    let (avoid_x, avoid_y) = calculate_avoidance_offset(graph, pos1, pos2, source, target);
    let needs_avoidance = avoid_x.abs() > AVOIDANCE_OFFSET_THRESHOLD || avoid_y.abs() > AVOIDANCE_OFFSET_THRESHOLD;

    if needs_avoidance {
        // Add segmented path
        let segment_length = ((pos2.0 - pos1.0).powi(2) + (pos2.1 - pos1.1).powi(2)).sqrt();

        // First segment: start to first transition
        let t1 = TRANSITION_LENGTH / segment_length;
        let mid1_x = pos1.0 + (pos2.0 - pos1.0) * t1;
        let mid1_y = pos1.1 + (pos2.1 - pos1.1) * t1;
        segments.push((pos1, (mid1_x + avoid_x, mid1_y + avoid_y)));

        // Middle segment: offset section
        let t2 = (segment_length - TRANSITION_LENGTH) / segment_length;
        let mid2_x = pos1.0 + (pos2.0 - pos1.0) * t2;
        let mid2_y = pos1.1 + (pos2.1 - pos1.1) * t2;
        segments.push(((mid1_x + avoid_x, mid1_y + avoid_y), (mid2_x + avoid_x, mid2_y + avoid_y)));

        // Last segment: second transition to end
        segments.push(((mid2_x + avoid_x, mid2_y + avoid_y), pos2));
    } else {
        // Simple straight line
        segments.push((pos1, pos2));
    }

    segments
}

/// Get all track segments including intermediate points for avoidance
/// Returns a list of line segments (start, end) that represent the actual drawn tracks
#[must_use]
pub fn get_track_segments(graph: &RailwayGraph) -> Vec<((f64, f64), (f64, f64))> {
    let mut segments = Vec::new();

    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();
        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        segments.extend(get_segments_for_edge(graph, source, target, pos1, pos2));
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
    cached_avoidance: &HashMap<EdgeIndex, (f64, f64)>,
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

        // Use cached avoidance offset
        let (avoid_x, avoid_y) = cached_avoidance.get(&edge_id).copied().unwrap_or((0.0, 0.0));
        let needs_avoidance = avoid_x.abs() > AVOIDANCE_OFFSET_THRESHOLD || avoid_y.abs() > AVOIDANCE_OFFSET_THRESHOLD;

        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();

        // Calculate perpendicular offset for parallel tracks
        let nx = -dy / len;
        let ny = dx / len;

        // When there's avoidance offset, use half junction distance to match junction renderer
        let junction_distance = if needs_avoidance {
            JUNCTION_STOP_DISTANCE * 0.5
        } else {
            JUNCTION_STOP_DISTANCE
        };

        ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

        if track_count == 1 {
            // Single track - draw in center (with avoidance if needed)
            let (actual_pos1, actual_pos2, _, _) = calculate_track_endpoints_for_edge(
                0, pos1, pos2, dx, dy, len, junction_distance,
                source_is_junction, target_is_junction, source, target, edge_id,
                orphaned_tracks, crossover_intersections,
            );

            ctx.set_stroke_style_str(track_color);
            ctx.begin_path();

            if needs_avoidance {
                // Draw segmented path: start -> offset section -> end
                let segment_length = ((actual_pos2.0 - actual_pos1.0).powi(2) + (actual_pos2.1 - actual_pos1.1).powi(2)).sqrt();

                // Check if we're connecting to junctions (which handle the avoidance offset themselves)
                let start_needs_transition = !source_is_junction;
                let end_needs_transition = !target_is_junction;

                draw_track_segment_with_avoidance(
                    ctx, actual_pos1, actual_pos2, segment_length,
                    (0.0, 0.0), (avoid_x, avoid_y),
                    (start_needs_transition, end_needs_transition)
                );
            } else {
                ctx.move_to(actual_pos1.0, actual_pos1.1);
                ctx.line_to(actual_pos2.0, actual_pos2.1);
            }

            ctx.stroke();
        } else {
            // Multiple tracks - distribute evenly (with avoidance if needed)
            let total_width = (track_count - 1) as f64 * TRACK_SPACING;
            let start_offset = -total_width / 2.0;

            #[allow(clippy::excessive_nesting)]
            for (i, _track) in edge.weight().tracks.iter().enumerate() {
                let offset = start_offset + (i as f64 * TRACK_SPACING);
                let ox = nx * offset;
                let oy = ny * offset;

                // Calculate endpoints for this specific track (may differ if orphaned)
                let (actual_pos1, actual_pos2, use_offset1, use_offset2) = calculate_track_endpoints_for_edge(
                    i, pos1, pos2, dx, dy, len, junction_distance,
                    source_is_junction, target_is_junction, source, target, edge_id,
                    orphaned_tracks, crossover_intersections,
                );

                ctx.set_stroke_style_str(track_color);
                ctx.begin_path();

                if needs_avoidance {
                    // Draw segmented path with offset
                    let segment_length = ((actual_pos2.0 - actual_pos1.0).powi(2) + (actual_pos2.1 - actual_pos1.1).powi(2)).sqrt();

                    // Check if we're connecting to junctions (which handle the avoidance offset themselves)
                    let start_needs_transition = !source_is_junction;
                    let end_needs_transition = !target_is_junction;

                    draw_track_segment_with_avoidance(
                        ctx, actual_pos1, actual_pos2, segment_length,
                        (ox, oy), (avoid_x, avoid_y),
                        (start_needs_transition, end_needs_transition)
                    );
                } else {
                    // Apply offset only if not already in the endpoint coordinates
                    let offset1 = if use_offset1 { (ox, oy) } else { (0.0, 0.0) };
                    let offset2 = if use_offset2 { (ox, oy) } else { (0.0, 0.0) };

                    ctx.move_to(actual_pos1.0 + offset1.0, actual_pos1.1 + offset1.1);
                    ctx.line_to(actual_pos2.0 + offset2.0, actual_pos2.1 + offset2.1);
                }

                ctx.stroke();
            }
        }
    }
}
