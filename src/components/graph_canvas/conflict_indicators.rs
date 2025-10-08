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
const CONFLICT_LABEL_COLOR: &str = "rgba(255, 255, 255, 0.9)";
const CONFLICT_LABEL_FONT_SIZE: f64 = 9.0;
const CONFLICT_WARNING_COLOR: &str = "rgba(255, 0, 0, 0.8)";
const CONFLICT_WARNING_FONT_SIZE: f64 = 14.0;
const MAX_CONFLICTS_DISPLAYED: usize = 1000;

// Station crossing constants
const CROSSING_FILL_COLOR: &str = "rgba(0, 200, 100, 0.3)";
const CROSSING_STROKE_COLOR: &str = "rgba(0, 150, 75, 0.6)";
const CROSSING_LINE_WIDTH: f64 = 2.0;

pub fn draw_conflict_highlights(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflicts: &[Conflict],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {
    // Limit to first 1000 conflicts to prevent performance issues
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

        // Draw a warning triangle at the conflict point
        let size = CONFLICT_TRIANGLE_SIZE / zoom_level;
        ctx.set_line_width(CONFLICT_LINE_WIDTH / zoom_level);

        // Draw filled triangle
        ctx.begin_path();
        ctx.move_to(x, y - size); // Top point
        ctx.line_to(x - size * 0.866, y + size * 0.5); // Bottom left
        ctx.line_to(x + size * 0.866, y + size * 0.5); // Bottom right
        ctx.close_path();

        // Fill with warning color
        ctx.set_fill_style_str(CONFLICT_FILL_COLOR);
        ctx.fill();

        // Stroke with thick black border
        ctx.set_stroke_style_str(CONFLICT_STROKE_COLOR);
        ctx.stroke();

        // Draw exclamation mark inside triangle
        ctx.set_fill_style_str(CONFLICT_ICON_COLOR);
        ctx.set_font(&format!(
            "bold {}px sans-serif",
            CONFLICT_ICON_FONT_SIZE / zoom_level
        ));
        let _ = ctx.fill_text("!", x - 2.0 / zoom_level, y + 4.0 / zoom_level);

        // Draw conflict details (simplified - just show line IDs)
        ctx.set_fill_style_str(CONFLICT_LABEL_COLOR);
        ctx.set_font(&format!(
            "{}px monospace",
            CONFLICT_LABEL_FONT_SIZE / zoom_level
        ));
        let label = format!("{} × {}", conflict.journey1_id, conflict.journey2_id);
        let _ = ctx.fill_text(&label, x + size + 5.0 / zoom_level, y);
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

#[allow(clippy::too_many_arguments)]
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
        let size = 15.0;
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

pub fn draw_block_violation_visualization(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflict: &Conflict,
    train_journeys: &[crate::train_journey::TrainJourney],
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
        let color1 = journey1.map(|j| j.color.as_str()).unwrap_or("#FF0000");
        let color2 = journey2.map(|j| j.color.as_str()).unwrap_or("#0000FF");

        // Convert hex to rgba with transparency
        let fill1 = format!("{}33", color1); // Add 33 for ~20% opacity
        let fill2 = format!("{}33", color2);

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
    ctx.set_line_width(1.0 / zoom_level);
    ctx.stroke_rect(x1, y1, width, height);
}

