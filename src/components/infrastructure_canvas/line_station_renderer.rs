use super::station_renderer::{CachedLabelPosition, LabelPosition};
use crate::models::{Junctions, Line, RailwayGraph, Stations};
use crate::theme::Theme;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::collections::{HashMap, HashSet};
use web_sys::CanvasRenderingContext2d;

// Import section-related logic from line_renderer
use super::line_renderer::{
    assign_visual_positions_with_reuse, get_lines_in_section, identify_sections,
    order_lines_for_section,
};

const LINE_BASE_WIDTH: f64 = 3.0;
const TICK_LENGTH: f64 = 6.0;
const TICK_WIDTH: f64 = 5.0;
const PILL_HEIGHT: f64 = 16.0;
const PILL_BORDER_WIDTH: f64 = 1.0;
const PILL_PADDING: f64 = 3.0;

// Line name label constants
const LINE_LABEL_PADDING: f64 = 4.0;
const LINE_LABEL_HEIGHT: f64 = 18.0;
const LINE_LABEL_SPACING: f64 = 4.0;
const LINE_LABEL_MIN_WIDTH: f64 = 40.0;
const CHAR_WIDTH_ESTIMATE: f64 = 7.5;

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

/// Check if a line stops at a given station (has `wait_time` > 0, or is first/last station)
fn line_stops_at_station(station_idx: NodeIndex, line: &Line, graph: &RailwayGraph) -> bool {
    if line.forward_route.is_empty() {
        return false;
    }

    // Check if this is the first station (line starts here)
    if let Some(first_segment) = line.forward_route.first() {
        let first_edge_idx = EdgeIndex::new(first_segment.edge_index);
        if let Some((source, _target)) = graph.graph.edge_endpoints(first_edge_idx) {
            if source == station_idx {
                return true;
            }
        }
    }

    // Check if this is the last station (line ends here)
    if let Some(last_segment) = line.forward_route.last() {
        let last_edge_idx = EdgeIndex::new(last_segment.edge_index);
        if let Some((_source, target)) = graph.graph.edge_endpoints(last_edge_idx) {
            if target == station_idx {
                return true;
            }
        }
    }

    // Check for intermediate stops with wait_time > 0
    for segment in &line.forward_route {
        let edge_idx = EdgeIndex::new(segment.edge_index);
        if let Some((_source, target)) = graph.graph.edge_endpoints(edge_idx) {
            if target == station_idx && !segment.wait_time.is_zero() {
                return true;
            }
        }
    }

    false
}

/// Check if a station is a terminal (start or end) for a given line
fn is_line_terminal(station_idx: NodeIndex, line: &Line, graph: &RailwayGraph) -> bool {
    if line.forward_route.is_empty() {
        return false;
    }

    // Check if this is the first station (line starts here)
    if let Some(first_segment) = line.forward_route.first() {
        let first_edge_idx = EdgeIndex::new(first_segment.edge_index);
        if let Some((source, _target)) = graph.graph.edge_endpoints(first_edge_idx) {
            if source == station_idx {
                return true;
            }
        }
    }

    // Check if this is the last station (line ends here)
    if let Some(last_segment) = line.forward_route.last() {
        let last_edge_idx = EdgeIndex::new(last_segment.edge_index);
        if let Some((_source, target)) = graph.graph.edge_endpoints(last_edge_idx) {
            if target == station_idx {
                return true;
            }
        }
    }

    false
}

/// Get all visible lines that pass through a given station (stopping or not)
fn get_lines_through_station<'a>(
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

/// Get all visible lines that stop at a given station (`wait_time` > 0)
fn get_stopping_lines_at_station<'a>(
    station_idx: NodeIndex,
    lines: &'a [Line],
    graph: &RailwayGraph,
) -> Vec<&'a Line> {
    let mut stopping_lines = Vec::new();

    for line in lines {
        if !line.visible {
            continue;
        }

        if line_stops_at_station(station_idx, line, graph) {
            stopping_lines.push(line);
        }
    }

    stopping_lines
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
    ctx.set_line_width(TICK_WIDTH / zoom);
    ctx.set_line_cap("butt");
    ctx.begin_path();
    ctx.move_to(pos.0, pos.1);
    ctx.line_to(pos.0 + dx, pos.1 + dy);
    ctx.stroke();
    ctx.restore();
}

/// Draw a line name label in a colored rectangle for terminal stations
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
fn draw_line_name_label(
    ctx: &CanvasRenderingContext2d,
    line_name: &str,
    line_color: &str,
    pos: (f64, f64),
    label_position: super::station_renderer::LabelPosition,
    station_name: &str,
    zoom: f64,
    label_index: usize, // For stacking multiple labels
    line_extent: Option<(f64, f64, f64)>, // (angle, min_offset, max_offset) for line mode adjustment
) {
    use super::station_renderer::calculate_readable_text_color;

    let font_size = 14.0 / zoom;

    // Measure actual text widths using canvas measureText
    ctx.set_font(&format!("{font_size}px sans-serif"));

    let line_text_width = ctx
        .measure_text(line_name)
        .map_or_else(|_| line_name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom, |m| m.width());
    let rect_width = (line_text_width + (LINE_LABEL_PADDING * 2.0)).max(LINE_LABEL_MIN_WIDTH);
    let rect_height = LINE_LABEL_HEIGHT;

    // Use fixed width for stacking to ensure equal spacing between labels
    let stack_width = LINE_LABEL_MIN_WIDTH;

    // Measure station name width to position line label after it
    let station_name_width = ctx
        .measure_text(station_name)
        .map_or_else(|_| station_name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom, |m| m.width());

    let station_node_radius = 8.0; // NODE_RADIUS from station_renderer
    let station_label_offset = 12.0; // LABEL_OFFSET from station_renderer

    // Apply line extent adjustment (same as station labels in line mode)
    let adjusted_pos = if let Some((angle, min_offset, max_offset)) = line_extent {
        let perp_x = -angle.sin();
        let perp_y = angle.cos();

        let extent_offset = match label_position {
            super::station_renderer::LabelPosition::Right
            | super::station_renderer::LabelPosition::TopRight
            | super::station_renderer::LabelPosition::BottomRight => {
                if max_offset > 0.0 { max_offset } else { 0.0 }
            }
            super::station_renderer::LabelPosition::Left
            | super::station_renderer::LabelPosition::TopLeft
            | super::station_renderer::LabelPosition::BottomLeft => {
                if min_offset < 0.0 { min_offset } else { 0.0 }
            }
            super::station_renderer::LabelPosition::Top | super::station_renderer::LabelPosition::Bottom => {
                if max_offset.abs() > min_offset.abs() { max_offset } else { min_offset }
            }
        };

        (pos.0 + perp_x * extent_offset, pos.1 + perp_y * extent_offset)
    } else {
        pos
    };

    // Calculate position based on label direction
    // Labels are positioned after the station name and stack with equal spacing
    let (label_x, label_y) = match label_position {
        super::station_renderer::LabelPosition::Right => {
            // Station label starts at offset 20.0 and extends rightward by station_name_width
            // Line labels continue extending rightward
            let base_offset = station_node_radius + station_label_offset + station_name_width + LINE_LABEL_SPACING;
            let offset = base_offset + (label_index as f64 * (stack_width + LINE_LABEL_SPACING));
            (adjusted_pos.0 + offset, adjusted_pos.1 - rect_height / 2.0)
        }
        super::station_renderer::LabelPosition::Left => {
            // Station label ends at offset -20.0 and extends leftward by station_name_width
            // Line labels continue extending leftward
            let base_offset = station_node_radius + station_label_offset + station_name_width + LINE_LABEL_SPACING;
            let offset = base_offset + (label_index as f64 * (stack_width + LINE_LABEL_SPACING));
            (adjusted_pos.0 - offset - rect_width, adjusted_pos.1 - rect_height / 2.0)
        }
        super::station_renderer::LabelPosition::Top => {
            // Station label is drawn at -45° rotation
            // Text starts at (offset*cos45, -offset*cos45) in rotated space
            // and extends rightward by station_name_width in rotated space
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            let base_offset = station_node_radius + station_label_offset; // 20.0
            let angle = -std::f64::consts::PI / 4.0; // -45°

            // Start position in rotated space
            let x_start_rotated = base_offset * cos45;
            let y_start_rotated = -base_offset * cos45;

            // Transform start to world space
            let world_start_x = x_start_rotated * angle.cos() - y_start_rotated * angle.sin();
            let world_start_y = x_start_rotated * angle.sin() + y_start_rotated * angle.cos();

            // End of station text in world space
            // (text extends along rotated x-axis direction)
            let text_end_x = adjusted_pos.0 + world_start_x + station_name_width * angle.cos();
            let text_end_y = adjusted_pos.1 + world_start_y + station_name_width * angle.sin();

            // Continue extending in the same diagonal direction
            // Add extra spacing for visual clearance
            let spacing_with_index = LINE_LABEL_SPACING + 6.0 + (label_index as f64 * (stack_width + LINE_LABEL_SPACING));
            (
                text_end_x + spacing_with_index * angle.cos(),
                text_end_y + spacing_with_index * angle.sin() - rect_height / 2.0,
            )
        }
        super::station_renderer::LabelPosition::Bottom => {
            // Station label is drawn at +45° rotation
            // Text starts at (offset*cos45, offset*cos45) in rotated space
            // and extends rightward by station_name_width in rotated space
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            let base_offset = station_node_radius + station_label_offset; // 20.0
            let angle = std::f64::consts::PI / 4.0; // +45°

            // Start position in rotated space
            let x_start_rotated = base_offset * cos45;
            let y_start_rotated = base_offset * cos45;

            // Transform start to world space
            let world_start_x = x_start_rotated * angle.cos() - y_start_rotated * angle.sin();
            let world_start_y = x_start_rotated * angle.sin() + y_start_rotated * angle.cos();

            // End of station text in world space
            // (text extends along rotated x-axis direction)
            let text_end_x = adjusted_pos.0 + world_start_x + station_name_width * angle.cos();
            let text_end_y = adjusted_pos.1 + world_start_y + station_name_width * angle.sin();

            // Continue extending in the same diagonal direction
            // Add extra spacing for visual clearance
            let spacing_with_index = LINE_LABEL_SPACING + 6.0 + (label_index as f64 * (stack_width + LINE_LABEL_SPACING));
            (
                text_end_x + spacing_with_index * angle.cos(),
                text_end_y + spacing_with_index * angle.sin() - rect_height / 2.0,
            )
        }
        super::station_renderer::LabelPosition::TopRight => {
            // Station label is at diagonal position (offset * 0.707, offset * 0.707)
            // Text extends horizontally rightward by station_name_width
            // Line labels continue extending diagonally, but account for horizontal text extent
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            let diagonal_offset = (station_node_radius + station_label_offset) * cos45;
            let horizontal_offset = station_name_width + LINE_LABEL_SPACING;
            let stack_offset = label_index as f64 * (stack_width + LINE_LABEL_SPACING) * cos45;
            (
                adjusted_pos.0 + diagonal_offset + horizontal_offset + stack_offset,
                adjusted_pos.1 - diagonal_offset - LINE_LABEL_SPACING - rect_height - stack_offset,
            )
        }
        super::station_renderer::LabelPosition::TopLeft => {
            // Station label is at diagonal position (-offset * 0.707, -offset * 0.707)
            // Text extends horizontally rightward by station_name_width (but from left position)
            // Line labels continue extending diagonally to top-left
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            let diagonal_offset = (station_node_radius + station_label_offset) * cos45;
            let stack_offset = label_index as f64 * (stack_width + LINE_LABEL_SPACING) * cos45;
            (
                adjusted_pos.0 - diagonal_offset - LINE_LABEL_SPACING - rect_width - stack_offset,
                adjusted_pos.1 - diagonal_offset - LINE_LABEL_SPACING - rect_height - stack_offset,
            )
        }
        super::station_renderer::LabelPosition::BottomRight => {
            // Station label is at diagonal position (offset * 0.707, offset * 0.707)
            // Text extends horizontally rightward by station_name_width
            // Line labels continue extending diagonally to bottom-right
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            let diagonal_offset = (station_node_radius + station_label_offset) * cos45;
            let horizontal_offset = station_name_width + LINE_LABEL_SPACING;
            let stack_offset = label_index as f64 * (stack_width + LINE_LABEL_SPACING) * cos45;
            (
                adjusted_pos.0 + diagonal_offset + horizontal_offset + stack_offset,
                adjusted_pos.1 + diagonal_offset + LINE_LABEL_SPACING + stack_offset,
            )
        }
        super::station_renderer::LabelPosition::BottomLeft => {
            // Station label is at diagonal position (-offset * 0.707, offset * 0.707)
            // Text extends horizontally rightward by station_name_width (but from left position)
            // Line labels continue extending diagonally to bottom-left
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;
            let diagonal_offset = (station_node_radius + station_label_offset) * cos45;
            let stack_offset = label_index as f64 * (stack_width + LINE_LABEL_SPACING) * cos45;
            (
                adjusted_pos.0 - diagonal_offset - LINE_LABEL_SPACING - rect_width - stack_offset,
                adjusted_pos.1 + diagonal_offset + LINE_LABEL_SPACING + stack_offset,
            )
        }
    };

    // Draw rectangle background
    ctx.save();
    ctx.set_fill_style_str(line_color);
    ctx.fill_rect(label_x, label_y, rect_width, rect_height);

    // Draw border (slightly darker/lighter than fill)
    ctx.set_stroke_style_str("#00000040");
    ctx.set_line_width(1.0 / zoom);
    ctx.stroke_rect(label_x, label_y, rect_width, rect_height);

    // Draw text centered both horizontally and vertically
    let text_color = calculate_readable_text_color(line_color);
    ctx.set_fill_style_str(text_color);
    ctx.set_font(&format!("{font_size}px sans-serif"));
    ctx.set_text_align("center");
    ctx.set_text_baseline("alphabetic");

    let text_x = label_x + rect_width / 2.0;
    // Position text vertically: use alphabetic baseline and offset by approximate text center
    // For better visual centering, position at rect center plus ~35% of font size
    let text_y = label_y + rect_height / 2.0 + font_size * 0.35;
    let _ = ctx.fill_text(line_name, text_x, text_y);

    ctx.restore();
}

/// Calculate the angle of lines passing through a station
#[allow(clippy::cast_precision_loss)]
fn calculate_line_angle(station_idx: NodeIndex, graph: &RailwayGraph) -> f64 {
    use petgraph::visit::EdgeRef;

    // Get connected edges
    let mut angles = Vec::new();
    for edge in graph.graph.edges(station_idx) {
        let (source, target) = (edge.source(), edge.target());
        let other_node = if source == station_idx {
            target
        } else {
            source
        };

        if let (Some(station_pos), Some(other_pos)) = (
            graph.get_station_position(station_idx),
            graph.get_station_position(other_node),
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
    visual_positions_map: &HashMap<EdgeIndex, (Vec<&Line>, HashMap<uuid::Uuid, usize>)>,
    graph: &RailwayGraph,
    zoom: f64,
    palette: &Palette,
) {
    // Calculate rotation based on line direction through station
    let angle = calculate_line_angle(station_idx, graph);

    // Calculate perpendicular direction for the pill (perpendicular to average line angle)
    let pill_perp_x = -angle.sin();
    let pill_perp_y = angle.cos();

    // Collect actual world-space positions for each stopping line
    let mut line_positions: Vec<(f64, f64)> = Vec::new();

    for line in station_lines {
        // Get the actual offset for this line using the same logic as ticks
        let (ox, oy) = calculate_line_offset_at_station(station_idx, line, visual_positions_map, graph, zoom)
            .unwrap_or((0.0, 0.0)); // Fallback to center if calculation fails

        // Store the actual world position
        line_positions.push((pos.0 + ox, pos.1 + oy));
    }

    if line_positions.is_empty() {
        return; // No valid positions found
    }

    // Project all positions onto the pill's perpendicular direction to find extent
    let mut perpendicular_offsets: Vec<f64> = Vec::new();
    for (px, py) in &line_positions {
        // Project relative to station position
        let rel_x = px - pos.0;
        let rel_y = py - pos.1;
        let projected = rel_x * pill_perp_x + rel_y * pill_perp_y;
        perpendicular_offsets.push(projected);
    }

    // Find min and max perpendicular offsets
    let min_offset = perpendicular_offsets.iter().copied().fold(f64::INFINITY, f64::min);
    let max_offset = perpendicular_offsets.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    // Add extra margin to ensure full coverage (account for line widths)
    let line_width_margin = (LINE_BASE_WIDTH + 2.0) / zoom;

    // Calculate pill dimensions with margins
    // The span across lines (perpendicular to line direction) becomes the HEIGHT after rotation
    // The thin dimension (along line direction) becomes the WIDTH after rotation
    let pill_span = (max_offset - min_offset) + (line_width_margin * 2.0) + (PILL_PADDING * 2.0);
    let pill_center_offset = (min_offset + max_offset) / 2.0;

    // Swap dimensions: width is thin (along line), height is span (across lines)
    let pill_width = PILL_HEIGHT;
    let pill_height = pill_span;

    // Draw pill centered at station position, offset to align with actual line positions
    ctx.save();
    let _ = ctx.translate(pos.0, pos.1);
    let _ = ctx.rotate(angle);

    // Apply center offset to align pill with where lines are actually drawn
    // Offset is in the y-direction (perpendicular to line after rotation)
    let _ = ctx.translate(0.0, pill_center_offset);

    // Draw pill background
    ctx.set_fill_style_str(palette.pill_fill);
    ctx.set_stroke_style_str(palette.pill_border);
    ctx.set_line_width(PILL_BORDER_WIDTH / zoom);

    let half_width = pill_width / 2.0;
    let half_height = pill_height / 2.0;
    let radius = half_width;

    // Draw rounded rectangle (vertical pill with rounded top and bottom caps)
    ctx.begin_path();
    // Start at left side, near top
    ctx.move_to(-half_width, -half_height + radius);
    // Line down left side to near bottom
    ctx.line_to(-half_width, half_height - radius);
    // Arc around bottom cap from left to right (counterclockwise: PI -> PI/2 -> 0)
    let _ = ctx.arc_with_anticlockwise(
        0.0,
        half_height - radius,
        radius,
        std::f64::consts::PI,
        0.0,
        true, // counterclockwise
    );
    // Line up right side to near top
    ctx.line_to(half_width, -half_height + radius);
    // Arc around top cap from right to left (counterclockwise: 0 -> -PI/2 -> -PI)
    let _ = ctx.arc_with_anticlockwise(
        0.0,
        -half_height + radius,
        radius,
        0.0,
        -std::f64::consts::PI,
        true, // counterclockwise
    );
    ctx.close_path();
    ctx.fill();
    ctx.stroke();

    ctx.restore();
}

/// Calculate line offset at a station for positioning ticks
/// Returns (`perpendicular_offset_x`, `perpendicular_offset_y`) for a line at a station
/// This must match the positioning logic from `line_renderer`'s section-based visual positions
#[allow(clippy::cast_precision_loss)]
fn calculate_line_offset_at_station(
    station_idx: NodeIndex,
    line: &Line,
    visual_positions_map: &HashMap<EdgeIndex, (Vec<&Line>, HashMap<uuid::Uuid, usize>)>,
    graph: &RailwayGraph,
    zoom: f64,
) -> Option<(f64, f64)> {
    use petgraph::visit::{EdgeRef, IntoEdgeReferences};

    // Find an edge connected to this station that this line uses
    // IMPORTANT: We must check ALL edges (both incoming and outgoing) since direction is irrelevant
    let mut edge_info: Option<(EdgeIndex, NodeIndex, NodeIndex)> = None;

    // Check all edges in the graph that connect to this station
    for edge_ref in graph.graph.edge_references() {
        let edge_idx = edge_ref.id();
        let (source, target) = (edge_ref.source(), edge_ref.target());

        // Skip edges that don't connect to this station
        if source != station_idx && target != station_idx {
            continue;
        }

        // Check if this line uses this edge AND we have visual position data for it
        let line_uses_edge = line
            .forward_route
            .iter()
            .any(|seg| EdgeIndex::new(seg.edge_index) == edge_idx);

        if !line_uses_edge {
            continue;
        }

        let has_visual_data = visual_positions_map.contains_key(&edge_idx);

        if has_visual_data {
            edge_info = Some((edge_idx, source, target));
            break;
        }
    }

    let (edge_idx, source, target) = edge_info?;

    // Get edge position data
    let (section_ordering, visual_pos_map) = visual_positions_map.get(&edge_idx)?;
    let visual_pos = visual_pos_map.get(&line.id)?;

    // Get positions
    let pos1 = graph.get_station_position(source)?;
    let pos2 = graph.get_station_position(target)?;

    // Calculate edge direction and perpendicular
    let dx = pos2.0 - pos1.0;
    let dy = pos2.1 - pos1.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.1 {
        return None;
    }

    // Perpendicular vector (pointing "left" of direction)
    let nx = -dy / len;
    let ny = dx / len;

    // Calculate widths and offsets (matching line_renderer logic exactly)
    let gap_width = (LINE_BASE_WIDTH + 2.0) / zoom;
    let section_line_widths: Vec<f64> = section_ordering
        .iter()
        .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
        .collect();

    let num_gaps = section_ordering.len().saturating_sub(1);
    let total_width: f64 = section_line_widths.iter().sum::<f64>() + (num_gaps as f64) * gap_width;

    // Calculate offset for this specific line using its visual position
    let start_offset = -total_width / 2.0;
    let offset_sum: f64 = section_line_widths
        .iter()
        .take(*visual_pos)
        .map(|&width| width + gap_width)
        .sum();
    let line_width = section_line_widths
        .get(*visual_pos)
        .copied()
        .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
    let offset = start_offset + offset_sum + line_width / 2.0;

    // Apply perpendicular offset
    Some((nx * offset, ny * offset))
}

/// Draw station markers and labels for line mode
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
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

    // Compute section information and visual positions (same logic as line_renderer)
    // This is needed to correctly position ticks on their respective lines
    let junctions: HashSet<NodeIndex> = graph
        .graph
        .node_indices()
        .filter(|&idx| graph.is_junction(idx))
        .collect();

    let sections = identify_sections(graph, &junctions);
    let section_lines = get_lines_in_section(&sections, lines);

    // Build edge_to_lines map
    let mut edge_to_lines: HashMap<EdgeIndex, Vec<&Line>> = HashMap::new();
    for line in lines {
        if !line.visible {
            continue;
        }
        for segment in &line.forward_route {
            let edge_idx = EdgeIndex::new(segment.edge_index);
            edge_to_lines.entry(edge_idx).or_default().push(line);
        }
    }

    // Compute section orderings and visual positions
    let mut section_orderings: HashMap<super::line_renderer::SectionId, Vec<&Line>> =
        HashMap::new();
    for section in &sections {
        if let Some(lines_in_section) = section_lines.get(&section.id) {
            let ordered = order_lines_for_section(lines_in_section, &section.edges);
            section_orderings.insert(section.id, ordered);
        }
    }

    let mut section_visual_positions: HashMap<
        super::line_renderer::SectionId,
        HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>>,
    > = HashMap::new();
    for section in &sections {
        if let Some(ordering) = section_orderings.get(&section.id) {
            let visual_positions =
                assign_visual_positions_with_reuse(section, ordering, &edge_to_lines, graph);
            section_visual_positions.insert(section.id, visual_positions);
        }
    }

    // Build a map from edge to (section_ordering, visual_positions) for quick lookup
    let mut edge_to_section: HashMap<EdgeIndex, super::line_renderer::SectionId> = HashMap::new();
    for section in &sections {
        for &edge_idx in &section.edges {
            edge_to_section.insert(edge_idx, section.id);
        }
    }

    let mut visual_positions_map: HashMap<EdgeIndex, (Vec<&Line>, HashMap<uuid::Uuid, usize>)> =
        HashMap::new();
    for (edge_idx, section_id) in &edge_to_section {
        if let Some(ordering) = section_orderings.get(section_id) {
            if let Some(section_vp) = section_visual_positions.get(section_id) {
                if let Some(edge_vp) = section_vp.get(edge_idx) {
                    visual_positions_map.insert(*edge_idx, (ordering.clone(), edge_vp.clone()));
                }
            }
        }
    }

    // Draw stations
    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else {
            continue;
        };
        let Some(node) = graph.graph.node_weight(idx) else {
            continue;
        };

        // Skip junctions
        if node.as_junction().is_some() {
            continue;
        }

        // Viewport culling
        if pos.0 < left - margin
            || pos.0 > right + margin
            || pos.1 < top - margin
            || pos.1 > bottom + margin
        {
            continue;
        }

        // Get all lines passing through this station
        let all_lines = get_lines_through_station(idx, lines, graph);
        let stopping_lines = get_stopping_lines_at_station(idx, lines, graph);

        if stopping_lines.is_empty() {
            continue; // No stopping lines, nothing to draw
        }

        // Get label position (from cache or manual override)
        let label_position = if let Some((_, cache)) = label_cache {
            cache.get(&idx).map(|c| c.position)
        } else {
            None
        }
        .or_else(|| {
            // Check for manual override
            if let Some(station) = node.as_station() {
                station.label_position
            } else {
                None
            }
        })
        .unwrap_or(LabelPosition::Right);

        // Determine marker type:
        // - If ALL lines stop and there are 2+ lines: draw interchange pill
        // - Otherwise: draw individual ticks for each stopping line
        let is_interchange = stopping_lines.len() == all_lines.len() && stopping_lines.len() >= 2;

        if is_interchange {
            draw_multi_line_pill(
                ctx,
                pos,
                idx,
                &stopping_lines,
                &visual_positions_map,
                graph,
                zoom,
                palette,
            );
        } else {
            // Draw individual ticks for each stopping line at their actual line positions
            for line in &stopping_lines {
                // Calculate where this line is actually drawn using section visual positions
                let offset =
                    calculate_line_offset_at_station(idx, line, &visual_positions_map, graph, zoom);
                let tick_pos = if let Some((ox, oy)) = offset {
                    (pos.0 + ox, pos.1 + oy)
                } else {
                    pos // Fallback to center if calculation fails
                };

                draw_single_line_tick(ctx, tick_pos, &line.color, label_position, zoom);
            }
        }

        // Draw line name labels for terminal stations
        let terminal_lines: Vec<&Line> = all_lines
            .iter()
            .filter(|line| is_line_terminal(idx, line, graph))
            .copied()
            .collect();

        // Calculate line extent for label positioning (same logic as station_renderer)
        let line_extent = if all_lines.is_empty() {
            None
        } else {
            // Calculate the angle of lines through this station
            let angle = calculate_line_angle(idx, graph);

            // Calculate perpendicular direction
            let perp_x = -angle.sin();
            let perp_y = angle.cos();

            // Collect perpendicular offsets for all lines
            let mut perpendicular_offsets = Vec::new();
            for line in &all_lines {
                if let Some((ox, oy)) = calculate_line_offset_at_station(idx, line, &visual_positions_map, graph, zoom) {
                    let projected = ox * perp_x + oy * perp_y;
                    perpendicular_offsets.push(projected);
                }
            }

            if perpendicular_offsets.is_empty() {
                None
            } else {
                let min_offset = perpendicular_offsets.iter().copied().fold(f64::INFINITY, f64::min);
                let max_offset = perpendicular_offsets.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                Some((angle, min_offset, max_offset))
            }
        };

        for (line_idx, line) in terminal_lines.iter().enumerate() {
            draw_line_name_label(
                ctx,
                &line.name,
                &line.color,
                pos,
                label_position,
                &node.display_name(),
                zoom,
                line_idx,
                line_extent,
            );
        }
    }

    // Draw labels using existing label rendering from station_renderer
    // Labels are already drawn by draw_cached_labels in station_renderer
}
