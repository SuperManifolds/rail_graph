use web_sys::CanvasRenderingContext2d;
use crate::models::{StationNode, RailwayGraph, Nodes, Node};
use super::types::GraphDimensions;

// Junction indicator constants
const JUNCTION_INDICATOR_COLOR: &str = "rgba(255, 200, 100, 0.1)";
const JUNCTION_MARKER_COLOR: &str = "rgba(255, 200, 100, 0.3)";
const GRID_PADDING_HOURS: i32 = 5;

#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_lossless)]
pub fn draw_junction_indicators(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[StationNode],
    graph: &RailwayGraph,
    zoom_level: f64,
    pan_offset_x: f64,
) {
    let all_nodes = graph.get_all_nodes_ordered();

    // Calculate visible range in the transformed coordinate system
    let x_min = -pan_offset_x / zoom_level;
    let x_max = (dims.graph_width - pan_offset_x) / zoom_level;

    let start_hour = (x_min / dims.hour_width).floor() as i32 - GRID_PADDING_HOURS;
    let end_hour = (x_max / dims.hour_width).ceil() as i32 + GRID_PADDING_HOURS;
    let start_x = dims.left_margin + (f64::from(start_hour) * dims.hour_width);
    let width = f64::from(end_hour - start_hour) * dims.hour_width;

    // Map stations to their row indices for rendering
    let station_height = dims.graph_height / (stations.len().max(1)) as f64;

    // Count which row each node should appear in
    let mut station_row = 0;
    let mut junction_positions = Vec::new();

    for node in &all_nodes {
        match node {
            Node::Station(_) => {
                station_row += 1;
            }
            Node::Junction(_) => {
                // Junction appears between current station_row - 1 and station_row
                if station_row > 0 {
                    let y = dims.top_margin + ((station_row as f64 - 0.5) * station_height);
                    junction_positions.push(y);
                }
            }
        }
    }

    // Draw junction indicators
    for junction_y in junction_positions {
        // Draw a faint background highlight across the entire width
        ctx.set_fill_style_str(JUNCTION_INDICATOR_COLOR);
        ctx.fill_rect(start_x, junction_y - station_height / 4.0, width, station_height / 2.0);

        // Draw a small marker on the left side
        ctx.set_fill_style_str(JUNCTION_MARKER_COLOR);
        let marker_size = 4.0 / zoom_level;
        ctx.fill_rect(dims.left_margin - marker_size, junction_y - marker_size, marker_size * 2.0, marker_size * 2.0);
    }
}
