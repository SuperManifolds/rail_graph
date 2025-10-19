use web_sys::CanvasRenderingContext2d;
use crate::models::Node;
use super::types::GraphDimensions;
use petgraph::stable_graph::NodeIndex;

// Station label constants
const STATION_LABEL_COLOR: &str = "#ddd";
const SINGLE_PLATFORM_LABEL_COLOR: &str = "#999";
const PASSING_LOOP_LABEL_COLOR: &str = "#777";
const STATION_LABEL_FONT: &str = "11px monospace";
const STATION_LABEL_X: f64 = 5.0;
const STATION_LABEL_Y_OFFSET: f64 = 3.0;

// Junction constants
const JUNCTION_LABEL_COLOR: &str = "#ffb84d";
const JUNCTION_DIAMOND_SIZE: f64 = 6.0;
const JUNCTION_LABEL_X_OFFSET: f64 = 12.0;

#[allow(clippy::cast_precision_loss)]
pub fn draw_station_labels(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[(NodeIndex, Node)],
    zoom_level: f64,
    pan_offset_y: f64,
) {
    let station_height = dims.graph_height / stations.len() as f64;

    // Draw labels for each node in the stations list (includes both stations and junctions)
    for (idx, (_, station_node)) in stations.iter().enumerate() {
        let base_y = (idx as f64 * station_height) + (station_height / 2.0);
        let adjusted_y = dims.top_margin + (base_y * zoom_level) + pan_offset_y;

        // Only draw if visible
        if adjusted_y >= dims.top_margin && adjusted_y <= dims.top_margin + dims.graph_height {
            // Check if this is a junction or a station
            match station_node {
                Node::Station(station) => {
                    if station.passing_loop {
                        draw_passing_loop_label(ctx, &station_node.display_name(), adjusted_y);
                    } else if station.platforms.len() == 1 {
                        draw_single_platform_label(ctx, &station_node.display_name(), adjusted_y);
                    } else {
                        draw_station_label(ctx, &station_node.display_name(), adjusted_y);
                    }
                }
                Node::Junction(_) => {
                    draw_junction_label(ctx, Some(&station_node.display_name()), adjusted_y);
                }
            }
        }
    }
}

fn draw_station_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64) {
    ctx.set_fill_style_str(STATION_LABEL_COLOR);
    ctx.set_font(STATION_LABEL_FONT);
    let _ = ctx.fill_text(station, STATION_LABEL_X, y + STATION_LABEL_Y_OFFSET);
}

fn draw_single_platform_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64) {
    ctx.set_fill_style_str(SINGLE_PLATFORM_LABEL_COLOR);
    ctx.set_font(STATION_LABEL_FONT);
    let _ = ctx.fill_text(station, STATION_LABEL_X, y + STATION_LABEL_Y_OFFSET);
}

fn draw_passing_loop_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64) {
    ctx.set_fill_style_str(PASSING_LOOP_LABEL_COLOR);
    ctx.set_font(STATION_LABEL_FONT);
    let _ = ctx.fill_text(station, STATION_LABEL_X, y + STATION_LABEL_Y_OFFSET);
}

fn draw_junction_label(ctx: &CanvasRenderingContext2d, junction_name: Option<&str>, y: f64) {
    // Draw diamond icon
    ctx.set_fill_style_str(JUNCTION_LABEL_COLOR);
    ctx.set_stroke_style_str(JUNCTION_LABEL_COLOR);
    ctx.set_line_width(1.5);

    ctx.begin_path();
    let center_x = STATION_LABEL_X + JUNCTION_DIAMOND_SIZE / 2.0;
    ctx.move_to(center_x, y - JUNCTION_DIAMOND_SIZE / 2.0);
    ctx.line_to(center_x + JUNCTION_DIAMOND_SIZE / 2.0, y);
    ctx.line_to(center_x, y + JUNCTION_DIAMOND_SIZE / 2.0);
    ctx.line_to(center_x - JUNCTION_DIAMOND_SIZE / 2.0, y);
    ctx.close_path();
    ctx.stroke();

    // Draw junction name if it has one
    if let Some(name) = junction_name {
        ctx.set_fill_style_str(JUNCTION_LABEL_COLOR);
        ctx.set_font(STATION_LABEL_FONT);
        let _ = ctx.fill_text(name, STATION_LABEL_X + JUNCTION_LABEL_X_OFFSET, y + STATION_LABEL_Y_OFFSET);
    }
}