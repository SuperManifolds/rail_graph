use web_sys::CanvasRenderingContext2d;
use crate::models::{SegmentState, Station};
use super::types::GraphDimensions;

// Station label constants
const STATION_LABEL_COLOR: &str = "#aaa";
const STATION_LABEL_FONT: &str = "11px monospace";
const STATION_LABEL_X: f64 = 5.0;
const STATION_LABEL_Y_OFFSET: f64 = 3.0;

// Segment toggle constants
const TOGGLE_X: f64 = 85.0;
const TOGGLE_SIZE: f64 = 12.0;
const TOGGLE_DOUBLE_TRACK_BG: &str = "rgba(255, 255, 255, 0.1)";
const TOGGLE_SINGLE_TRACK_BG: &str = "rgba(0, 0, 0, 0.3)";
const TOGGLE_BORDER_COLOR: &str = "#666";
const TOGGLE_BORDER_WIDTH: f64 = 1.0;
const TOGGLE_ICON_COLOR: &str = "#fff";
const TOGGLE_ICON_FONT: &str = "10px monospace";
const TOGGLE_ICON_X_OFFSET: f64 = -4.0;
const TOGGLE_ICON_Y_OFFSET: f64 = 4.0;
const TOGGLE_DOUBLE_TRACK_ICON: &str = "≡";
const TOGGLE_SINGLE_TRACK_ICON: &str = "─";

pub fn draw_station_labels(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    zoom_level: f64,
    pan_offset_y: f64,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    for (i, station) in stations.iter().enumerate() {
        let base_y = (i as f64 * station_height) + (station_height / 2.0);
        let adjusted_y = dims.top_margin + (base_y * zoom_level) + pan_offset_y;

        // Only draw label if it's within the visible graph area
        if adjusted_y >= dims.top_margin && adjusted_y <= dims.top_margin + dims.graph_height {
            draw_station_label(ctx, station, adjusted_y);
        }
    }
}

fn draw_station_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64) {
    ctx.set_fill_style_str(STATION_LABEL_COLOR);
    ctx.set_font(STATION_LABEL_FONT);
    let _ = ctx.fill_text(station, STATION_LABEL_X, y + STATION_LABEL_Y_OFFSET);
}

pub fn draw_segment_toggles(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    segment_state: &SegmentState,
    zoom_level: f64,
    pan_offset_y: f64,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    for i in 1..stations.len() {
        let segment_index = i;
        let is_double_tracked = segment_state
            .double_tracked_segments
            .contains(&segment_index);

        // Calculate position between the two stations
        let base_y1 = ((i - 1) as f64 * station_height) + (station_height / 2.0);
        let base_y2 = (i as f64 * station_height) + (station_height / 2.0);
        let center_y = (base_y1 + base_y2) / 2.0;
        let adjusted_y = dims.top_margin + (center_y * zoom_level) + pan_offset_y;

        // Only draw if visible
        if adjusted_y >= dims.top_margin && adjusted_y <= dims.top_margin + dims.graph_height {
            // Draw background
            let bg_color = if is_double_tracked {
                TOGGLE_DOUBLE_TRACK_BG
            } else {
                TOGGLE_SINGLE_TRACK_BG
            };
            ctx.set_fill_style_str(bg_color);
            ctx.fill_rect(
                TOGGLE_X - TOGGLE_SIZE/2.0,
                adjusted_y - TOGGLE_SIZE/2.0,
                TOGGLE_SIZE,
                TOGGLE_SIZE
            );

            // Draw border
            ctx.set_stroke_style_str(TOGGLE_BORDER_COLOR);
            ctx.set_line_width(TOGGLE_BORDER_WIDTH);
            ctx.stroke_rect(
                TOGGLE_X - TOGGLE_SIZE/2.0,
                adjusted_y - TOGGLE_SIZE/2.0,
                TOGGLE_SIZE,
                TOGGLE_SIZE
            );

            // Draw icon
            ctx.set_fill_style_str(TOGGLE_ICON_COLOR);
            ctx.set_font(TOGGLE_ICON_FONT);
            let icon = if is_double_tracked {
                TOGGLE_DOUBLE_TRACK_ICON
            } else {
                TOGGLE_SINGLE_TRACK_ICON
            };
            let _ = ctx.fill_text(
                icon,
                TOGGLE_X + TOGGLE_ICON_X_OFFSET,
                adjusted_y + TOGGLE_ICON_Y_OFFSET
            );
        }
    }
}

/// Check if a mouse click hit a toggle button for double-track segments
pub fn check_toggle_click(
    mouse_x: f64,
    mouse_y: f64,
    canvas_height: f64,
    stations: &[Station],
    zoom_level: f64,
    pan_offset_y: f64,
) -> Option<usize> {
    use super::canvas::{TOP_MARGIN, BOTTOM_PADDING};

    let graph_height = canvas_height - TOP_MARGIN - BOTTOM_PADDING;
    let station_height = graph_height / stations.len() as f64;

    // Check if click is in the toggle area horizontally
    if (TOGGLE_X - TOGGLE_SIZE/2.0..=TOGGLE_X + TOGGLE_SIZE/2.0).contains(&mouse_x) {
        // Check each segment toggle
        for i in 1..stations.len() {
            let segment_index = i;

            // Calculate position between the two stations (same logic as draw_segment_toggles)
            let base_y1 = ((i - 1) as f64 * station_height) + (station_height / 2.0);
            let base_y2 = (i as f64 * station_height) + (station_height / 2.0);
            let center_y = (base_y1 + base_y2) / 2.0;
            let adjusted_y = TOP_MARGIN + (center_y * zoom_level) + pan_offset_y;

            // Check if click is within this toggle button
            if (adjusted_y - TOGGLE_SIZE/2.0..=adjusted_y + TOGGLE_SIZE/2.0).contains(&mouse_y) {
                return Some(segment_index);
            }
        }
    }

    None
}