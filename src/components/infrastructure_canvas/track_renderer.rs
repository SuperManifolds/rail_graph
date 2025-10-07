use crate::models::RailwayGraph;
use petgraph::visit::EdgeRef;
use web_sys::CanvasRenderingContext2d;

const TRACK_SPACING: f64 = 3.0;
const STATION_AVOIDANCE_THRESHOLD: f64 = 20.0; // Minimum distance from station
const STATION_AVOIDANCE_OFFSET: f64 = 25.0; // How far to push track away
const TRANSITION_LENGTH: f64 = 30.0; // Distance over which to transition to/from offset

/// Check if a line segment from pos1 to pos2 passes near any stations (excluding source and target)
/// Returns a perpendicular offset to shift the track away from the station
fn calculate_avoidance_offset(
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
        if t < 0.1 || t > 0.9 {
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
    let needs_avoidance = avoid_x.abs() > 0.1 || avoid_y.abs() > 0.1;

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

        // Calculate perpendicular offset for parallel tracks
        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();
        let nx = -dy / len;
        let ny = dx / len;

        ctx.set_line_width(2.0 / zoom);

        // Check if we need to offset to avoid any stations
        let (avoid_x, avoid_y) = calculate_avoidance_offset(graph, pos1, pos2, source, target);
        let needs_avoidance = avoid_x.abs() > 0.1 || avoid_y.abs() > 0.1;

        if track_count == 1 {
            // Single track - draw in center (with avoidance if needed)
            ctx.set_stroke_style_str("#444");
            ctx.begin_path();

            if needs_avoidance {
                // Draw segmented path: start -> offset section -> end
                let segment_length = ((pos2.0 - pos1.0).powi(2) + (pos2.1 - pos1.1).powi(2)).sqrt();
                let transition_length = 30.0; // Distance over which to transition

                ctx.move_to(pos1.0, pos1.1);

                // Transition to offset
                let t1 = transition_length / segment_length;
                let mid1_x = pos1.0 + (pos2.0 - pos1.0) * t1;
                let mid1_y = pos1.1 + (pos2.1 - pos1.1) * t1;
                ctx.line_to(mid1_x + avoid_x, mid1_y + avoid_y);

                // Continue with offset
                let t2 = (segment_length - transition_length) / segment_length;
                let mid2_x = pos1.0 + (pos2.0 - pos1.0) * t2;
                let mid2_y = pos1.1 + (pos2.1 - pos1.1) * t2;
                ctx.line_to(mid2_x + avoid_x, mid2_y + avoid_y);

                // Transition back
                ctx.line_to(pos2.0, pos2.1);
            } else {
                ctx.move_to(pos1.0, pos1.1);
                ctx.line_to(pos2.0, pos2.1);
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

                ctx.set_stroke_style_str("#555");
                ctx.begin_path();

                if needs_avoidance {
                    // Draw segmented path with offset
                    let segment_length = ((pos2.0 - pos1.0).powi(2) + (pos2.1 - pos1.1).powi(2)).sqrt();
                    let transition_length = 30.0;

                    ctx.move_to(pos1.0 + ox, pos1.1 + oy);

                    let t1 = transition_length / segment_length;
                    let mid1_x = pos1.0 + (pos2.0 - pos1.0) * t1;
                    let mid1_y = pos1.1 + (pos2.1 - pos1.1) * t1;
                    ctx.line_to(mid1_x + ox + avoid_x, mid1_y + oy + avoid_y);

                    let t2 = (segment_length - transition_length) / segment_length;
                    let mid2_x = pos1.0 + (pos2.0 - pos1.0) * t2;
                    let mid2_y = pos1.1 + (pos2.1 - pos1.1) * t2;
                    ctx.line_to(mid2_x + ox + avoid_x, mid2_y + oy + avoid_y);

                    ctx.line_to(pos2.0 + ox, pos2.1 + oy);
                } else {
                    ctx.move_to(pos1.0 + ox, pos1.1 + oy);
                    ctx.line_to(pos2.0 + ox, pos2.1 + oy);
                }

                ctx.stroke();
            }
        }
    }
}
