use crate::models::RailwayGraph;
use super::{track_renderer, station_renderer};
use web_sys::CanvasRenderingContext2d;
use petgraph::stable_graph::NodeIndex;

const CANVAS_BACKGROUND_COLOR: &str = "#0a0a0a";
const EMPTY_MESSAGE_COLOR: &str = "#666";
const EMPTY_MESSAGE_FONT: &str = "16px sans-serif";
const EMPTY_MESSAGE_TEXT: &str = "No stations in network";
const EMPTY_MESSAGE_OFFSET_X: f64 = 80.0;

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
