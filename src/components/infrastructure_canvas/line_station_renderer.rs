use crate::models::{RailwayGraph, Stations, Line};
use crate::theme::Theme;
use super::station_renderer::{LabelPosition, CachedLabelPosition};
use web_sys::CanvasRenderingContext2d;
use std::collections::HashMap;
use petgraph::stable_graph::NodeIndex;

const LINE_BASE_WIDTH: f64 = 3.0;
const TICK_LENGTH: f64 = 10.0;
const PILL_HEIGHT: f64 = 6.0;
const PILL_BORDER_WIDTH: f64 = 1.0;
const PILL_PADDING: f64 = 1.5;

struct Palette {
    pill_fill: &'static str,
    pill_border: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    pill_fill: "#ffffff",
    pill_border: "#000000",
};

const LIGHT_PALETTE: Palette = Palette {
    pill_fill: "#ffffff",
    pill_border: "#000000",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

/// Get all visible lines that stop at a given station
fn get_lines_at_station<'a>(
    station_idx: NodeIndex,
    lines: &'a [Line],
    graph: &RailwayGraph,
) -> Vec<&'a Line> {
    let mut lines_at_station = Vec::new();

    for line in lines {
        if !line.visible {
            continue;
        }

        // Check if this line's route includes this station
        let station_path = line.get_station_path(graph);
        if station_path.contains(&station_idx) {
            lines_at_station.push(line);
        }
    }

    lines_at_station
}

/// Draw a tick marker for a single line at a station
fn draw_single_line_tick(
    ctx: &CanvasRenderingContext2d,
    pos: (f64, f64),
    line_color: &str,
    label_position: LabelPosition,
    zoom: f64,
) {
    let tick_length = TICK_LENGTH;

    // Calculate tick direction based on label position
    let (dx, dy) = match label_position {
        LabelPosition::Right => (tick_length, 0.0),
        LabelPosition::Left => (-tick_length, 0.0),
        LabelPosition::Top => (0.0, -tick_length),
        LabelPosition::Bottom => (0.0, tick_length),
        LabelPosition::TopRight => {
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            (tick_length * cos45, -tick_length * cos45)
        }
        LabelPosition::TopLeft => {
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            (-tick_length * cos45, -tick_length * cos45)
        }
        LabelPosition::BottomRight => {
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            (tick_length * cos45, tick_length * cos45)
        }
        LabelPosition::BottomLeft => {
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            (-tick_length * cos45, tick_length * cos45)
        }
    };

    ctx.save();
    ctx.set_stroke_style_str(line_color);
    ctx.set_line_width(LINE_BASE_WIDTH / zoom);
    ctx.set_line_cap("round");
    ctx.begin_path();
    ctx.move_to(pos.0, pos.1);
    ctx.line_to(pos.0 + dx, pos.1 + dy);
    ctx.stroke();
    ctx.restore();
}

/// Calculate the angle of lines passing through a station
#[allow(clippy::cast_precision_loss)]
fn calculate_line_angle(
    station_idx: NodeIndex,
    graph: &RailwayGraph,
) -> f64 {
    use petgraph::visit::EdgeRef;

    // Get connected edges
    let mut angles = Vec::new();
    for edge in graph.graph.edges(station_idx) {
        let (source, target) = (edge.source(), edge.target());
        let other_node = if source == station_idx { target } else { source };

        if let (Some(station_pos), Some(other_pos)) = (
            graph.get_station_position(station_idx),
            graph.get_station_position(other_node)
        ) {
            let dx = other_pos.0 - station_pos.0;
            let dy = other_pos.1 - station_pos.1;
            let angle = dy.atan2(dx);
            angles.push(angle);
        }
    }

    if angles.is_empty() {
        return 0.0; // Default to horizontal
    }

    // Calculate average angle (handling wraparound)
    let avg_angle = angles.iter().sum::<f64>() / angles.len() as f64;
    avg_angle
}

/// Draw a pill marker covering multiple lines at a station
#[allow(clippy::cast_precision_loss)]
fn draw_multi_line_pill(
    ctx: &CanvasRenderingContext2d,
    pos: (f64, f64),
    station_idx: NodeIndex,
    station_lines: &[&Line],
    graph: &RailwayGraph,
    zoom: f64,
    palette: &Palette,
) {
    // Sort lines by ID for consistent ordering
    let mut sorted_lines = station_lines.to_vec();
    sorted_lines.sort_by_key(|line| line.id);

    // Calculate total width of all lines
    let line_widths: Vec<f64> = sorted_lines.iter()
        .map(|line| (LINE_BASE_WIDTH + line.thickness) / zoom)
        .collect();
    // Gap should be equal to width of a standard line (BASE + default thickness of 2.0)
    let gap_width = (LINE_BASE_WIDTH + 2.0) / zoom;
    let num_gaps = sorted_lines.len().saturating_sub(1);
    let total_width: f64 = line_widths.iter().sum::<f64>()
        + (num_gaps as f64) * gap_width;

    // Calculate pill dimensions
    let pill_width = total_width + (PILL_PADDING * 2.0);
    let pill_height = PILL_HEIGHT;

    // Calculate rotation based on line direction through station
    let angle = calculate_line_angle(station_idx, graph);

    // Draw pill centered at station position
    ctx.save();
    let _ = ctx.translate(pos.0, pos.1);
    let _ = ctx.rotate(angle);

    // Draw pill background
    ctx.set_fill_style_str(palette.pill_fill);
    ctx.set_stroke_style_str(palette.pill_border);
    ctx.set_line_width(PILL_BORDER_WIDTH / zoom);

    let half_width = pill_width / 2.0;
    let half_height = pill_height / 2.0;
    let radius = half_height;

    // Draw rounded rectangle
    ctx.begin_path();
    ctx.move_to(-half_width + radius, -half_height);
    ctx.line_to(half_width - radius, -half_height);
    let _ = ctx.arc(half_width - radius, 0.0, radius, -std::f64::consts::PI / 2.0, std::f64::consts::PI / 2.0);
    ctx.line_to(-half_width + radius, half_height);
    let _ = ctx.arc(-half_width + radius, 0.0, radius, std::f64::consts::PI / 2.0, -std::f64::consts::PI / 2.0);
    ctx.close_path();
    ctx.fill();
    ctx.stroke();

    // Draw individual line stripes inside the pill
    let mut current_offset = -total_width / 2.0;

    for (i, line) in sorted_lines.iter().enumerate() {
        let line_width = line_widths[i];

        // Draw filled rectangle for each line
        ctx.set_fill_style_str(&line.color);
        ctx.fill_rect(
            current_offset,
            -half_height + PILL_PADDING,
            line_width,
            (half_height - PILL_PADDING) * 2.0
        );

        current_offset += line_width + gap_width;
    }

    ctx.restore();
}

/// Draw station markers and labels for line mode
#[allow(clippy::too_many_arguments)]
pub fn draw_line_stations(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    lines: &[Line],
    zoom: f64,
    viewport_bounds: (f64, f64, f64, f64),
    label_cache: &Option<(f64, HashMap<NodeIndex, CachedLabelPosition>)>,
    theme: Theme,
) {
    let palette = get_palette(theme);
    let (left, top, right, bottom) = viewport_bounds;
    let margin = 100.0;

    // Draw stations
    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(node) = graph.graph.node_weight(idx) else { continue };

        // Skip junctions
        if node.as_junction().is_some() {
            continue;
        }

        // Viewport culling
        if pos.0 < left - margin || pos.0 > right + margin ||
           pos.1 < top - margin || pos.1 > bottom + margin {
            continue;
        }

        // Get lines stopping at this station
        let station_lines = get_lines_at_station(idx, lines, graph);

        if station_lines.is_empty() {
            continue;
        }

        // Get label position (from cache or manual override)
        let label_position = if let Some((_, cache)) = label_cache {
            cache.get(&idx).map(|c| c.position)
        } else {
            None
        }.or_else(|| {
            // Check for manual override
            if let Some(station) = node.as_station() {
                station.label_position
            } else {
                None
            }
        }).unwrap_or(LabelPosition::Right);

        // Draw marker based on number of lines
        if station_lines.len() == 1 {
            draw_single_line_tick(ctx, pos, &station_lines[0].color, label_position, zoom);
        } else {
            draw_multi_line_pill(ctx, pos, idx, &station_lines, graph, zoom, palette);
        }
    }

    // Draw labels using existing label rendering from station_renderer
    // Labels are already drawn by draw_cached_labels in station_renderer
}
