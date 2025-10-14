use web_sys::CanvasRenderingContext2d;
use crate::models::Node;
use crate::train_journey::TrainJourney;
use super::types::GraphDimensions;
use std::collections::HashMap;
use petgraph::stable_graph::NodeIndex;

// Train journey constants
const MIDNIGHT_WRAP_THRESHOLD: f64 = 0.5;
const HOVER_DISTANCE_THRESHOLD: f64 = 10.0; // pixels
const DOT_RADIUS_MULTIPLIER: f64 = 1.5; // Scale dots relative to line thickness
const MIN_DOT_RADIUS: f64 = 2.0; // Minimum dot radius in pixels
const TOTAL_HOURS: f64 = 48.0; // Total hours displayed on the graph

/// Build a map from `NodeIndex` to display position (0, 1, 2, ...)
/// All nodes (stations and junctions) get sequential integer positions
#[allow(clippy::cast_precision_loss)]
fn build_node_position_map(
    nodes: &[(NodeIndex, Node)],
) -> HashMap<NodeIndex, usize> {
    nodes.iter()
        .enumerate()
        .map(|(idx, (node_idx, _))| (*node_idx, idx))
        .collect()
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    nodes: &[(NodeIndex, Node)],
    train_journeys: &[&TrainJourney],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    let node_height = dims.graph_height / nodes.len() as f64;
    let node_positions = build_node_position_map(nodes);

    // Draw lines for each journey
    for journey in train_journeys {
        if journey.station_times.is_empty() {
            continue;
        }

        ctx.set_stroke_style_str(&journey.color);
        ctx.set_line_width(journey.thickness / zoom_level);
        ctx.begin_path();

        let mut last_visible_point: Option<(f64, f64, usize)> = None; // (x, y, view_position)
        let mut prev_x = 0.0;

        for (node_idx, arrival_time, departure_time) in &journey.station_times {
            // Look up the display position for this station
            let station_idx = node_positions.get(node_idx);

            let arrival_fraction = time_to_fraction(*arrival_time);
            let departure_fraction = time_to_fraction(*departure_time);
            let mut arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);
            let mut departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

            if prev_x > 0.0 && arrival_x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                arrival_x += dims.graph_width;
                departure_x += dims.graph_width;
            }

            // Only draw if this node is visible
            let Some(&idx) = station_idx else {
                // Node is not visible - break the line
                last_visible_point = None;
                prev_x = departure_x;
                continue;
            };

            let y = dims.top_margin
                + (idx as f64 * node_height)
                + (node_height / 2.0);

            // Draw segment from previous point if applicable
            if let Some(last_visible_idx) = last_visible_point.map(|(_, _, idx)| idx) {
                // Check if this node is consecutive with the previous node in the view (forward or backward)
                let is_consecutive = idx == last_visible_idx + 1 || idx + 1 == last_visible_idx;

                if is_consecutive {
                    // Draw normal diagonal segment from last visible point to this arrival
                    ctx.line_to(arrival_x, y);
                }
                // If not consecutive, don't draw a segment (gap with invisible nodes)
            } else {
                // First visible point - start the path
                ctx.move_to(arrival_x, y);
            }

            // Draw horizontal segment if there's wait time and this is a station (not a junction)
            let is_junction = matches!(nodes.get(idx).map(|(_, node)| node), Some(Node::Junction(_)));
            if !is_junction && (arrival_x - departure_x).abs() > f64::EPSILON {
                ctx.line_to(departure_x, y);
            }

            // Update last visible point to departure position
            last_visible_point = Some((departure_x, y, idx));
            prev_x = departure_x;
        }

        ctx.stroke();
    }

    // Draw dots for each journey
    for journey in train_journeys {
        if journey.station_times.is_empty() {
            continue;
        }

        ctx.set_fill_style_str(&journey.color);
        let dot_radius = (journey.thickness * DOT_RADIUS_MULTIPLIER).max(MIN_DOT_RADIUS);
        ctx.begin_path();

        // Collect visible node info first so we can look ahead
        let visible_nodes: Vec<_> = journey.station_times.iter()
            .filter_map(|(node_idx, arrival_time, departure_time)| {
                let idx = *node_positions.get(node_idx)?;

                let arrival_fraction = time_to_fraction(*arrival_time);
                let departure_fraction = time_to_fraction(*departure_time);
                let arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);
                let departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

                Some((idx, arrival_x, departure_x))
            })
            .collect();

        // Apply midnight wrapping
        let mut wrapped_nodes = Vec::with_capacity(visible_nodes.len());
        let mut prev_x = 0.0;
        for (idx, mut arrival_x, mut departure_x) in visible_nodes {
            if prev_x > 0.0 && arrival_x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                arrival_x += dims.graph_width;
                departure_x += dims.graph_width;
            }
            wrapped_nodes.push((idx, arrival_x, departure_x));
            prev_x = departure_x;
        }

        for (i, &(idx, arrival_x, departure_x)) in wrapped_nodes.iter().enumerate() {
            let y = dims.top_margin
                + (idx as f64 * node_height)
                + (node_height / 2.0);

            // Check if this node has a segment connecting to previous or next visible node
            let has_prev_segment = if i > 0 {
                let prev_idx = wrapped_nodes[i - 1].0;
                idx == prev_idx + 1 || idx + 1 == prev_idx
            } else {
                false
            };

            let has_next_segment = if i + 1 < wrapped_nodes.len() {
                let next_idx = wrapped_nodes[i + 1].0;
                idx == next_idx + 1 || idx + 1 == next_idx
            } else {
                false
            };

            let has_segment = has_prev_segment || has_next_segment;

            // Only draw dots if this node has at least one connecting segment
            // or if it's a station (not junction) with wait time (horizontal segment)
            let is_junction = matches!(nodes.get(idx).map(|(_, node)| node), Some(Node::Junction(_)));
            let has_wait_time = !is_junction && (arrival_x - departure_x).abs() > f64::EPSILON;

            if has_segment || has_wait_time {
                // Add arrival dot to path (move_to starts a new subpath)
                ctx.move_to(arrival_x + dot_radius / zoom_level, y);
                let _ = ctx.arc(arrival_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);

                // Add departure dot if different from arrival and this is a station (not a junction)
                if has_wait_time {
                    ctx.move_to(departure_x + dot_radius / zoom_level, y);
                    let _ = ctx.arc(departure_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                }
            }
        }

        ctx.fill();
    }
}

#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn check_journey_hover(
    mouse_x: f64,
    mouse_y: f64,
    train_journeys: &[&TrainJourney],
    nodes: &[(NodeIndex, Node)],
    canvas_width: f64,
    canvas_height: f64,
    viewport: &super::types::ViewportState,
) -> Option<uuid::Uuid> {
    use super::canvas::{LEFT_MARGIN, TOP_MARGIN, RIGHT_PADDING, BOTTOM_PADDING};

    let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;
    let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;

    // Check if mouse is within the graph area
    if mouse_x < LEFT_MARGIN || mouse_x > LEFT_MARGIN + graph_width
        || mouse_y < TOP_MARGIN || mouse_y > TOP_MARGIN + graph_height {
        return None;
    }

    let node_height = graph_height / nodes.len() as f64;
    let node_positions = build_node_position_map(nodes);

    train_journeys
        .iter()
        .find_map(|journey| {
            check_single_journey_hover(
                mouse_x,
                mouse_y,
                journey,
                graph_width,
                node_height,
                viewport,
                &node_positions,
            )
        })
}

#[allow(clippy::cast_precision_loss)]
fn check_single_journey_hover(
    mouse_x: f64,
    mouse_y: f64,
    journey: &TrainJourney,
    graph_width: f64,
    node_height: f64,
    viewport: &super::types::ViewportState,
    node_positions: &HashMap<NodeIndex, usize>,
) -> Option<uuid::Uuid> {
    use super::canvas::{LEFT_MARGIN, TOP_MARGIN};
    use crate::time::time_to_fraction;

    let mut prev_departure_point: Option<(f64, f64)> = None;

    let hour_width = graph_width / TOTAL_HOURS;
    let mut first_point = true;
    let mut prev_x = 0.0;

    for (node_idx, arrival_time, departure_time) in &journey.station_times {
        // Look up the display position for this station
        let station_idx = node_positions.get(node_idx);

        let arrival_fraction = time_to_fraction(*arrival_time);
        let departure_fraction = time_to_fraction(*departure_time);
        let mut arrival_x_zoomed = arrival_fraction * hour_width;
        let mut departure_x_zoomed = departure_fraction * hour_width;

        // Handle midnight wrap (same logic as drawing)
        if !first_point && arrival_x_zoomed < prev_x - graph_width * MIDNIGHT_WRAP_THRESHOLD {
            arrival_x_zoomed += graph_width;
            departure_x_zoomed += graph_width;
        }

        // Only process hover detection for visible stations
        if let Some(&idx) = station_idx {
            let y_in_zoomed = (idx as f64 * node_height) + (node_height / 2.0);

            // Transform to screen coordinates
            let arrival_screen_x = LEFT_MARGIN + (arrival_x_zoomed * viewport.zoom_level * viewport.zoom_level_x) + viewport.pan_offset_x;
            let departure_screen_x = LEFT_MARGIN + (departure_x_zoomed * viewport.zoom_level * viewport.zoom_level_x) + viewport.pan_offset_x;
            let screen_y = TOP_MARGIN + (y_in_zoomed * viewport.zoom_level) + viewport.pan_offset_y;

            // Check diagonal segment from previous departure to this arrival
            if let Some((prev_dep_x, prev_dep_y)) = prev_departure_point {
                let distance = point_to_line_distance(mouse_x, mouse_y, prev_dep_x, prev_dep_y, arrival_screen_x, screen_y);
                if distance < HOVER_DISTANCE_THRESHOLD {
                    return Some(journey.id);
                }
            }

            // Check horizontal segment from arrival to departure at this station
            if (arrival_screen_x - departure_screen_x).abs() > f64::EPSILON {
                let distance = point_to_line_distance(mouse_x, mouse_y, arrival_screen_x, screen_y, departure_screen_x, screen_y);
                if distance < HOVER_DISTANCE_THRESHOLD {
                    return Some(journey.id);
                }
            }

            prev_departure_point = Some((departure_screen_x, screen_y));
            first_point = false;
        }

        prev_x = departure_x_zoomed;
    }

    None
}

fn point_to_line_distance(px: f64, py: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length_squared = dx * dx + dy * dy;

    if length_squared == 0.0 {
        // Line segment is a point
        return ((px - x1) * (px - x1) + (py - y1) * (py - y1)).sqrt();
    }

    // Calculate projection parameter
    let t = ((px - x1) * dx + (py - y1) * dy) / length_squared;
    let t = t.clamp(0.0, 1.0);

    // Find closest point on line segment
    let closest_x = x1 + t * dx;
    let closest_y = y1 + t * dy;

    // Calculate distance
    ((px - closest_x) * (px - closest_x) + (py - closest_y) * (py - closest_y)).sqrt()
}