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
    }

    // Draw labels using existing label rendering from station_renderer
    // Labels are already drawn by draw_cached_labels in station_renderer
}
