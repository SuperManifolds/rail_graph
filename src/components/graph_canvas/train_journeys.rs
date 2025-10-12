use web_sys::CanvasRenderingContext2d;
use crate::models::StationNode;
use crate::train_journey::TrainJourney;
use super::types::GraphDimensions;

// Train journey constants
const MIDNIGHT_WRAP_THRESHOLD: f64 = 0.5;
const HOVER_DISTANCE_THRESHOLD: f64 = 10.0; // pixels
const DOT_RADIUS_MULTIPLIER: f64 = 1.5; // Scale dots relative to line thickness
const MIN_DOT_RADIUS: f64 = 2.0; // Minimum dot radius in pixels
const TOTAL_HOURS: f64 = 48.0; // Total hours displayed on the graph

#[allow(clippy::cast_precision_loss)]
pub fn draw_train_journeys(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[StationNode],
    train_journeys: &[&TrainJourney],
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    use std::collections::HashMap;

    let station_height = dims.graph_height / stations.len() as f64;

    // Group journeys by color and thickness for batching
    let mut journeys_by_style: HashMap<(String, u32), Vec<&TrainJourney>> = HashMap::new();
    for journey in train_journeys {
        if !journey.station_indices.is_empty() {
            // Convert thickness to integer microns to use as key
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let thickness_key = (journey.thickness * 1000.0) as u32;
            journeys_by_style
                .entry((journey.color.clone(), thickness_key))
                .or_default()
                .push(journey);
        }
    }

    // Draw all journeys grouped by style
    for ((color, thickness_key), journeys) in &journeys_by_style {
        let thickness = f64::from(*thickness_key) / 1000.0;

        ctx.set_stroke_style_str(color);
        ctx.set_line_width(thickness / zoom_level);
        ctx.begin_path();

        // Draw all line paths for this style
        for journey in journeys {
            let mut first_point = true;
            let mut prev_x = 0.0;

            for (i, (_station_name, arrival_time, departure_time)) in journey.station_times.iter().enumerate() {
                let station_idx = journey.station_indices[i];
                let arrival_fraction = time_to_fraction(*arrival_time);
                let departure_fraction = time_to_fraction(*departure_time);
                let mut arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);
                let mut departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

                if !first_point && arrival_x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                    arrival_x += dims.graph_width;
                    departure_x += dims.graph_width;
                }
                let y = dims.top_margin
                    + (station_idx as f64 * station_height)
                    + (station_height / 2.0);

                if first_point {
                    ctx.move_to(arrival_x, y);
                    first_point = false;
                } else {
                    ctx.line_to(arrival_x, y);
                }

                if (arrival_x - departure_x).abs() > f64::EPSILON {
                    ctx.line_to(departure_x, y);
                }

                prev_x = departure_x;
            }
        }

        // Stroke all lines for this style at once
        ctx.stroke();

        // Draw dots in batches
        ctx.set_fill_style_str(color);
        let dot_radius = (thickness * DOT_RADIUS_MULTIPLIER).max(MIN_DOT_RADIUS);

        for journey in journeys {
            let mut prev_x = 0.0;
            for (i, (_station_name, arrival_time, departure_time)) in journey.station_times.iter().enumerate() {
                let station_idx = journey.station_indices[i];
                let arrival_fraction = time_to_fraction(*arrival_time);
                let departure_fraction = time_to_fraction(*departure_time);
                let mut arrival_x = dims.left_margin + (arrival_fraction * dims.hour_width);
                let mut departure_x = dims.left_margin + (departure_fraction * dims.hour_width);

                if prev_x > 0.0 && arrival_x < prev_x - dims.graph_width * MIDNIGHT_WRAP_THRESHOLD {
                    arrival_x += dims.graph_width;
                    departure_x += dims.graph_width;
                }

                let y = dims.top_margin
                    + (station_idx as f64 * station_height)
                    + (station_height / 2.0);

                ctx.begin_path();
                let _ = ctx.arc(arrival_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                ctx.fill();

                if (arrival_x - departure_x).abs() > f64::EPSILON {
                    ctx.begin_path();
                    let _ = ctx.arc(departure_x, y, dot_radius / zoom_level, 0.0, std::f64::consts::PI * 2.0);
                    ctx.fill();
                }

                prev_x = departure_x;
            }
        }
    }
}

#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn check_journey_hover(
    mouse_x: f64,
    mouse_y: f64,
    train_journeys: &[&TrainJourney],
    stations: &[StationNode],
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

    let station_height = graph_height / stations.len() as f64;

    train_journeys
        .iter()
        .find_map(|journey| {
            check_single_journey_hover(
                mouse_x,
                mouse_y,
                journey,
                stations,
                graph_width,
                station_height,
                viewport,
            )
        })
}

#[allow(clippy::cast_precision_loss)]
fn check_single_journey_hover(
    mouse_x: f64,
    mouse_y: f64,
    journey: &TrainJourney,
    stations: &[StationNode],
    graph_width: f64,
    station_height: f64,
    viewport: &super::types::ViewportState,
) -> Option<uuid::Uuid> {
    use super::canvas::{LEFT_MARGIN, TOP_MARGIN};
    use crate::time::time_to_fraction;

    let mut prev_departure_point: Option<(f64, f64)> = None;

    let hour_width = graph_width / TOTAL_HOURS;
    let mut first_point = true;
    let mut prev_x = 0.0;

    for (station_name, arrival_time, departure_time) in &journey.station_times {
        let Some(station_idx) = stations.iter().position(|s| s.name == *station_name) else {
            continue;
        };

        let arrival_fraction = time_to_fraction(*arrival_time);
        let departure_fraction = time_to_fraction(*departure_time);
        let mut arrival_x_zoomed = arrival_fraction * hour_width;
        let mut departure_x_zoomed = departure_fraction * hour_width;

        // Handle midnight wrap (same logic as drawing)
        if !first_point && arrival_x_zoomed < prev_x - graph_width * MIDNIGHT_WRAP_THRESHOLD {
            arrival_x_zoomed += graph_width;
            departure_x_zoomed += graph_width;
        }

        let y_in_zoomed = (station_idx as f64 * station_height) + (station_height / 2.0);

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
        prev_x = departure_x_zoomed;
        first_point = false;
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