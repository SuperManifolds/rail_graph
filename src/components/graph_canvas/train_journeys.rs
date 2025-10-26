use web_sys::CanvasRenderingContext2d;
use crate::models::Node;
use crate::train_journey::TrainJourney;
use crate::constants::BASE_MIDNIGHT;
use super::types::GraphDimensions;
use petgraph::stable_graph::NodeIndex;

// Train journey constants
const MIDNIGHT_WRAP_THRESHOLD: f64 = 0.5;
const HOVER_DISTANCE_THRESHOLD: f64 = 10.0; // pixels
const DOT_RADIUS_MULTIPLIER: f64 = 1.5; // Scale dots relative to line thickness
const MIN_DOT_RADIUS: f64 = 2.0; // Minimum dot radius in pixels
const TOTAL_HOURS: f64 = 48.0; // Total hours displayed on the graph

/// Assign view positions for a matched edge based on direction
fn assign_edge_positions(
    result: &mut [Option<usize>],
    seg_idx: usize,
    view_pos: usize,
    journey_start_node: NodeIndex,
    view_edge_start_node: NodeIndex,
) {
    // Bounds check: ensure we have space for both endpoints
    if seg_idx + 1 >= result.len() {
        return;
    }

    let going_forward = journey_start_node == view_edge_start_node;

    if going_forward {
        result[seg_idx] = Some(view_pos);
        result[seg_idx + 1] = Some(view_pos + 1);
    } else {
        // Going backward along this edge
        result[seg_idx] = Some(view_pos + 1);
        result[seg_idx + 1] = Some(view_pos);
    }
}

/// Match journey stations to view positions using edge-based matching
/// This correctly handles duplicate nodes by matching the actual edges traversed
/// Supports bidirectional search for return journeys
#[must_use]
pub fn match_journey_stations_to_view_by_edges(
    journey_segments: &[crate::train_journey::JourneySegment],
    journey_stations: &[(NodeIndex, chrono::NaiveDateTime, chrono::NaiveDateTime)],
    view_edge_path: &[usize],
    view_nodes: &[(NodeIndex, Node)],
) -> Vec<Option<usize>> {
    let mut result = vec![None; journey_stations.len()];
    let mut last_view_pos: Option<usize> = None;

    // Match each journey segment to view edge path
    for (seg_idx, segment) in journey_segments.iter().enumerate() {
        // Skip if this segment doesn't have a corresponding station
        // (This can happen if the journey has invalid nodes in the route)
        if seg_idx >= journey_stations.len() {
            continue;
        }

        let journey_edge = segment.edge_index;

        // Search bidirectionally from last position
        let start_pos = last_view_pos.unwrap_or(0);
        let mut matched = false;

        // First try forward from start_pos
        for view_pos in start_pos..view_edge_path.len() {
            if view_edge_path[view_pos] == journey_edge {
                let journey_start_node = journey_stations[seg_idx].0;
                let view_edge_start_node = view_nodes[view_pos].0;

                assign_edge_positions(&mut result, seg_idx, view_pos, journey_start_node, view_edge_start_node);
                last_view_pos = Some(view_pos);
                matched = true;
                break;
            }
        }

        // If not found forward, try backward from start_pos
        if !matched && start_pos > 0 {
            for view_pos in (0..start_pos).rev() {
                if view_edge_path[view_pos] == journey_edge {
                    let journey_start_node = journey_stations[seg_idx].0;
                    let view_edge_start_node = view_nodes[view_pos].0;

                    assign_edge_positions(&mut result, seg_idx, view_pos, journey_start_node, view_edge_start_node);
                    last_view_pos = Some(view_pos);
                    break;
                }
            }
        }
    }

    result
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    nodes: &[(NodeIndex, Node)],
    station_y_positions: &[f64],
    train_journeys: &[&TrainJourney],
    view_edge_path: &[usize],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    // Draw lines for each journey
    for journey in train_journeys {
        if journey.station_times.is_empty() {
            continue;
        }

        // Match journey stations to view positions using edge-based matching (handles duplicate nodes correctly)
        let station_positions = match_journey_stations_to_view_by_edges(
            &journey.segments,
            &journey.station_times,
            view_edge_path,
            nodes,
        );

        ctx.set_stroke_style_str(&journey.color);
        ctx.set_line_width(journey.thickness / zoom_level);
        ctx.begin_path();

        let mut last_visible_point: Option<(f64, f64, usize)> = None; // (x, y, view_position)

        for (i, (_node_idx, arrival_time, departure_time)) in journey.station_times.iter().enumerate() {
            // Look up the display position for this station
            let station_idx = station_positions.get(i).and_then(|&opt| opt);

            let arrival_fraction = time_to_fraction(*arrival_time);
            let arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);

            let departure_fraction = time_to_fraction(*departure_time);
            let departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

            // Only draw if this node is visible
            let Some(idx) = station_idx else {
                // Node is not visible - break the line
                last_visible_point = None;
                continue;
            };

            // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
            let y = station_y_positions[idx] - super::canvas::TOP_MARGIN;

            // Check if this arrival is before the week start (day -1 Sunday)
            let arrival_before_week_start = *arrival_time < BASE_MIDNIGHT;

            // Draw segment from previous point if applicable
            // Skip drawing if the arrival is before the week start
            if last_visible_point.is_some() && !arrival_before_week_start {
                // Draw diagonal segment from last visible point to this arrival
                // Consecutive journey stations always have a railway connection
                ctx.line_to(arrival_x, y);
            } else if !arrival_before_week_start {
                // First visible point in the current week - start the path
                ctx.move_to(arrival_x, y);
            }

            // Draw horizontal segment if there's wait time and this is a station (not a junction)
            // Skip drawing wait segments that are entirely before the week start (day -1 Sunday)
            let is_junction = matches!(nodes.get(idx).map(|(_, node)| node), Some(Node::Junction(_)));
            let has_wait_time = !is_junction && departure_x - arrival_x > f64::EPSILON;
            let wait_before_week_start = *departure_time < BASE_MIDNIGHT;

            if has_wait_time && !wait_before_week_start {
                ctx.line_to(departure_x, y);
            }

            // Update last visible point to the actual position we drew to
            // If we skipped the wait segment, use arrival_x instead of departure_x
            // Only update if arrival is in the current week
            if !arrival_before_week_start {
                let last_x = if has_wait_time && !wait_before_week_start { departure_x } else { arrival_x };
                last_visible_point = Some((last_x, y, idx));
            } else if *departure_time >= BASE_MIDNIGHT {
                // Arrival was before week start but departure is in current week
                // Start the line at the departure point for next segment
                last_visible_point = Some((departure_x, y, idx));
            }
        }

        ctx.stroke();
    }

    // Draw dots for each journey
    for journey in train_journeys {
        if journey.station_times.is_empty() {
            continue;
        }

        // Match journey stations to view positions using edge-based matching (handles duplicate nodes correctly)
        let station_positions = match_journey_stations_to_view_by_edges(
            &journey.segments,
            &journey.station_times,
            view_edge_path,
            nodes,
        );

        ctx.set_fill_style_str(&journey.color);
        let dot_radius = (journey.thickness * DOT_RADIUS_MULTIPLIER).max(MIN_DOT_RADIUS);
        ctx.begin_path();

        // Collect visible node info with original node_idx
        let visible_nodes: Vec<_> = journey.station_times.iter()
            .enumerate()
            .filter_map(|(i, (node_idx, arrival_time, departure_time))| {
                let idx = station_positions.get(i).and_then(|&opt| opt)?;

                let arrival_fraction = time_to_fraction(*arrival_time);
                let departure_fraction = time_to_fraction(*departure_time);
                let arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);
                let departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

                Some((*node_idx, idx, arrival_x, departure_x))
            })
            .collect();

        // Get journey station times for checking if dots are before week start
        for &(node_idx, idx, arrival_x, departure_x) in &visible_nodes {
            // Find the corresponding station_times entry
            // visible_nodes is a filtered version of station_times, need to find the original index
            let station_time = journey.station_times.iter()
                .enumerate()
                .find(|(_, (n_idx, _, _))| *n_idx == node_idx)
                .map(|(_, (_, arrival, departure))| (arrival, departure));

            let Some((arrival_time, departure_time)) = station_time else {
                continue;
            };

            // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
            let y = station_y_positions[idx] - super::canvas::TOP_MARGIN;

            // Check if this is a station (not junction) with wait time
            let is_junction = matches!(nodes.get(idx).map(|(_, node)| node), Some(Node::Junction(_)));
            let has_wait_time = !is_junction && departure_x - arrival_x > f64::EPSILON;
            let wait_before_week_start = *departure_time < BASE_MIDNIGHT;

            // Check if this is the actual start or end of the entire journey using stored route endpoints
            let is_route_start = Some(node_idx) == journey.route_start_node;
            let is_route_end = Some(node_idx) == journey.route_end_node;

            // Draw dots if: has wait time (and not before week start), OR (is actual start/end of route AND not a junction AND not before week start)
            let should_draw_endpoint = (is_route_start || is_route_end) && !is_junction && *arrival_time >= BASE_MIDNIGHT;
            let should_draw_wait_dots = has_wait_time && !wait_before_week_start;

            if !should_draw_wait_dots && !should_draw_endpoint {
                continue;
            }

            // Add arrival dot to path (move_to starts a new subpath)
            ctx.move_to(arrival_x + dot_radius / zoom_level, y);
            let _ = ctx.arc(arrival_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);

            // Add departure dot if different from arrival and this is a station (not a junction)
            if should_draw_wait_dots {
                ctx.move_to(departure_x + dot_radius / zoom_level, y);
                let _ = ctx.arc(departure_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);
            }
        }

        ctx.fill();
    }
}

#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
#[must_use]
pub fn check_journey_hover(
    mouse_x: f64,
    mouse_y: f64,
    train_journeys: &[&TrainJourney],
    nodes: &[(NodeIndex, Node)],
    station_y_positions: &[f64],
    view_edge_path: &[usize],
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

    train_journeys
        .iter()
        .find_map(|journey| {
            check_single_journey_hover(
                mouse_x,
                mouse_y,
                journey,
                nodes,
                graph_width,
                station_y_positions,
                view_edge_path,
                viewport,
            )
        })
}

#[allow(clippy::cast_precision_loss)]
fn check_single_journey_hover(
    mouse_x: f64,
    mouse_y: f64,
    journey: &TrainJourney,
    nodes: &[(NodeIndex, Node)],
    graph_width: f64,
    station_y_positions: &[f64],
    view_edge_path: &[usize],
    viewport: &super::types::ViewportState,
) -> Option<uuid::Uuid> {
    use super::canvas::{LEFT_MARGIN, TOP_MARGIN};
    use crate::time::time_to_fraction;

    // Match journey stations to view positions using edge-based matching
    let station_positions = match_journey_stations_to_view_by_edges(
        &journey.segments,
        &journey.station_times,
        view_edge_path,
        nodes,
    );

    let mut prev_departure_point: Option<(f64, f64)> = None;

    let hour_width = graph_width / TOTAL_HOURS;
    let mut first_point = true;
    let mut prev_x = 0.0;

    for (i, (_node_idx, arrival_time, departure_time)) in journey.station_times.iter().enumerate() {
        // Look up the display position for this station
        let station_idx = station_positions.get(i).and_then(|&opt| opt);

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
        if let Some(idx) = station_idx {
            let y_in_zoomed = station_y_positions[idx] - TOP_MARGIN;

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
            let has_wait_time = departure_screen_x - arrival_screen_x > f64::EPSILON;

            if has_wait_time {
                let distance = point_to_line_distance(mouse_x, mouse_y, arrival_screen_x, screen_y, departure_screen_x, screen_y);
                if distance < HOVER_DISTANCE_THRESHOLD {
                    return Some(journey.id);
                }
            }

            // Update prev point to the actual position we drew to
            let last_x = if has_wait_time { departure_screen_x } else { arrival_screen_x };
            prev_departure_point = Some((last_x, screen_y));
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