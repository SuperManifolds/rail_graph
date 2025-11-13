use web_sys::CanvasRenderingContext2d;
use crate::models::{Node, RailwayGraph};
use crate::theme::Theme;
use super::types::GraphDimensions;
use petgraph::visit::EdgeRef;
use petgraph::stable_graph::NodeIndex;

const GRID_PADDING_HOURS: i32 = 5;

struct Palette {
    background: &'static str,
    station_grid: &'static str,
    single_platform_grid: &'static str,
    junction_grid: &'static str,
    double_track_bg: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    background: "#0a0a0a",
    station_grid: "#1a1a1a",
    single_platform_grid: "#121212",
    junction_grid: "#ffb84d",
    double_track_bg: "rgba(255, 255, 255, 0.03)",
};

const LIGHT_PALETTE: Palette = Palette {
    background: "#fafafa",
    station_grid: "#e0e0e0",
    single_platform_grid: "#ebebeb",
    junction_grid: "#cc8800",
    double_track_bg: "rgba(0, 0, 0, 0.02)",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

pub fn draw_background(ctx: &CanvasRenderingContext2d, width: f64, height: f64, theme: Theme) {
    let palette = get_palette(theme);
    ctx.set_fill_style_str(palette.background);
    ctx.fill_rect(0.0, 0.0, width, height);
}

pub fn draw_station_grid(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[(NodeIndex, Node)],
    station_y_positions: &[f64],
    zoom_level: f64,
    pan_offset_x: f64,
    theme: Theme,
) {
    use super::canvas::TOP_MARGIN;
    let palette = get_palette(theme);

    for (i, (_, station_node)) in stations.iter().enumerate() {
        // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
        let y = station_y_positions[i] - TOP_MARGIN;

        // Use different color for single-platform stations and junctions
        let color = match station_node {
            Node::Station(station) if station.platforms.len() == 1 => {
                palette.single_platform_grid
            }
            Node::Junction(_) => {
                palette.junction_grid
            }
            Node::Station(_) => palette.station_grid,
        };

        draw_horizontal_line(ctx, dims, y, zoom_level, pan_offset_x, color);
    }
}

#[allow(clippy::cast_possible_truncation)]
fn draw_horizontal_line(ctx: &CanvasRenderingContext2d, dims: &GraphDimensions, y: f64, zoom_level: f64, pan_offset_x: f64, color: &str) {
    ctx.set_stroke_style_str(color);
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
    stations: &[(NodeIndex, Node)],
    station_y_positions: &[f64],
    graph: &RailwayGraph,
    zoom_level: f64,
    pan_offset_x: f64,
    theme: Theme,
) {
    use super::canvas::TOP_MARGIN;
    let palette = get_palette(theme);

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
        let (node1, _) = &stations[segment_idx - 1];
        let (node2, _) = &stations[segment_idx];

        // Check if there's a multi-tracked edge between these stations
        let has_multiple_tracks = {
            // Check both directions for an edge
            graph.graph.edges(*node1).any(|e| {
                e.target() == *node2 && e.weight().tracks.len() >= 2
            }) || graph.graph.edges(*node2).any(|e| {
                e.target() == *node1 && e.weight().tracks.len() >= 2
            })
        };

        if has_multiple_tracks {
            // Get the Y positions for the two stations
            // Note: station_y_positions include the original TOP_MARGIN, subtract it for transformed coords
            let station1_y = station_y_positions[segment_idx - 1] - TOP_MARGIN;
            let station2_y = station_y_positions[segment_idx] - TOP_MARGIN;

            // Cover the entire area between the two stations
            let top_y = station1_y.min(station2_y);
            let height = (station2_y - station1_y).abs();

            // Draw lighter background rectangle
            ctx.set_fill_style_str(palette.double_track_bg);
            ctx.fill_rect(start_x, top_y, width, height);
        }
    }
}
