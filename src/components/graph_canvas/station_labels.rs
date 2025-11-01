use web_sys::CanvasRenderingContext2d;
use crate::models::Node;
use super::types::GraphDimensions;
use petgraph::stable_graph::NodeIndex;

// Station label constants
const STATION_LABEL_COLOR: &str = "#ddd";
const SINGLE_PLATFORM_LABEL_COLOR: &str = "#888";
const PASSING_LOOP_LABEL_COLOR: &str = "#777";
const STATION_LABEL_FONT: &str = "11px monospace";
const STATION_LABEL_X: f64 = 5.0;
const STATION_LABEL_Y_OFFSET: f64 = 3.0;
const LABEL_RIGHT_PADDING: f64 = 5.0; // Space between label and graph area

// Junction constants
const JUNCTION_LABEL_COLOR: &str = "#ffb84d";
const JUNCTION_DIAMOND_SIZE: f64 = 6.0;
const JUNCTION_LABEL_X_OFFSET: f64 = 12.0;

/// Truncate text with ellipsis if it exceeds the maximum width
/// Returns the potentially truncated text
fn truncate_text_with_ellipsis(ctx: &CanvasRenderingContext2d, text: &str, max_width: f64) -> String {
    // Measure the full text
    let metrics = ctx.measure_text(text).ok();
    let text_width = metrics.map_or(0.0, |m| m.width());

    if text_width <= max_width {
        return text.to_string();
    }

    // Text is too long, we need to truncate
    let ellipsis = "...";
    let ellipsis_width = ctx.measure_text(ellipsis).ok().map_or(15.0, |m| m.width());
    let available_width = max_width - ellipsis_width;

    // Binary search for the right number of characters
    let mut low = 0;
    let mut high = text.chars().count();
    let mut best_fit = 0;

    while low <= high {
        let mid = (low + high) / 2;
        let truncated: String = text.chars().take(mid).collect();
        let truncated_width = ctx.measure_text(&truncated).ok().map_or(0.0, |m| m.width());

        if truncated_width <= available_width {
            best_fit = mid;
            low = mid + 1;
        } else {
            high = mid.saturating_sub(1);
        }
    }

    let truncated: String = text.chars().take(best_fit).collect();
    format!("{truncated}{ellipsis}")
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_station_labels(
    ctx: &CanvasRenderingContext2d,
    dims: &GraphDimensions,
    stations: &[(NodeIndex, Node)],
    station_y_positions: &[f64],
    zoom_level: f64,
    pan_offset_y: f64,
) {
    use super::canvas::TOP_MARGIN as ORIGINAL_TOP_MARGIN;

    let station_label_width = dims.left_margin;
    // Draw labels for each node in the stations list (includes both stations and junctions)

    for (idx, (_, station_node)) in stations.iter().enumerate() {
        // station_y_positions include the original TOP_MARGIN, subtract it to get graph-relative coords
        // Then apply zoom and pan transformations to get screen coordinates
        let base_y = station_y_positions[idx] - ORIGINAL_TOP_MARGIN;
        let adjusted_y = dims.top_margin + (base_y * zoom_level) + pan_offset_y;

        // Only draw if visible
        if adjusted_y >= dims.top_margin && adjusted_y <= dims.top_margin + dims.graph_height {
            // Check if this is a junction or a station
            match station_node {
                Node::Station(station) => {
                    if station.passing_loop {
                        draw_passing_loop_label(ctx, &station_node.display_name(), adjusted_y, station_label_width);
                    } else if station.platforms.len() == 1 {
                        draw_single_platform_label(ctx, &station_node.display_name(), adjusted_y, station_label_width);
                    } else {
                        draw_station_label(ctx, &station_node.display_name(), adjusted_y, station_label_width);
                    }
                }
                Node::Junction(_) => {
                    draw_junction_label(ctx, Some(&station_node.display_name()), adjusted_y, station_label_width);
                }
            }
        }
    }
}

fn draw_station_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64, station_label_width: f64) {
    ctx.set_fill_style_str(STATION_LABEL_COLOR);
    ctx.set_font(STATION_LABEL_FONT);
    let max_width = station_label_width - STATION_LABEL_X - LABEL_RIGHT_PADDING;
    let text = truncate_text_with_ellipsis(ctx, station, max_width);
    let _ = ctx.fill_text(&text, STATION_LABEL_X, y + STATION_LABEL_Y_OFFSET);
}

fn draw_single_platform_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64, station_label_width: f64) {
    ctx.set_fill_style_str(SINGLE_PLATFORM_LABEL_COLOR);
    ctx.set_font(STATION_LABEL_FONT);
    let max_width = station_label_width - STATION_LABEL_X - LABEL_RIGHT_PADDING;
    let text = truncate_text_with_ellipsis(ctx, station, max_width);
    let _ = ctx.fill_text(&text, STATION_LABEL_X, y + STATION_LABEL_Y_OFFSET);
}

fn draw_passing_loop_label(ctx: &CanvasRenderingContext2d, station: &str, y: f64, station_label_width: f64) {
    ctx.set_fill_style_str(PASSING_LOOP_LABEL_COLOR);
    ctx.set_font(STATION_LABEL_FONT);
    let max_width = station_label_width - STATION_LABEL_X - LABEL_RIGHT_PADDING;
    let text = truncate_text_with_ellipsis(ctx, station, max_width);
    let _ = ctx.fill_text(&text, STATION_LABEL_X, y + STATION_LABEL_Y_OFFSET);
}

fn draw_junction_label(ctx: &CanvasRenderingContext2d, junction_name: Option<&str>, y: f64, station_label_width: f64) {
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
        let max_width = station_label_width - (STATION_LABEL_X + JUNCTION_LABEL_X_OFFSET) - LABEL_RIGHT_PADDING;
        let text = truncate_text_with_ellipsis(ctx, name, max_width);
        let _ = ctx.fill_text(&text, STATION_LABEL_X + JUNCTION_LABEL_X_OFFSET, y + STATION_LABEL_Y_OFFSET);
    }
}

/// Check if mouse is hovering over a station label
/// Returns the full station name and viewport coordinates for tooltip
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn check_station_label_hover(
    canvas_x: f64,
    canvas_y: f64,
    viewport_x: f64,
    viewport_y: f64,
    stations: &[(NodeIndex, Node)],
    station_y_positions: &[f64],
    top_margin: f64,
    zoom_level: f64,
    pan_offset_y: f64,
    station_label_width: f64,
) -> Option<(String, f64, f64)> {
    use super::canvas::TOP_MARGIN as ORIGINAL_TOP_MARGIN;
    const HOVER_Y_TOLERANCE: f64 = 8.0; // Vertical tolerance for hover detection

    // Check if mouse is in the label area (left margin)
    if canvas_x >= station_label_width {
        return None;
    }

    // Find which station label the mouse is over
    for (idx, (_, station_node)) in stations.iter().enumerate() {
        let base_y = station_y_positions[idx] - ORIGINAL_TOP_MARGIN;
        let adjusted_y = top_margin + (base_y * zoom_level) + pan_offset_y;

        // Check if mouse y is near this station's label
        if (canvas_y - adjusted_y).abs() < HOVER_Y_TOLERANCE {
            let full_name = station_node.display_name().clone();
            return Some((full_name, viewport_x, viewport_y));
        }
    }

    None
}