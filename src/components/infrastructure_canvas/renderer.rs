use crate::models::RailwayGraph;
use super::{track_renderer, station_renderer};
use web_sys::CanvasRenderingContext2d;
use petgraph::stable_graph::NodeIndex;

const CANVAS_BACKGROUND_COLOR: &str = "#0a0a0a";
const EMPTY_MESSAGE_COLOR: &str = "#666";
const EMPTY_MESSAGE_FONT: &str = "16px sans-serif";
const EMPTY_MESSAGE_TEXT: &str = "No stations in network";
const EMPTY_MESSAGE_OFFSET_X: f64 = 80.0;

const GRID_SIZE: f64 = 30.0; // Must match auto_layout.rs GRID_SIZE
const GRID_COLOR: &str = "#141414";
const GRID_LINE_WIDTH: f64 = 0.25;

/// Draw a subtle grid pattern to show snap points
fn draw_grid(
    ctx: &CanvasRenderingContext2d,
    width: f64,
    height: f64,
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
) {
    ctx.save();

    ctx.set_stroke_style_str(GRID_COLOR);
    ctx.set_line_width(GRID_LINE_WIDTH);

    // Calculate visible world bounds
    let left = -pan_x / zoom;
    let top = -pan_y / zoom;
    let right = (width - pan_x) / zoom;
    let bottom = (height - pan_y) / zoom;

    // Round to nearest grid line
    let start_x = (left / GRID_SIZE).floor() * GRID_SIZE;
    let start_y = (top / GRID_SIZE).floor() * GRID_SIZE;

    // Apply transformations
    let _ = ctx.translate(pan_x, pan_y);
    let _ = ctx.scale(zoom, zoom);

    ctx.begin_path();

    // Draw vertical lines
    let mut x = start_x;
    while x <= right {
        ctx.move_to(x, top);
        ctx.line_to(x, bottom);
        x += GRID_SIZE;
    }

    // Draw horizontal lines
    let mut y = start_y;
    while y <= bottom {
        ctx.move_to(left, y);
        ctx.line_to(right, y);
        y += GRID_SIZE;
    }

    ctx.stroke();
    ctx.restore();
}

pub fn draw_infrastructure(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    (width, height): (f64, f64),
    zoom: f64,
    pan_x: f64,
    pan_y: f64,
    selected_stations: &[NodeIndex],
) {
    // Clear canvas
    ctx.set_fill_style_str(CANVAS_BACKGROUND_COLOR);
    ctx.fill_rect(0.0, 0.0, width, height);

    // Draw grid
    draw_grid(ctx, width, height, zoom, pan_x, pan_y);

    if graph.graph.node_count() == 0 {
        // Show message if no stations
        ctx.set_fill_style_str(EMPTY_MESSAGE_COLOR);
        ctx.set_font(EMPTY_MESSAGE_FONT);
        let _ = ctx.fill_text(EMPTY_MESSAGE_TEXT, width / 2.0 - EMPTY_MESSAGE_OFFSET_X, height / 2.0);
        return;
    }

    // Save context and apply transformations
    ctx.save();
    let _ = ctx.translate(pan_x, pan_y);
    let _ = ctx.scale(zoom, zoom);

    // Draw tracks first so they're behind nodes
    track_renderer::draw_tracks(ctx, graph, zoom);

    // Draw stations and junctions on top
    station_renderer::draw_stations(ctx, graph, zoom, selected_stations);

    // Restore context
    ctx.restore();
}
