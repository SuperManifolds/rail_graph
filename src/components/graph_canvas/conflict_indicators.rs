use super::types::GraphDimensions;
use crate::models::StationNode;
use crate::conflict::{Conflict, StationCrossing};
use web_sys::CanvasRenderingContext2d;

// Conflict highlight constants
const CONFLICT_TRIANGLE_SIZE: f64 = 15.0;
const CONFLICT_LINE_WIDTH: f64 = 1.5;
const CONFLICT_FILL_COLOR: &str = "rgba(255, 200, 0, 0.9)";
const CONFLICT_STROKE_COLOR: &str = "rgba(0, 0, 0, 0.8)";
const CONFLICT_ICON_COLOR: &str = "#000";
const CONFLICT_ICON_FONT_SIZE: f64 = 12.0;
const CONFLICT_ICON_OFFSET_X: f64 = 2.0;
const CONFLICT_ICON_OFFSET_Y: f64 = 4.0;
const CONFLICT_LABEL_COLOR: &str = "rgba(255, 255, 255, 0.9)";
const CONFLICT_LABEL_FONT_SIZE: f64 = 9.0;
const CONFLICT_LABEL_OFFSET: f64 = 5.0;
const CONFLICT_WARNING_COLOR: &str = "rgba(255, 0, 0, 0.8)";
const CONFLICT_WARNING_FONT_SIZE: f64 = 14.0;
const MAX_CONFLICTS_DISPLAYED: usize = 9999;

// Station crossing constants
const CROSSING_FILL_COLOR: &str = "rgba(0, 200, 100, 0.3)";
const CROSSING_STROKE_COLOR: &str = "rgba(0, 150, 75, 0.6)";
const CROSSING_LINE_WIDTH: f64 = 2.0;

// Block visualization constants
const BLOCK_FILL_OPACITY: &str = "33"; // ~20% opacity in hex
const BLOCK_STROKE_OPACITY: &str = "99"; // ~60% opacity in hex
const BLOCK_BORDER_WIDTH: f64 = 1.0;

#[allow(clippy::cast_precision_loss)]
pub fn draw_conflict_highlights(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflicts: &[&Conflict],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    let size = CONFLICT_TRIANGLE_SIZE / zoom_level;

    // Batch all triangle fills
    ctx.set_fill_style_str(CONFLICT_FILL_COLOR);
    ctx.begin_path();

    for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
        let time_fraction = time_to_fraction(conflict.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);

        // Calculate y position based on the conflict position between stations
        let y = dims.top_margin
            + (conflict.station1_idx as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (conflict.station2_idx - conflict.station1_idx) as f64);

        // Add triangle to batch
        ctx.move_to(x, y - size);
        ctx.line_to(x - size * 0.866, y + size * 0.5);
        ctx.line_to(x + size * 0.866, y + size * 0.5);
        ctx.close_path();
    }

    // Fill all triangles at once
    ctx.fill();

    // Batch all triangle strokes
    ctx.set_stroke_style_str(CONFLICT_STROKE_COLOR);
    ctx.set_line_width(CONFLICT_LINE_WIDTH / zoom_level);
    ctx.begin_path();

    for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
        let time_fraction = time_to_fraction(conflict.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);

        let y = dims.top_margin
            + (conflict.station1_idx as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (conflict.station2_idx - conflict.station1_idx) as f64);

        // Add triangle to batch
        ctx.move_to(x, y - size);
        ctx.line_to(x - size * 0.866, y + size * 0.5);
        ctx.line_to(x + size * 0.866, y + size * 0.5);
        ctx.close_path();
    }

    // Stroke all triangles at once
    ctx.stroke();

    // Set font and color for exclamation marks once
    ctx.set_fill_style_str(CONFLICT_ICON_COLOR);
    ctx.set_font(&format!(
        "bold {}px sans-serif",
        CONFLICT_ICON_FONT_SIZE / zoom_level
    ));

    // Draw all exclamation marks
    for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
        let time_fraction = time_to_fraction(conflict.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);

        let y = dims.top_margin
            + (conflict.station1_idx as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (conflict.station2_idx - conflict.station1_idx) as f64);

        let _ = ctx.fill_text("!", x - CONFLICT_ICON_OFFSET_X / zoom_level, y + CONFLICT_ICON_OFFSET_Y / zoom_level);
    }

    // Set font and color for labels once
    ctx.set_fill_style_str(CONFLICT_LABEL_COLOR);
    ctx.set_font(&format!(
        "{}px monospace",
        CONFLICT_LABEL_FONT_SIZE / zoom_level
    ));

    // Draw all labels
    for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
        let time_fraction = time_to_fraction(conflict.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);

        let y = dims.top_margin
            + (conflict.station1_idx as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (conflict.station2_idx - conflict.station1_idx) as f64);

        let label = format!("{} × {}", conflict.journey1_id, conflict.journey2_id);
        let _ = ctx.fill_text(&label, x + size + CONFLICT_LABEL_OFFSET / zoom_level, y);
    }

    // If there are more conflicts than displayed, show a count
    if conflicts.len() > MAX_CONFLICTS_DISPLAYED {
        ctx.set_fill_style_str(CONFLICT_WARNING_COLOR);
        ctx.set_font(&format!(
            "bold {}px monospace",
            CONFLICT_WARNING_FONT_SIZE / zoom_level
        ));
        let warning_text = format!(
            "⚠ {} more conflicts not shown",
            conflicts.len() - MAX_CONFLICTS_DISPLAYED
        );
        let _ = ctx.fill_text(&warning_text, 10.0, dims.top_margin - 10.0);
    }
}

#[allow(clippy::too_many_arguments, clippy::cast_precision_loss)]
#[must_use]
pub fn check_conflict_hover(
    mouse_x: f64,
    mouse_y: f64,
    conflicts: &[Conflict],
    stations: &[StationNode],
    canvas_width: f64,
    canvas_height: f64,
    zoom_level: f64,
    zoom_level_x: f64,
    pan_offset_x: f64,
    pan_offset_y: f64,
) -> Option<(Conflict, f64, f64)> {
    use super::canvas::{BOTTOM_PADDING, LEFT_MARGIN, RIGHT_PADDING, TOP_MARGIN};
    use crate::time::time_to_fraction;

    let graph_width = canvas_width - LEFT_MARGIN - RIGHT_PADDING;
    let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;

    // Check if mouse is within the graph area first
    if mouse_x < LEFT_MARGIN
        || mouse_x > LEFT_MARGIN + graph_width
        || mouse_y < TOP_MARGIN
        || mouse_y > TOP_MARGIN + graph_height
    {
        return None;
    }

    for conflict in conflicts {
        // Calculate conflict position in screen coordinates
        // The canvas uses: translate(LEFT_MARGIN, TOP_MARGIN) + translate(pan) + scale(zoom)
        let time_fraction = time_to_fraction(conflict.time);
        let total_hours = 48.0;
        let hour_width = graph_width / total_hours;

        // Position in zoomed coordinate system (before translation)
        let x_in_zoomed = time_fraction * hour_width;

        let station_height = graph_height / stations.len() as f64;
        let y_in_zoomed = (conflict.station1_idx as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (conflict.station2_idx - conflict.station1_idx) as f64);

        // Transform to screen coordinates
        let screen_x = LEFT_MARGIN + (x_in_zoomed * zoom_level * zoom_level_x) + pan_offset_x;
        let screen_y = TOP_MARGIN + (y_in_zoomed * zoom_level) + pan_offset_y;

        // Check if mouse is within conflict marker bounds
        let size = CONFLICT_TRIANGLE_SIZE;
        if mouse_x >= screen_x - size
            && mouse_x <= screen_x + size
            && mouse_y >= screen_y - size
            && mouse_y <= screen_y + size
        {
            return Some((conflict.clone(), mouse_x, mouse_y));
        }
    }

    None
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_station_crossings(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    station_crossings: &[StationCrossing],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    for crossing in station_crossings {
        let time_fraction = time_to_fraction(crossing.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);
        // Use station_height / 2.0 offset to center on the station line
        let y = dims.top_margin
            + (crossing.station_idx as f64 * station_height)
            + (station_height / 2.0);

        // Draw a translucent green circle at the crossing point
        // Radius represents 1.5 minutes of travel time
        let one_minute_width = dims.hour_width / 60.0;
        let radius = one_minute_width * 1.5;

        ctx.begin_path();
        let _ = ctx.arc(x, y, radius, 0.0, 2.0 * std::f64::consts::PI);

        // Fill with translucent green
        ctx.set_fill_style_str(CROSSING_FILL_COLOR);
        ctx.fill();

        // Stroke with darker green border
        ctx.set_stroke_style_str(CROSSING_STROKE_COLOR);
        ctx.set_line_width(CROSSING_LINE_WIDTH / zoom_level);
        ctx.stroke();
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_block_violation_visualization(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflict: &Conflict,
    train_journeys: &[&crate::train_journey::TrainJourney],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    // Use the segment times stored in the conflict
    if let (Some((s1_start, s1_end)), Some((s2_start, s2_end))) =
        (conflict.segment1_times, conflict.segment2_times) {

        let start_idx = conflict.station1_idx;
        let end_idx = conflict.station2_idx;

        // Find the journeys to get their colors
        let journey1 = train_journeys.iter().find(|j| j.line_id == conflict.journey1_id);
        let journey2 = train_journeys.iter().find(|j| j.line_id == conflict.journey2_id);

        // Get colors from journeys (hex format like #FF0000)
        let color1 = journey1.map_or("#FF0000", |j| j.color.as_str());
        let color2 = journey2.map_or("#0000FF", |j| j.color.as_str());

        // Convert hex to rgba with transparency
        let fill1 = format!("{color1}{BLOCK_FILL_OPACITY}");
        let fill2 = format!("{color2}{BLOCK_FILL_OPACITY}");

        // Draw first journey's block
        draw_block_rectangle(
            ctx,
            dims,
            (s1_start, s1_end),
            (start_idx, end_idx),
            station_height,
            zoom_level,
            time_to_fraction,
            (&fill1, color1),
        );

        // Draw second journey's block
        draw_block_rectangle(
            ctx,
            dims,
            (s2_start, s2_end),
            (start_idx, end_idx),
            station_height,
            zoom_level,
            time_to_fraction,
            (&fill2, color2),
        );
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_journey_blocks(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    journey: &crate::train_journey::TrainJourney,
    stations: &[crate::models::StationNode],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {

    // Get journey color
    let color = journey.color.as_str();
    let fill_color = format!("{color}{BLOCK_FILL_OPACITY}");
    let stroke_color = format!("{color}{BLOCK_STROKE_OPACITY}");

    // Create station name to index mapping
    let station_map: std::collections::HashMap<&str, usize> = stations
        .iter()
        .enumerate()
        .map(|(idx, station)| (station.name.as_str(), idx))
        .collect();

    // Draw a block for each segment
    for i in 1..journey.station_times.len() {
        let (station_from, _arrival_from, departure_from) = &journey.station_times[i - 1];
        let (station_to, arrival_to, _departure_to) = &journey.station_times[i];

        // Look up station indices
        let Some(&start_idx) = station_map.get(station_from.as_str()) else {
            continue;
        };
        let Some(&end_idx) = station_map.get(station_to.as_str()) else {
            continue;
        };

        draw_block_rectangle(
            ctx,
            dims,
            (*departure_from, *arrival_to),
            (start_idx, end_idx),
            station_height,
            zoom_level,
            time_to_fraction,
            (&fill_color, &stroke_color),
        );
    }
}

#[allow(clippy::cast_precision_loss)]
fn draw_block_rectangle(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    times: (chrono::NaiveDateTime, chrono::NaiveDateTime),
    stations: (usize, usize),
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    colors: (&str, &str),
) {
    let (start_time, end_time) = times;
    let (start_station_idx, end_station_idx) = stations;
    let (fill_color, stroke_color) = colors;

    let x1 = dims.left_margin + (time_to_fraction(start_time) * dims.hour_width);
    let x2 = dims.left_margin + (time_to_fraction(end_time) * dims.hour_width);
    let y1 = dims.top_margin + (start_station_idx as f64 * station_height) + (station_height / 2.0);
    let y2 = dims.top_margin + (end_station_idx as f64 * station_height) + (station_height / 2.0);

    let width = x2 - x1;
    let height = y2 - y1;

    // Draw rectangle
    ctx.set_fill_style_str(fill_color);
    ctx.fill_rect(x1, y1, width, height);

    // Draw border with zoom-adjusted line width
    ctx.set_stroke_style_str(stroke_color);
    ctx.set_line_width(BLOCK_BORDER_WIDTH / zoom_level);
    ctx.stroke_rect(x1, y1, width, height);
}

