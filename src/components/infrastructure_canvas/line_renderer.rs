use crate::models::{Line, RailwayGraph, Stations};
use crate::theme::Theme;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::{HashMap, HashSet};
use web_sys::CanvasRenderingContext2d;

const LINE_BASE_WIDTH: f64 = 3.0;
const AVOIDANCE_OFFSET_THRESHOLD: f64 = 0.1;
const TRANSITION_LENGTH: f64 = 30.0;
const JUNCTION_STOP_DISTANCE: f64 = 14.0;

/// Key for identifying a junction connection between two edges
#[derive(Hash, Eq, PartialEq, Clone, Copy)]
struct JunctionConnectionKey {
    junction: NodeIndex,
    from_edge: EdgeIndex,
    to_edge: EdgeIndex,
}

/// Draw lines through junctions
#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
fn draw_junction_connections(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    junction_connections: &HashMap<JunctionConnectionKey, Vec<&Line>>,
    edge_to_lines: &HashMap<EdgeIndex, Vec<&Line>>,
    zoom: f64,
) {
    for (connection_key, connection_lines) in junction_connections {
        let Some(junction_pos) = graph.get_station_position(connection_key.junction) else {
            continue;
        };

        // Get the endpoints of both edges
        let Some((from_src, from_tgt)) = graph.graph.edge_endpoints(connection_key.from_edge) else {
            continue;
        };
        let Some((to_src, to_tgt)) = graph.graph.edge_endpoints(connection_key.to_edge) else {
            continue;
        };

        // Determine the node positions for entry and exit edges
        // We need to know if junction is source or target to calculate direction correctly
        let from_junction_is_target = from_tgt == connection_key.junction;
        let to_junction_is_source = to_src == connection_key.junction;

        let from_node_pos = if from_junction_is_target {
            graph.get_station_position(from_src)
        } else {
            graph.get_station_position(from_tgt)
        };

        let to_node_pos = if to_junction_is_source {
            graph.get_station_position(to_tgt)
        } else {
            graph.get_station_position(to_src)
        };

        let (Some(from_pos), Some(to_pos)) = (from_node_pos, to_node_pos) else {
            continue;
        };

        // Calculate edge directions FROM source TO target (matching edge drawing code)
        // For entry edge: if junction is target, edge goes from_pos -> junction
        //                 if junction is source, edge goes junction -> from_pos
        let entry_delta = if from_junction_is_target {
            (junction_pos.0 - from_pos.0, junction_pos.1 - from_pos.1)
        } else {
            (from_pos.0 - junction_pos.0, from_pos.1 - junction_pos.1)
        };
        let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();

        // For exit edge: if junction is source, edge goes junction -> to_pos
        //                if junction is target, edge goes to_pos -> junction
        let exit_delta = if to_junction_is_source {
            (to_pos.0 - junction_pos.0, to_pos.1 - junction_pos.1)
        } else {
            (junction_pos.0 - to_pos.0, junction_pos.1 - to_pos.1)
        };
        let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();

        if entry_distance < 0.1 || exit_distance < 0.1 {
            continue;
        }

        // Get the line lists for each edge to find actual positions
        let Some(from_edge_lines) = edge_to_lines.get(&connection_key.from_edge) else {
            continue;
        };
        let Some(to_edge_lines) = edge_to_lines.get(&connection_key.to_edge) else {
            continue;
        };

        // Calculate base stop points without extension
        let entry_base_no_ext = if from_junction_is_target {
            (
                junction_pos.0 - (entry_delta.0 / entry_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 - (entry_delta.1 / entry_distance) * JUNCTION_STOP_DISTANCE,
            )
        } else {
            (
                junction_pos.0 + (entry_delta.0 / entry_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 + (entry_delta.1 / entry_distance) * JUNCTION_STOP_DISTANCE,
            )
        };

        let exit_base_no_ext = if to_junction_is_source {
            (
                junction_pos.0 + (exit_delta.0 / exit_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 + (exit_delta.1 / exit_distance) * JUNCTION_STOP_DISTANCE,
            )
        } else {
            (
                junction_pos.0 - (exit_delta.0 / exit_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 - (exit_delta.1 / exit_distance) * JUNCTION_STOP_DISTANCE,
            )
        };

        // Calculate perpendicular vectors for offsets (matching edge drawing code)
        // Perpendicular is (-dy/len, dx/len) where (dx, dy) is the edge direction
        let entry_perp = (-entry_delta.1 / entry_distance, entry_delta.0 / entry_distance);
        let mut exit_perp = (-exit_delta.1 / exit_distance, exit_delta.0 / exit_distance);

        // Check if perpendiculars point in opposite directions
        let dot_product = entry_perp.0 * exit_perp.0 + entry_perp.1 * exit_perp.1;
        let flip_exit_offsets = if dot_product < 0.0 {
            // Flip exit perpendicular so they point in the same global direction
            exit_perp = (-exit_perp.0, -exit_perp.1);
            true
        } else {
            false
        };

        // Sort edge lines for consistent ordering
        let mut sorted_from_lines = from_edge_lines.clone();
        sorted_from_lines.sort_by_key(|line| line.id);

        let mut sorted_to_lines = to_edge_lines.clone();
        sorted_to_lines.sort_by_key(|line| line.id);

        // If perpendiculars point in opposite directions, reverse both line orders
        // to account for the geometric flip
        if flip_exit_offsets {
            sorted_from_lines.reverse();
            sorted_to_lines.reverse();
        }

        // Calculate widths for each edge
        let from_widths: Vec<f64> = sorted_from_lines.iter()
            .map(|line| (LINE_BASE_WIDTH + line.thickness) / zoom)
            .collect();

        let to_widths: Vec<f64> = sorted_to_lines.iter()
            .map(|line| (LINE_BASE_WIDTH + line.thickness) / zoom)
            .collect();

        // Gap should be equal to width of a standard line (BASE + default thickness of 2.0)
        let gap_width = (LINE_BASE_WIDTH + 2.0) / zoom;
        let from_num_gaps = from_widths.len().saturating_sub(1);
        let to_num_gaps = to_widths.len().saturating_sub(1);
        let from_total_width: f64 = from_widths.iter().sum::<f64>()
            + (from_num_gaps as f64) * gap_width;
        let to_total_width: f64 = to_widths.iter().sum::<f64>()
            + (to_num_gaps as f64) * gap_width;

        // Draw each line through the junction from its position on entry edge to its position on exit edge
        for line in connection_lines {
            // Find position on entry edge
            let Some(from_idx) = sorted_from_lines.iter().position(|l| l.id == line.id) else {
                continue;
            };

            let entry_offset = {
                let start_offset = -from_total_width / 2.0;
                let mut offset = start_offset;
                for width in from_widths.iter().take(from_idx) {
                    offset += width + gap_width;
                }
                offset + from_widths[from_idx] / 2.0
            };

            // Find position on exit edge
            let Some(to_idx) = sorted_to_lines.iter().position(|l| l.id == line.id) else {
                continue;
            };

            let exit_offset = {
                let start_offset = -to_total_width / 2.0;
                let mut offset = start_offset;
                for width in to_widths.iter().take(to_idx) {
                    offset += width + gap_width;
                }
                offset + to_widths[to_idx] / 2.0
            };

            // Apply perpendicular offsets to base points for line positioning
            let entry_point = (
                entry_base_no_ext.0 + entry_perp.0 * entry_offset,
                entry_base_no_ext.1 + entry_perp.1 * entry_offset
            );

            let exit_point = (
                exit_base_no_ext.0 + exit_perp.0 * exit_offset,
                exit_base_no_ext.1 + exit_perp.1 * exit_offset
            );

            let line_world_width = (LINE_BASE_WIDTH + line.thickness) / zoom;
            ctx.set_line_width(line_world_width);
            ctx.set_stroke_style_str(&line.color);
            ctx.begin_path();
            ctx.move_to(entry_point.0, entry_point.1);

            // Find control point by intersecting straight lines along edge directions
            // Line 1: entry_point + t * entry_delta (continuing along entry edge)
            // Line 2: exit_point + s * (-exit_delta) (going back along exit edge)
            let entry_dir = (entry_delta.0 / entry_distance, entry_delta.1 / entry_distance);
            let exit_dir_back = (-exit_delta.0 / exit_distance, -exit_delta.1 / exit_distance);

            // Find intersection: entry_point + t * entry_dir = exit_point + s * exit_dir_back
            let det = entry_dir.0 * exit_dir_back.1 - entry_dir.1 * exit_dir_back.0;

            if det.abs() > 0.01 {
                // Lines intersect (not parallel)
                let dx = exit_point.0 - entry_point.0;
                let dy = exit_point.1 - entry_point.1;
                let t = (dx * exit_dir_back.1 - dy * exit_dir_back.0) / det;

                // Check if intersection is in forward direction (t > 0)
                if t > 0.0 {
                    // Use quadratic curve with intersection as control point
                    let control_point = (
                        entry_point.0 + t * entry_dir.0,
                        entry_point.1 + t * entry_dir.1
                    );
                    ctx.quadratic_curve_to(control_point.0, control_point.1, exit_point.0, exit_point.1);
                } else {
                    // Intersection behind us, use straight line
                    ctx.line_to(exit_point.0, exit_point.1);
                }
            } else {
                // Lines are parallel, use S-curve (bezier with two control points)
                let control_dist = 15.0;
                let cp1 = (
                    entry_point.0 + entry_dir.0 * control_dist,
                    entry_point.1 + entry_dir.1 * control_dist
                );
                let cp2 = (
                    exit_point.0 + exit_dir_back.0 * control_dist,
                    exit_point.1 + exit_dir_back.1 * control_dist
                );
                ctx.bezier_curve_to(cp1.0, cp1.1, cp2.0, cp2.1, exit_point.0, exit_point.1);
            }

            ctx.stroke();
        }
    }
}

/// Draw a line segment with optional avoidance transitions
fn draw_line_segment_with_avoidance(
    ctx: &CanvasRenderingContext2d,
    pos1: (f64, f64),
    pos2: (f64, f64),
    segment_length: f64,
    line_offset: (f64, f64),
    avoidance_offset: (f64, f64),
    transitions: (bool, bool),
) {
    let (ox, oy) = line_offset;
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

#[allow(clippy::cast_precision_loss)]
pub fn draw_lines(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    lines: &[Line],
    zoom: f64,
    cached_avoidance: &HashMap<EdgeIndex, (f64, f64)>,
    viewport_bounds: (f64, f64, f64, f64),
    junctions: &HashSet<NodeIndex>,
    _theme: Theme,
) {
    let (left, top, right, bottom) = viewport_bounds;
    let margin = 200.0; // Buffer to include lines slightly outside viewport

    // Group lines by edge for offset calculation
    let mut edge_to_lines: HashMap<EdgeIndex, Vec<&Line>> = HashMap::new();

    // Group lines by junction connections for drawing through junctions
    let mut junction_connections: HashMap<JunctionConnectionKey, Vec<&Line>> = HashMap::new();

    // Filter visible lines and build mappings
    for line in lines {
        if !line.visible {
            continue;
        }

        for segment in &line.forward_route {
            let edge_idx = EdgeIndex::new(segment.edge_index);
            edge_to_lines.entry(edge_idx).or_default().push(line);
        }

        // Build junction connection map
        for i in 0..line.forward_route.len().saturating_sub(1) {
            let from_segment = &line.forward_route[i];
            let to_segment = &line.forward_route[i + 1];

            let from_edge_idx = EdgeIndex::new(from_segment.edge_index);
            let to_edge_idx = EdgeIndex::new(to_segment.edge_index);

            // Find the shared node between these edges
            let (Some((from_src, from_tgt)), Some((to_src, to_tgt))) = (
                graph.graph.edge_endpoints(from_edge_idx),
                graph.graph.edge_endpoints(to_edge_idx)
            ) else {
                continue;
            };

            let shared_node = [
                (from_tgt == to_src || from_tgt == to_tgt, from_tgt),
                (from_src == to_src || from_src == to_tgt, from_src),
            ]
            .into_iter()
            .find_map(|(is_shared, node)| is_shared.then_some(node));

            // If shared node is a junction, record this connection
            if let Some(junction) = shared_node.filter(|n| junctions.contains(n)) {
                let key = JunctionConnectionKey {
                    junction,
                    from_edge: from_edge_idx,
                    to_edge: to_edge_idx,
                };
                junction_connections.entry(key).or_default().push(line);
            }
        }
    }

    // Draw each edge's lines
    for (edge_idx, edge_lines) in &edge_to_lines {
        // Get edge endpoints
        let Some((source, target)) = graph.graph.edge_endpoints(*edge_idx) else {
            continue;
        };

        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        // Viewport culling: skip lines completely outside visible area
        let min_x = pos1.0.min(pos2.0);
        let max_x = pos1.0.max(pos2.0);
        let min_y = pos1.1.min(pos2.1);
        let max_y = pos1.1.max(pos2.1);

        if max_x < left - margin || min_x > right + margin ||
           max_y < top - margin || min_y > bottom + margin {
            continue;
        }

        // Check if source or target is a junction
        let source_is_junction = junctions.contains(&source);
        let target_is_junction = junctions.contains(&target);

        // Use cached avoidance offset
        let (avoid_x, avoid_y) = cached_avoidance.get(edge_idx).copied().unwrap_or((0.0, 0.0));
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

        // Calculate perpendicular offset for parallel lines
        let nx = -dy / len;
        let ny = dx / len;

        // Sort lines by ID for consistent ordering
        let mut sorted_lines = edge_lines.clone();
        sorted_lines.sort_by_key(|line| line.id);

        let line_count = sorted_lines.len();

        if line_count == 1 {
            // Single line - draw in center
            let line = sorted_lines[0];
            let line_width = (LINE_BASE_WIDTH + line.thickness) / zoom;
            ctx.set_line_width(line_width);
            ctx.set_stroke_style_str(&line.color);
            ctx.begin_path();

            if needs_avoidance {
                // Draw segmented path: start -> offset section -> end
                let segment_length = ((actual_pos2.0 - actual_pos1.0).powi(2) + (actual_pos2.1 - actual_pos1.1).powi(2)).sqrt();

                // Check if we're connecting to junctions
                let start_needs_transition = !source_is_junction;
                let end_needs_transition = !target_is_junction;

                draw_line_segment_with_avoidance(
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
            // Multiple lines - position them adjacent with no gaps
            // When canvas is scaled and we set line_width = w / zoom,
            // the actual width in world coordinates is w / zoom (not w).
            // So we must calculate positions using the zoom-adjusted widths.

            let line_widths_world: Vec<f64> = sorted_lines.iter()
                .map(|line| (LINE_BASE_WIDTH + line.thickness) / zoom)
                .collect();

            // Gap should be equal to width of a standard line (BASE + default thickness of 2.0)
            let gap_width = (LINE_BASE_WIDTH + 2.0) / zoom;
            let num_gaps = sorted_lines.len().saturating_sub(1);
            let total_width: f64 = line_widths_world.iter().sum::<f64>()
                + (num_gaps as f64) * gap_width;
            let start_offset = -total_width / 2.0;

            let mut current_offset = start_offset;

            for (i, line) in sorted_lines.iter().enumerate() {
                let line_world_width = line_widths_world[i];

                // Position line center at current_offset + half of its width
                let offset = current_offset + line_world_width / 2.0;
                let ox = nx * offset;
                let oy = ny * offset;

                // Set line width (already calculated with zoom adjustment)
                ctx.set_line_width(line_world_width);
                ctx.set_stroke_style_str(&line.color);
                ctx.begin_path();

                // Move to next line position (with gap)
                current_offset += line_world_width + gap_width;

                if needs_avoidance {
                    // Draw segmented path with offset
                    let segment_length = ((actual_pos2.0 - actual_pos1.0).powi(2) + (actual_pos2.1 - actual_pos1.1).powi(2)).sqrt();

                    // Check if we're connecting to junctions
                    let start_needs_transition = !source_is_junction;
                    let end_needs_transition = !target_is_junction;

                    draw_line_segment_with_avoidance(
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

    // Draw junction connections
    draw_junction_connections(ctx, graph, &junction_connections, &edge_to_lines, zoom);
}
