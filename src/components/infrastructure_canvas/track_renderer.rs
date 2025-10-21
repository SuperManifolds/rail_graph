use crate::models::{RailwayGraph, Stations, Junctions};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use web_sys::CanvasRenderingContext2d;

// Track layout constants
const TRACK_SPACING: f64 = 3.0;
const STATION_AVOIDANCE_THRESHOLD: f64 = 20.0; // Minimum distance from station
const STATION_AVOIDANCE_OFFSET: f64 = 25.0; // How far to push track away
const TRANSITION_LENGTH: f64 = 30.0; // Distance over which to transition to/from offset
const AVOIDANCE_OFFSET_THRESHOLD: f64 = 0.1; // Minimum offset to trigger avoidance rendering
const PROJECTION_MIN: f64 = 0.1; // Minimum projection parameter for station checking
const PROJECTION_MAX: f64 = 0.9; // Maximum projection parameter for station checking

// Track rendering constants
const TRACK_LINE_WIDTH: f64 = 2.0;
const TRACK_COLOR: &str = "#444";
const JUNCTION_STOP_DISTANCE: f64 = 14.0; // Stop drawing tracks this far from junction center

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

            // Return perpendicular offset to shift entire track away from station
            return (perp_x * side * STATION_AVOIDANCE_OFFSET, perp_y * side * STATION_AVOIDANCE_OFFSET);
        }
    }

    (0.0, 0.0)
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

#[allow(clippy::cast_precision_loss)]
pub fn draw_tracks(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
) {
    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();
        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        let track_count = edge.weight().tracks.len();

        if track_count == 0 {
            continue;
        }

        // Check if source or target is a junction
        let source_is_junction = graph.is_junction(source);
        let target_is_junction = graph.is_junction(target);

        // Check if we need to offset to avoid any stations
        let (avoid_x, avoid_y) = calculate_avoidance_offset(graph, pos1, pos2, source, target);
        let needs_avoidance = avoid_x.abs() > AVOIDANCE_OFFSET_THRESHOLD || avoid_y.abs() > AVOIDANCE_OFFSET_THRESHOLD;

        // Calculate actual start and end points, stopping before junctions
        let mut actual_pos1 = pos1;
        let mut actual_pos2 = pos2;

        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();

        // When there's avoidance offset, use half junction distance to match junction renderer
        let junction_distance = if needs_avoidance {
            JUNCTION_STOP_DISTANCE * 0.5
        } else {
            JUNCTION_STOP_DISTANCE
        };

        if source_is_junction && len > junction_distance {
            // Move start point away from junction
            let t = junction_distance / len;
            actual_pos1 = (pos1.0 + dx * t, pos1.1 + dy * t);
        }

        if target_is_junction && len > junction_distance {
            // Move end point away from junction
            let t = junction_distance / len;
            actual_pos2 = (pos2.0 - dx * t, pos2.1 - dy * t);
        }

        // Calculate perpendicular offset for parallel tracks
        let nx = -dy / len;
        let ny = dx / len;

        ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

        if track_count == 1 {
            // Single track - draw in center (with avoidance if needed)
            ctx.set_stroke_style_str(TRACK_COLOR);
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

            for (i, _track) in edge.weight().tracks.iter().enumerate() {
                let offset = start_offset + (i as f64 * TRACK_SPACING);
                let ox = nx * offset;
                let oy = ny * offset;

                ctx.set_stroke_style_str(TRACK_COLOR);
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
                    ctx.move_to(actual_pos1.0 + ox, actual_pos1.1 + oy);
                    ctx.line_to(actual_pos2.0 + ox, actual_pos2.1 + oy);
                }

                ctx.stroke();
            }
        }
    }
}
