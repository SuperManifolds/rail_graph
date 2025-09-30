use web_sys::CanvasRenderingContext2d;
use crate::models::SegmentState;
use super::types::GraphDimensions;

// Background constants
const BACKGROUND_COLOR: &str = "#0a0a0a";

// Station grid constants
const STATION_GRID_COLOR: &str = "#1a1a1a";
const GRID_PADDING_HOURS: i32 = 5;

// Double track indicator constants
const DOUBLE_TRACK_BG_COLOR: &str = "rgba(255, 255, 255, 0.03)";

pub fn draw_background(ctx: &CanvasRenderingContext2d, width: f64, height: f64) {
    ctx.set_fill_style_str(BACKGROUND_COLOR);
    ctx.fill_rect(0.0, 0.0, width, height);
}

pub fn draw_station_grid(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, stations: &[String]) {
    let station_height = dims.graph_height / stations.len() as f64;

    for (i, _station) in stations.iter().enumerate() {
        let y = calculate_station_y(dims, i, station_height);
        draw_horizontal_line(ctx, dims, y);
    }
}

fn calculate_station_y(dims: &GraphDimensions, index: usize, station_height: f64) -> f64 {
    dims.top_margin + (index as f64 * station_height) + (station_height / 2.0)
}

fn draw_horizontal_line(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, y: f64) {
    ctx.set_stroke_style_str(STATION_GRID_COLOR);
    ctx.begin_path();

    // Calculate the same extended range as the hour grid
    let hours_visible = (dims.graph_width / dims.hour_width).ceil() as i32;
    let start_hour = -GRID_PADDING_HOURS;
    let end_hour = hours_visible + GRID_PADDING_HOURS;

    let start_x = dims.left_margin + (start_hour as f64 * dims.hour_width);
    let end_x = dims.left_margin + (end_hour as f64 * dims.hour_width);

    ctx.move_to(start_x, y);
    ctx.line_to(end_x, y);
    ctx.stroke();
}

pub fn draw_double_track_indicators(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[String],
    segment_state: &SegmentState,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    // Draw lighter background for double-tracked segments
    for &segment_idx in &segment_state.double_tracked_segments {
        if segment_idx > 0 && segment_idx < stations.len() {
            // Calculate the Y positions for the two stations
            let station1_y = calculate_station_y(dims, segment_idx - 1, station_height);
            let station2_y = calculate_station_y(dims, segment_idx, station_height);

            // Cover the entire area between the two stations
            let top_y = station1_y.min(station2_y);
            let height = (station2_y - station1_y).abs();

            // Calculate the same extended range as other grid elements
            let hours_visible = (dims.graph_width / dims.hour_width).ceil() as i32;
            let start_hour = -GRID_PADDING_HOURS;
            let end_hour = hours_visible + GRID_PADDING_HOURS;
            let start_x = dims.left_margin + (start_hour as f64 * dims.hour_width);
            let width = (end_hour - start_hour) as f64 * dims.hour_width;

            // Draw lighter background rectangle
            ctx.set_fill_style_str(DOUBLE_TRACK_BG_COLOR);
            ctx.fill_rect(start_x, top_y, width, height);
        }
    }
}