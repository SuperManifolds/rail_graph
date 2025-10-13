use web_sys::CanvasRenderingContext2d;
use crate::models::{RailwayGraph, StationNode, Nodes, Node};
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
    stations: &[(NodeIndex, StationNode)],
    graph: &RailwayGraph,
    zoom_level: f64,
    pan_offset_y: f64,
) {
    let all_nodes = graph.get_all_nodes_ordered();
    let station_height = dims.graph_height / stations.len() as f64;

    // First pass: determine Y positions for all nodes
    let mut station_positions = Vec::new();
    let mut junction_positions = Vec::new();
    let mut prev_station_row: Option<usize> = None;

    for node in &all_nodes {
        match node {
            Node::Station(station) => {
                let station_index = stations.iter().position(|(_, s)| s.name == station.name);
                if let Some(idx) = station_index {
                    let base_y = (idx as f64 * station_height) + (station_height / 2.0);
                    let adjusted_y = dims.top_margin + (base_y * zoom_level) + pan_offset_y;
                    station_positions.push((station.name.clone(), adjusted_y));
                    prev_station_row = Some(idx);
                }
            }
            Node::Junction(junction) => {
                // Junction should be between the previous station and the next station
                // We'll position it in the second pass after we know both station positions
                junction_positions.push((junction.clone(), prev_station_row));
            }
        }
    }

    // Second pass: calculate junction positions based on their adjacent stations
    let mut junction_y_positions = Vec::new();
    for (junction, prev_row) in &junction_positions {
        if let Some(prev) = prev_row {
            // Junction is between prev station and next station (prev + 1)
            let prev_y = (*prev as f64 * station_height) + (station_height / 2.0);
            let next_y = ((*prev + 1) as f64 * station_height) + (station_height / 2.0);
            let junction_base_y = (prev_y + next_y) / 2.0;
            let adjusted_y = dims.top_margin + (junction_base_y * zoom_level) + pan_offset_y;
            junction_y_positions.push((junction.name.clone(), adjusted_y));
        }
    }

    // Draw stations
    for (station_name, y) in &station_positions {
        if *y >= dims.top_margin && *y <= dims.top_margin + dims.graph_height {
            draw_station_label(ctx, station_name, *y);
        }
    }

    // Draw junctions
    for (junction_name, y) in &junction_y_positions {
        if *y >= dims.top_margin && *y <= dims.top_margin + dims.graph_height {
            draw_junction_label(ctx, junction_name.as_deref(), *y);
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
    stations: &[(NodeIndex, StationNode)],
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
    stations: &[(NodeIndex, StationNode)],
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