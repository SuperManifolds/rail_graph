use super::types::GraphDimensions;
use crate::models::Node;
use crate::conflict::Conflict;
use web_sys::{CanvasRenderingContext2d, Path2d};

// Conflict highlight constants
const CONFLICT_TRIANGLE_SIZE: f64 = 15.0;
const CONFLICT_LABEL_COLOR: &str = "rgba(255, 255, 255, 0.9)";
const CONFLICT_LABEL_FONT_SIZE: f64 = 9.0;
const CONFLICT_LABEL_OFFSET: f64 = 5.0;
const CONFLICT_WARNING_COLOR: &str = "rgba(255, 0, 0, 0.8)";
const CONFLICT_WARNING_FONT_SIZE: f64 = 14.0;
const MAX_CONFLICTS_DISPLAYED: usize = 9999;

// Block visualization constants
const BLOCK_FILL_OPACITY: &str = "33"; // ~20% opacity in hex
const BLOCK_STROKE_OPACITY: &str = "99"; // ~60% opacity in hex
const BLOCK_BORDER_WIDTH: f64 = 1.0;

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn draw_conflict_highlights(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflicts: &[&Conflict],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    station_idx_map: &std::collections::HashMap<usize, usize>,
) {
    let size = CONFLICT_TRIANGLE_SIZE / zoom_level;
    let bar_width = 1.5 / zoom_level;
    let bar_height = 6.0 / zoom_level;
    let dot_radius = 1.0 / zoom_level;

    // Create reusable Path2D objects for the triangle shape centered at origin
    let triangle_path = Path2d::new().ok();
    if let Some(ref path) = triangle_path {
        path.move_to(0.0, -size);
        path.line_to(-size * 0.866, size * 0.5);
        path.line_to(size * 0.866, size * 0.5);
        path.close_path();
    }

    // Create reusable Path2D for the exclamation dot
    let dot_path = Path2d::new().ok();
    if let Some(ref path) = dot_path {
        let _ = path.arc(0.0, 4.0 / zoom_level, dot_radius, 0.0, std::f64::consts::PI * 2.0);
    }

    // Set styles once
    ctx.set_line_width(1.5 / zoom_level);

    // Draw each marker by translating and stamping the paths
    for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
        // Map full-graph indices to display indices
        let Some(&display_idx1) = station_idx_map.get(&conflict.station1_idx) else {
            continue; // Station not in current view
        };
        let Some(&display_idx2) = station_idx_map.get(&conflict.station2_idx) else {
            continue; // Station not in current view
        };

        let time_fraction = time_to_fraction(conflict.time);
        let x = dims.left_margin + (time_fraction * dims.hour_width);

        let y = dims.top_margin
            + (display_idx1 as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (display_idx2 as f64 - display_idx1 as f64));

        ctx.save();
        ctx.translate(x, y).ok();

        // Draw triangle using Path2D
        if let Some(ref path) = triangle_path {
            ctx.set_fill_style_str("rgba(255, 200, 0, 0.9)");
            ctx.set_stroke_style_str("rgba(0, 0, 0, 0.8)");
            ctx.fill_with_path_2d(path);
            ctx.stroke_with_path(path);
        }

        // Draw exclamation bar
        ctx.set_fill_style_str("#000");
        ctx.fill_rect(-bar_width / 2.0, -bar_height / 2.0 - 1.0 / zoom_level, bar_width, bar_height);

        // Draw exclamation dot using Path2D
        if let Some(ref path) = dot_path {
            ctx.fill_with_path_2d(path);
        }

        ctx.restore();
    }

    // Only draw labels when zoomed in enough (zoom level > 2.0)
    if zoom_level > 2.0 {
        // Set font and color for labels once
        ctx.set_fill_style_str(CONFLICT_LABEL_COLOR);
        ctx.set_font(&format!(
            "{}px monospace",
            CONFLICT_LABEL_FONT_SIZE / zoom_level
        ));

        // Draw all labels
        for conflict in conflicts.iter().take(MAX_CONFLICTS_DISPLAYED) {
            // Map full-graph indices to display indices
            let Some(&display_idx1) = station_idx_map.get(&conflict.station1_idx) else {
                continue; // Station not in current view
            };
            let Some(&display_idx2) = station_idx_map.get(&conflict.station2_idx) else {
                continue; // Station not in current view
            };

            let time_fraction = time_to_fraction(conflict.time);
            let x = dims.left_margin + (time_fraction * dims.hour_width);
            let y = dims.top_margin
                + (display_idx1 as f64 * station_height)
                + (station_height / 2.0)
                + (conflict.position
                    * station_height
                    * (display_idx2 as f64 - display_idx1 as f64));

            let label = format!("{} × {}", conflict.journey1_id, conflict.journey2_id);
            let _ = ctx.fill_text(&label, x + size + CONFLICT_LABEL_OFFSET / zoom_level, y);
        }
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
    stations: &[(petgraph::stable_graph::NodeIndex, Node)],
    canvas_width: f64,
    canvas_height: f64,
    zoom_level: f64,
    zoom_level_x: f64,
    pan_offset_x: f64,
    pan_offset_y: f64,
    station_idx_map: &std::collections::HashMap<usize, usize>,
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
        // Map full-graph indices to display indices
        let Some(&display_idx1) = station_idx_map.get(&conflict.station1_idx) else {
            continue; // Station not in current view
        };
        let Some(&display_idx2) = station_idx_map.get(&conflict.station2_idx) else {
            continue; // Station not in current view
        };

        // Calculate conflict position in screen coordinates
        // The canvas uses: translate(LEFT_MARGIN, TOP_MARGIN) + translate(pan) + scale(zoom)
        let time_fraction = time_to_fraction(conflict.time);
        let total_hours = 48.0;
        let hour_width = graph_width / total_hours;

        // Position in zoomed coordinate system (before translation)
        let x_in_zoomed = time_fraction * hour_width;

        let station_height = graph_height / stations.len() as f64;
        let y_in_zoomed = (display_idx1 as f64 * station_height)
            + (station_height / 2.0)
            + (conflict.position
                * station_height
                * (display_idx2 as f64 - display_idx1 as f64));

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
pub fn draw_block_violation_visualization(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    conflict: &Conflict,
    train_journeys: &[&crate::train_journey::TrainJourney],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
    station_idx_map: &std::collections::HashMap<usize, usize>,
) {
    // Use the segment times stored in the conflict
    if let (Some((s1_start, s1_end)), Some((s2_start, s2_end))) =
        (conflict.segment1_times, conflict.segment2_times) {

        // Map full-graph indices to display indices
        let Some(&display_idx1) = station_idx_map.get(&conflict.station1_idx) else {
            return; // Station not in current view
        };
        let Some(&display_idx2) = station_idx_map.get(&conflict.station2_idx) else {
            return; // Station not in current view
        };

        // Find the journeys to get their colors
        let journey1 = train_journeys.iter().find(|j| j.train_number == conflict.journey1_id);
        let journey2 = train_journeys.iter().find(|j| j.train_number == conflict.journey2_id);

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
            (display_idx1, display_idx2),
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
            (display_idx1, display_idx2),
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
    stations: &[(petgraph::stable_graph::NodeIndex, crate::models::Node)],
    station_height: f64,
    zoom_level: f64,
    time_to_fraction: fn(chrono::NaiveDateTime) -> f64,
) {

    // Get journey color
    let color = journey.color.as_str();
    let fill_color = format!("{color}{BLOCK_FILL_OPACITY}");
    let stroke_color = format!("{color}{BLOCK_STROKE_OPACITY}");

    // Create NodeIndex to display index mapping
    let station_map: std::collections::HashMap<petgraph::stable_graph::NodeIndex, usize> = stations
        .iter()
        .enumerate()
        .map(|(idx, (node_idx, _))| (*node_idx, idx))
        .collect();

    // Draw a block for each segment
    for i in 1..journey.station_times.len() {
        let (node_from, _arrival_from, departure_from) = &journey.station_times[i - 1];
        let (node_to, arrival_to, _departure_to) = &journey.station_times[i];

        // Look up station indices
        let Some(&start_idx) = station_map.get(node_from) else {
            continue;
        };
        let Some(&end_idx) = station_map.get(node_to) else {
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

