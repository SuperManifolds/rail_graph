use web_sys::CanvasRenderingContext2d;
use crate::models::{StationNode, RailwayGraph};
use super::types::GraphDimensions;
use petgraph::visit::EdgeRef;

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

#[allow(clippy::cast_precision_loss)]
pub fn draw_station_grid(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, stations: &[StationNode], zoom_level: f64, pan_offset_x: f64) {
    let station_height = dims.graph_height / stations.len() as f64;

    for (i, _station) in stations.iter().enumerate() {
        let y = calculate_station_y(dims, i, station_height);
        draw_horizontal_line(ctx, dims, y, zoom_level, pan_offset_x);
    }
}

#[allow(clippy::cast_precision_loss)]
fn calculate_station_y(dims: &GraphDimensions, index: usize, station_height: f64) -> f64 {
    dims.top_margin + (index as f64 * station_height) + (station_height / 2.0)
}

#[allow(clippy::cast_possible_truncation)]
fn draw_horizontal_line(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, y: f64, zoom_level: f64, pan_offset_x: f64) {
    ctx.set_stroke_style_str(STATION_GRID_COLOR);
    ctx.set_line_width(1.0 / zoom_level);
    ctx.begin_path();

    // Calculate visible range in the transformed coordinate system
    let x_min = -pan_offset_x / zoom_level;
    let x_max = (dims.graph_width - pan_offset_x) / zoom_level;

    // Calculate which hour lines are visible
    let start_hour = (x_min / dims.hour_width).floor() as i32 - GRID_PADDING_HOURS;
    let end_hour = (x_max / dims.hour_width).ceil() as i32 + GRID_PADDING_HOURS;

    let start_x = dims.left_margin + (f64::from(start_hour) * dims.hour_width);
    let end_x = dims.left_margin + (f64::from(end_hour) * dims.hour_width);

    ctx.move_to(start_x, y);
    ctx.line_to(end_x, y);
    ctx.stroke();
}

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn draw_double_track_indicators(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[StationNode],
    graph: &RailwayGraph,
    zoom_level: f64,
    pan_offset_x: f64,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    // Calculate visible range in the transformed coordinate system
    let x_min = -pan_offset_x / zoom_level;
    let x_max = (dims.graph_width - pan_offset_x) / zoom_level;

    let start_hour = (x_min / dims.hour_width).floor() as i32 - GRID_PADDING_HOURS;
    let end_hour = (x_max / dims.hour_width).ceil() as i32 + GRID_PADDING_HOURS;
    let start_x = dims.left_margin + (f64::from(start_hour) * dims.hour_width);
    let width = f64::from(end_hour - start_hour) * dims.hour_width;

    // Draw lighter background for double-tracked segments
    // Check each consecutive pair of stations
    for segment_idx in 1..stations.len() {
        let prev_station = &stations[segment_idx - 1];
        let curr_station = &stations[segment_idx];

        // Check if there's a multi-tracked edge between these stations
        let has_multiple_tracks = if let (Some(node1), Some(node2)) =
            (graph.get_station_index(&prev_station.name), graph.get_station_index(&curr_station.name)) {

            // Check both directions for an edge
            graph.graph.edges(node1).any(|e| {
                e.target() == node2 && e.weight().tracks.len() >= 2
            }) || graph.graph.edges(node2).any(|e| {
                e.target() == node1 && e.weight().tracks.len() >= 2
            })
        } else {
            false
        };

        if has_multiple_tracks {
            // Calculate the Y positions for the two stations
            let station1_y = calculate_station_y(dims, segment_idx - 1, station_height);
            let station2_y = calculate_station_y(dims, segment_idx, station_height);

            // Cover the entire area between the two stations
            let top_y = station1_y.min(station2_y);
            let height = (station2_y - station1_y).abs();

            // Draw lighter background rectangle
            ctx.set_fill_style_str(DOUBLE_TRACK_BG_COLOR);
            ctx.fill_rect(start_x, top_y, width, height);
        }
    }
}
