use web_sys::CanvasRenderingContext2d;
use crate::models::{RailwayGraph, Node};
use super::types::GraphDimensions;
use petgraph::stable_graph::NodeIndex;

// Station label constants
const STATION_LABEL_COLOR: &str = "#aaa";
const STATION_LABEL_FONT: &str = "11px monospace";
const STATION_LABEL_X: f64 = 5.0;
const STATION_LABEL_Y_OFFSET: f64 = 3.0;

// Junction constants
const JUNCTION_LABEL_COLOR: &str = "#ffb84d";
const JUNCTION_DIAMOND_SIZE: f64 = 6.0;
const JUNCTION_LABEL_X_OFFSET: f64 = 12.0;

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
                Node::Station(_) => {
                    draw_station_label(ctx, &station_node.display_name(), adjusted_y);
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

#[allow(clippy::cast_precision_loss)]
pub fn draw_segment_toggles(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[(NodeIndex, Node)],
    graph: &RailwayGraph,
    zoom_level: f64,
    pan_offset_y: f64,
) {
    use petgraph::visit::EdgeRef;

    let station_height = dims.graph_height / stations.len() as f64;

    for i in 1..stations.len() {
        let (node1, _) = &stations[i - 1];
        let (node2, _) = &stations[i];

        // Check if there's a multi-tracked edge between these stations
        let has_multiple_tracks = {
            // Check both directions for an edge
            graph.graph.edges(*node1).any(|e| {
                e.target() == *node2 && e.weight().tracks.len() >= 2
            }) || graph.graph.edges(*node2).any(|e| {
                e.target() == *node1 && e.weight().tracks.len() >= 2
            })
        };

        // Calculate position between the two stations
        let base_y1 = ((i - 1) as f64 * station_height) + (station_height / 2.0);
        let base_y2 = (i as f64 * station_height) + (station_height / 2.0);
        let center_y = (base_y1 + base_y2) / 2.0;
        let adjusted_y = dims.top_margin + (center_y * zoom_level) + pan_offset_y;

        // Only draw if visible
        if adjusted_y >= dims.top_margin && adjusted_y <= dims.top_margin + dims.graph_height {
            // Draw background
            let bg_color = if has_multiple_tracks {
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
            let icon = if has_multiple_tracks {
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
#[allow(clippy::cast_precision_loss)]
#[must_use]
pub fn check_toggle_click(
    mouse_x: f64,
    mouse_y: f64,
    canvas_height: f64,
    stations: &[(NodeIndex, Node)],
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