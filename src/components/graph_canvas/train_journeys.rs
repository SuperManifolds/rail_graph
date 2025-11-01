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
const CONTINUATION_ARROW_LENGTH: f64 = 12.0; // Length of continuation arrow
const CONTINUATION_ARROW_HEAD_SIZE: f64 = 6.0; // Size of arrow head
const NON_EDITED_JOURNEY_OPACITY: f64 = 0.5; // Opacity for journeys when line editor is open

/// Update search direction based on position change
fn update_search_direction(
    search_direction_is_forward: &mut Option<bool>,
    last_view_pos: Option<usize>,
    current_view_pos: usize,
) {
    if let Some(last_pos) = last_view_pos {
        *search_direction_is_forward = Some(current_view_pos > last_pos);
    }
}

/// Assign view positions for a matched edge based on direction
/// Does not overwrite positions that have already been set (important for duplicate nodes)
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
        // Only set if not already set (avoid overwriting in case of duplicate nodes)
        if result[seg_idx].is_none() {
            result[seg_idx] = Some(view_pos);
        }
        if result[seg_idx + 1].is_none() {
            result[seg_idx + 1] = Some(view_pos + 1);
        }
    } else {
        // Going backward along this edge
        if result[seg_idx].is_none() {
            result[seg_idx] = Some(view_pos + 1);
        }
        if result[seg_idx + 1].is_none() {
            result[seg_idx + 1] = Some(view_pos);
        }
    }
}

/// Verify that an edge at `view_pos` connects to an existing position
/// Returns true if the edge is valid for matching (either no existing position, or connects correctly)
fn verify_edge_connectivity(
    result: &[Option<usize>],
    seg_idx: usize,
    view_pos: usize,
    journey_start_node: NodeIndex,
    view_edge_start_node: NodeIndex,
    view_nodes_len: usize,
) -> bool {
    if let Some(existing_pos) = result[seg_idx] {
        let going_forward = journey_start_node == view_edge_start_node;
        let edge_start_pos = if going_forward {
            view_pos
        } else if view_pos + 1 < view_nodes_len {
            view_pos + 1
        } else {
            return false; // Invalid position
        };

        edge_start_pos == existing_pos
    } else {
        true // No existing position, so any edge is valid
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
    let mut search_direction_is_forward: Option<bool> = None;

    // Match each journey segment to view edge path
    for (seg_idx, segment) in journey_segments.iter().enumerate() {
        // Skip if this segment doesn't have a corresponding station
        // (This can happen if the journey has invalid nodes in the route)
        if seg_idx >= journey_stations.len() {
            continue;
        }

        let journey_edge = segment.edge_index;
        let start_pos = last_view_pos.unwrap_or(0);
        let mut matched = false;

        // Determine search direction based on recent matches
        // If we don't know the direction yet, try forward first (default behavior)
        let try_forward_first = search_direction_is_forward.unwrap_or(true);

        let search_ranges: [(std::ops::RangeInclusive<usize>, bool); 2] = if try_forward_first {
            [(start_pos..=view_edge_path.len().saturating_sub(1), true), (0..=start_pos.saturating_sub(1), false)]
        } else {
            [(0..=start_pos.saturating_sub(1), false), (start_pos..=view_edge_path.len().saturating_sub(1), true)]
        };

        for (range, is_forward_search) in search_ranges {
            let positions: Vec<usize> = if is_forward_search {
                range.collect()
            } else {
                range.rev().collect()
            };

            for view_pos in positions {
                if view_edge_path[view_pos] != journey_edge {
                    continue;
                }

                let journey_start_node = journey_stations[seg_idx].0;
                let view_edge_start_node = view_nodes[view_pos].0;

                // If this journey station already has an assigned position (from previous segment),
                // verify that this edge connects to it. This handles backtracking through duplicate nodes.
                if !verify_edge_connectivity(
                    &result,
                    seg_idx,
                    view_pos,
                    journey_start_node,
                    view_edge_start_node,
                    view_nodes.len(),
                ) {
                    continue;
                }

                assign_edge_positions(&mut result, seg_idx, view_pos, journey_start_node, view_edge_start_node);

                // Update search direction based on position change
                update_search_direction(&mut search_direction_is_forward, last_view_pos, view_pos);
                last_view_pos = Some(view_pos);
                matched = true;
                break;
            }

            if matched {
                break;
            }
        }
    }

    result
}

/// Draw an arrow indicator showing that a journey continues beyond the visible view
/// Always draws a right-pointing arrow (→) to indicate the direction of travel
fn draw_continuation_indicator(
    ctx: &CanvasRenderingContext2d,
    x: f64,
    y: f64,
    color: &str,
    line_width: f64,
    zoom_level: f64,
) {
    let arrow_length = CONTINUATION_ARROW_LENGTH / zoom_level;
    let head_size = CONTINUATION_ARROW_HEAD_SIZE / zoom_level;

    ctx.save();
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(line_width);
    ctx.set_line_cap("round");
    ctx.set_line_join("round");
    ctx.begin_path();

    // Draw → arrow (direction of travel)
    // Horizontal line going right
    ctx.move_to(x, y);
    ctx.line_to(x + arrow_length, y);

    // Arrow head at right
    ctx.move_to(x + arrow_length - head_size / 2.0, y - head_size / 2.0);
    ctx.line_to(x + arrow_length, y);
    ctx.line_to(x + arrow_length - head_size / 2.0, y + head_size / 2.0);

    ctx.stroke();
    ctx.restore();
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines, clippy::cast_possible_truncation, clippy::too_many_arguments)]
pub fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    nodes: &[(NodeIndex, Node)],
    station_y_positions: &[f64],
    train_journeys: &[&TrainJourney],
    view_edge_path: &[usize],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    edited_line_ids: &std::collections::HashSet<uuid::Uuid>,
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

        // Apply dimming to journeys not in edited lines
        let should_dim = !edited_line_ids.is_empty() && !edited_line_ids.contains(&journey.line_id);
        let color = if should_dim {
            super::types::hex_to_rgba(&journey.color, NON_EDITED_JOURNEY_OPACITY)
        } else {
            journey.color.clone()
        };

        ctx.set_stroke_style_str(&color);
        ctx.set_line_width(journey.thickness / zoom_level);
        ctx.begin_path();

        let mut last_visible_point: Option<(f64, f64, usize)> = None; // (x, y, view_position)
        let mut first_visible_point: Option<(f64, f64)> = None; // (x, y) of first visible station
        let mut first_visible_node: Option<NodeIndex> = None;
        let mut last_visible_node: Option<NodeIndex> = None;

        for (i, (node_idx, arrival_time, departure_time)) in journey.station_times.iter().enumerate() {
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

            // Bounds check with logging
            if idx >= station_y_positions.len() {
                web_sys::console::warn_3(
                    &wasm_bindgen::JsValue::from_str("Station index out of bounds for train"),
                    &wasm_bindgen::JsValue::from_str(&journey.train_number),
                    &wasm_bindgen::JsValue::from_str(&format!("idx: {}, len: {}, station: {}", idx, station_y_positions.len(), i))
                );
                continue;
            }

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

                // Track first visible point
                if first_visible_point.is_none() {
                    first_visible_point = Some((arrival_x, y));
                    first_visible_node = Some(*node_idx);
                }

                // Always update last visible node
                last_visible_node = Some(*node_idx);
            } else if *departure_time >= BASE_MIDNIGHT {
                // Arrival was before week start but departure is in current week
                // Start the line at the departure point for next segment
                last_visible_point = Some((departure_x, y, idx));

                if first_visible_point.is_none() {
                    first_visible_point = Some((departure_x, y));
                    first_visible_node = Some(*node_idx);
                }

                last_visible_node = Some(*node_idx);
            }
        }

        ctx.stroke();

        // Draw continuation indicators if journey extends beyond visible area
        if let Some((first_x, first_y)) = first_visible_point {
            // Check if first visible node is NOT the actual route start
            if first_visible_node != journey.route_start_node {
                draw_continuation_indicator(
                    ctx,
                    first_x,
                    first_y,
                    &journey.color,
                    journey.thickness / zoom_level,
                    zoom_level,
                );
            }
        }

        if let Some((last_x, last_y, _)) = last_visible_point {
            // Check if last visible node is NOT the actual route end
            if last_visible_node != journey.route_end_node {
                draw_continuation_indicator(
                    ctx,
                    last_x,
                    last_y,
                    &journey.color,
                    journey.thickness / zoom_level,
                    zoom_level,
                );
            }
        }
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

        // Apply dimming to journeys not in edited lines
        let should_dim = !edited_line_ids.is_empty() && !edited_line_ids.contains(&journey.line_id);
        let color = if should_dim {
            super::types::hex_to_rgba(&journey.color, NON_EDITED_JOURNEY_OPACITY)
        } else {
            journey.color.clone()
        };

        ctx.set_fill_style_str(&color);
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
    dims: &super::types::GraphDimensions,
    viewport: &super::types::ViewportState,
) -> Option<uuid::Uuid> {
    // Check if mouse is within the graph area
    if mouse_x < dims.left_margin || mouse_x > dims.left_margin + dims.graph_width
        || mouse_y < dims.top_margin || mouse_y > dims.top_margin + dims.graph_height {
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
                dims,
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
    dims: &super::types::GraphDimensions,
    station_y_positions: &[f64],
    view_edge_path: &[usize],
    viewport: &super::types::ViewportState,
) -> Option<uuid::Uuid> {
    use crate::time::time_to_fraction;

    // Match journey stations to view positions using edge-based matching
    let station_positions = match_journey_stations_to_view_by_edges(
        &journey.segments,
        &journey.station_times,
        view_edge_path,
        nodes,
    );

    let mut prev_departure_point: Option<(f64, f64)> = None;

    let hour_width = dims.graph_width / TOTAL_HOURS;
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
        if !first_point && arrival_x_zoomed < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
            arrival_x_zoomed += dims.graph_width;
            departure_x_zoomed += dims.graph_width;
        }

        // Only process hover detection for visible stations
        if let Some(idx) = station_idx {
            let y_in_zoomed = station_y_positions[idx] - dims.top_margin;

            // Transform to screen coordinates
            let arrival_screen_x = dims.left_margin + (arrival_x_zoomed * viewport.zoom_level * viewport.zoom_level_x) + viewport.pan_offset_x;
            let departure_screen_x = dims.left_margin + (departure_x_zoomed * viewport.zoom_level * viewport.zoom_level_x) + viewport.pan_offset_x;
            let screen_y = dims.top_margin + (y_in_zoomed * viewport.zoom_level) + viewport.pan_offset_y;

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