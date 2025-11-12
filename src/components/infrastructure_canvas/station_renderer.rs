use crate::models::{RailwayGraph, Stations, Junctions, Line};
use crate::theme::Theme;
use crate::components::infrastructure_canvas::{track_renderer, junction_renderer, line_renderer, line_station_renderer};
use crate::geometry::line_segments_intersect;
use web_sys::CanvasRenderingContext2d;
use std::collections::{HashMap, HashSet};
use indexmap::IndexMap;
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

type TrackSegment = ((f64, f64), (f64, f64));

const NODE_RADIUS: f64 = 8.0;
const LABEL_OFFSET: f64 = 12.0;
const JUNCTION_LABEL_OFFSET: f64 = 12.0;
const CHAR_WIDTH_ESTIMATE: f64 = 7.5;
const JUNCTION_LABEL_RADIUS: f64 = 22.0;
const SELECTION_RING_WIDTH: f64 = 3.0;
const SELECTION_RING_OFFSET: f64 = 4.0;

struct Palette {
    station: &'static str,
    passing_loop: &'static str,
    node_fill: &'static str,
    label: &'static str,
    selection_ring: &'static str,
}

const DARK_PALETTE: Palette = Palette {
    station: "#4a9eff",
    passing_loop: "#888",
    node_fill: "#2a2a2a",
    label: "#fff",
    selection_ring: "#ffaa00",
};

const LIGHT_PALETTE: Palette = Palette {
    station: "#1976d2",
    passing_loop: "#666",
    node_fill: "#f0f0f0",
    label: "#1a1a1a",
    selection_ring: "#ff8800",
};

fn get_palette(theme: Theme) -> &'static Palette {
    match theme {
        Theme::Dark => &DARK_PALETTE,
        Theme::Light => &LIGHT_PALETTE,
    }
}

/// Calculate readable text color (white or black) based on background color luminance
#[must_use]
pub fn calculate_readable_text_color(hex_color: &str) -> &'static str {
    let trimmed = hex_color.trim_start_matches('#');

    if trimmed.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&trimmed[0..2], 16),
            u8::from_str_radix(&trimmed[2..4], 16),
            u8::from_str_radix(&trimmed[4..6], 16),
        ) {
            // Normalize RGB to 0-1 range
            #[allow(clippy::cast_precision_loss)]
            let r_norm = f64::from(r) / 255.0;
            #[allow(clippy::cast_precision_loss)]
            let g_norm = f64::from(g) / 255.0;
            #[allow(clippy::cast_precision_loss)]
            let b_norm = f64::from(b) / 255.0;

            // Calculate relative luminance using sRGB coefficients
            let luminance = 0.2126 * r_norm + 0.7152 * g_norm + 0.0722 * b_norm;

            // Use white text for dark backgrounds, black text for light backgrounds
            return if luminance < 0.5 {
                "#ffffff"
            } else {
                "#000000"
            };
        }
    }

    // Fallback: use white text if parsing fails
    "#ffffff"
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum LabelPosition {
    Right,
    Left,
    Top,
    Bottom,
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[derive(Clone, Copy)]
pub struct CachedLabelPosition {
    pub position: LabelPosition,
    pub bounds: LabelBounds,
}

impl LabelPosition {
    fn all() -> Vec<LabelPosition> {
        vec![
            LabelPosition::Right,
            LabelPosition::Left,
            LabelPosition::Top,
            LabelPosition::Bottom,
            LabelPosition::TopRight,
            LabelPosition::TopLeft,
            LabelPosition::BottomRight,
            LabelPosition::BottomLeft,
        ]
    }

    fn calculate_label_pos_with_offset(self, node_pos: (f64, f64), text_width: f64, font_size: f64, offset: f64) -> (f64, f64) {
        let (x, y) = node_pos;
        match self {
            LabelPosition::Right => (x + offset, y + font_size / 3.0),
            LabelPosition::Left => (x - offset - text_width, y + font_size / 3.0),
            LabelPosition::Top => (x - text_width / 2.0, y - offset),
            LabelPosition::Bottom => (x - text_width / 2.0, y + offset + font_size),
            LabelPosition::TopRight => (x + offset * 0.7, y - offset * 0.7),
            LabelPosition::TopLeft => (x - offset * 0.7 - text_width, y - offset * 0.7),
            LabelPosition::BottomRight => (x + offset * 0.7, y + offset * 0.7 + font_size),
            LabelPosition::BottomLeft => (x - offset * 0.7 - text_width, y + offset * 0.7 + font_size),
        }
    }

    fn rotation_angle(self) -> f64 {
        match self {
            LabelPosition::Top | LabelPosition::TopRight | LabelPosition::BottomLeft => -std::f64::consts::PI / 4.0,
            LabelPosition::Bottom | LabelPosition::BottomRight | LabelPosition::TopLeft => std::f64::consts::PI / 4.0,
            _ => 0.0,
        }
    }

    fn is_diagonal(self) -> bool {
        matches!(self,
            LabelPosition::Top |
            LabelPosition::Bottom |
            LabelPosition::TopRight |
            LabelPosition::TopLeft |
            LabelPosition::BottomRight |
            LabelPosition::BottomLeft
        )
    }

    fn text_align(self) -> &'static str {
        match self {
            LabelPosition::Left | LabelPosition::TopLeft | LabelPosition::BottomLeft => "right",
            LabelPosition::Right | LabelPosition::TopRight | LabelPosition::BottomRight
                | LabelPosition::Top | LabelPosition::Bottom => "left",
        }
    }

    fn text_baseline() -> &'static str {
        "middle"
    }
}

#[derive(Clone, Copy)]
pub struct LabelBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LabelBounds {
    fn overlaps(&self, other: &LabelBounds) -> bool {
        !(self.x + self.width < other.x ||
          other.x + other.width < self.x ||
          self.y + self.height < other.y ||
          other.y + other.height < self.y)
    }

    fn overlaps_node(&self, node_pos: (f64, f64), radius: f64) -> bool {
        let closest_x = node_pos.0.max(self.x).min(self.x + self.width);
        let closest_y = node_pos.1.max(self.y).min(self.y + self.height);
        let dx = node_pos.0 - closest_x;
        let dy = node_pos.1 - closest_y;
        (dx * dx + dy * dy) < (radius * radius)
    }

    fn intersects_line(&self, p1: (f64, f64), p2: (f64, f64)) -> bool {
        // Check if line segment intersects with rectangle
        // First check if either endpoint is inside the rectangle
        if self.contains_point(p1) || self.contains_point(p2) {
            return true;
        }

        // Check if line intersects any of the four edges of the rectangle
        let corners = [
            (self.x, self.y),
            (self.x + self.width, self.y),
            (self.x + self.width, self.y + self.height),
            (self.x, self.y + self.height),
        ];

        for i in 0..4 {
            let c1 = corners[i];
            let c2 = corners[(i + 1) % 4];
            if line_segments_intersect(p1, p2, c1, c2) {
                return true;
            }
        }

        false
    }

    fn contains_point(&self, point: (f64, f64)) -> bool {
        point.0 >= self.x && point.0 <= self.x + self.width &&
        point.1 >= self.y && point.1 <= self.y + self.height
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_station_nodes(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
    selected_stations: &[NodeIndex],
    highlighted_edges: &std::collections::HashSet<petgraph::stable_graph::EdgeIndex>,
    viewport_bounds: (f64, f64, f64, f64),
    junctions: &HashSet<NodeIndex>,
    cached_avoidance: &HashMap<petgraph::stable_graph::EdgeIndex, (f64, f64)>,
    orphaned_tracks: &HashMap<(petgraph::stable_graph::EdgeIndex, NodeIndex), HashSet<usize>>,
    crossover_intersections: &HashMap<(petgraph::stable_graph::EdgeIndex, NodeIndex, usize), (f64, f64)>,
    show_lines: bool,
    palette: &Palette,
) -> Vec<(NodeIndex, (f64, f64), f64)> {
    let mut node_positions = Vec::new();
    let (left, top, right, bottom) = viewport_bounds;
    let margin = 100.0; // Buffer to include nodes slightly outside viewport

    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(node) = graph.graph.node_weight(idx) else { continue };

        // Viewport culling: skip nodes completely outside visible area
        if pos.0 < left - margin || pos.0 > right + margin ||
           pos.1 < top - margin || pos.1 > bottom + margin {
            continue;
        }

        if let Some(station) = node.as_station() {
            let (border_color, radius) = if station.passing_loop {
                (palette.passing_loop, NODE_RADIUS * 0.3)
            } else {
                (palette.station, NODE_RADIUS)
            };

            // Draw stations as circles (but not in line mode - custom markers are drawn separately)
            if !show_lines {
                ctx.set_fill_style_str(palette.node_fill);
                ctx.set_stroke_style_str(border_color);
                ctx.set_line_width(2.0 / zoom);
                ctx.begin_path();
                let _ = ctx.arc(pos.0, pos.1, radius, 0.0, std::f64::consts::PI * 2.0);
                ctx.fill();
                ctx.stroke();

                // Draw selection ring if this station is selected
                if selected_stations.contains(&idx) {
                    ctx.set_stroke_style_str(palette.selection_ring);
                    ctx.set_line_width(SELECTION_RING_WIDTH / zoom);
                    ctx.begin_path();
                    let _ = ctx.arc(pos.0, pos.1, radius + SELECTION_RING_OFFSET, 0.0, std::f64::consts::PI * 2.0);
                    ctx.stroke();
                }
            }

            node_positions.push((idx, pos, radius));
        } else if junctions.contains(&idx) {
            // Draw junction (but not in lines mode - lines are drawn separately)
            if !show_lines {
                junction_renderer::draw_junction(ctx, graph, idx, pos, zoom, highlighted_edges, cached_avoidance, orphaned_tracks, crossover_intersections, selected_stations);
            }
            // Use larger radius for label overlap to account for junction connection lines
            node_positions.push((idx, pos, JUNCTION_LABEL_RADIUS));
        }
    }

    node_positions
}

fn calculate_label_bounds(
    position: LabelPosition,
    pos: (f64, f64),
    text_width: f64,
    font_size: f64,
    offset: f64,
) -> LabelBounds {
    let label_pos = position.calculate_label_pos_with_offset(pos, text_width, font_size, offset);

    if position.is_diagonal() {
        let cos45 = std::f64::consts::FRAC_1_SQRT_2;
        let text_height = font_size * 1.2;
        let rotated_width = text_width * cos45 + text_height * cos45;
        let rotated_height = text_width * cos45 + text_height * cos45;

        let angle = position.rotation_angle();

        let (x_offset_rotated, y_offset_rotated) = match position {
            LabelPosition::Top => (offset * cos45, -offset * cos45),
            LabelPosition::Bottom => (offset * cos45, offset * cos45),
            LabelPosition::TopRight | LabelPosition::BottomRight => (offset, 0.0),
            LabelPosition::TopLeft | LabelPosition::BottomLeft => (-offset, 0.0),
            _ => (0.0, 0.0),
        };

        let world_x = x_offset_rotated * angle.cos() - y_offset_rotated * angle.sin();
        let world_y = x_offset_rotated * angle.sin() + y_offset_rotated * angle.cos();

        let center_x = pos.0 + world_x + (text_width / 2.0) * angle.cos();
        let center_y = pos.1 + world_y + (text_width / 2.0) * angle.sin();

        LabelBounds {
            x: center_x - rotated_width / 2.0,
            y: center_y - rotated_height / 2.0,
            width: rotated_width,
            height: rotated_height,
        }
    } else {
        LabelBounds {
            x: label_pos.0,
            y: label_pos.1 - font_size,
            width: text_width,
            height: font_size * 1.2,
        }
    }
}

fn count_label_overlaps(
    bounds: &LabelBounds,
    idx: NodeIndex,
    label_positions: &HashMap<NodeIndex, (LabelBounds, LabelPosition)>,
    node_positions: &[(NodeIndex, (f64, f64), f64)],
    track_segments: &[TrackSegment],
) -> usize {
    let mut overlaps = 0;

    for (other_bounds, _) in label_positions.values() {
        if bounds.overlaps(other_bounds) {
            overlaps += 1;
        }
    }

    for (other_idx, other_pos, other_radius) in node_positions {
        if *other_idx != idx && bounds.overlaps_node(*other_pos, *other_radius + 3.0) {
            overlaps += 1;
        }
    }

    for (p1, p2) in track_segments {
        if bounds.intersects_line(*p1, *p2) {
            overlaps += 1;
        }
    }

    overlaps
}

/// Adjust station position for label drawing based on line extent in line mode
fn adjust_position_for_line_extent(
    pos: (f64, f64),
    label_position: LabelPosition,
    extent: Option<(f64, f64, f64)>, // (angle, min_offset, max_offset)
) -> (f64, f64) {
    let Some((angle, min_offset, max_offset)) = extent else {
        return pos;
    };

    // Calculate perpendicular direction
    let perp_x = -angle.sin();
    let perp_y = angle.cos();

    // Determine which offset to use based on label direction
    let offset = match label_position {
        LabelPosition::Right | LabelPosition::TopRight | LabelPosition::BottomRight => {
            // Label on right side - use positive extent
            if max_offset > 0.0 { max_offset } else { 0.0 }
        }
        LabelPosition::Left | LabelPosition::TopLeft | LabelPosition::BottomLeft => {
            // Label on left side - use negative extent
            if min_offset < 0.0 { min_offset } else { 0.0 }
        }
        LabelPosition::Top => {
            // For top, use whichever extent is larger in magnitude
            if max_offset.abs() > min_offset.abs() { max_offset } else { min_offset }
        }
        LabelPosition::Bottom => {
            // For bottom, use whichever extent is larger in magnitude
            if max_offset.abs() > min_offset.abs() { max_offset } else { min_offset }
        }
    };

    // Apply the perpendicular offset
    (pos.0 + perp_x * offset, pos.1 + perp_y * offset)
}

fn draw_station_label(
    ctx: &CanvasRenderingContext2d,
    station_name: &str,
    pos: (f64, f64),
    position: LabelPosition,
    radius: f64,
    offset: f64,
    scale: f64,
) {
    ctx.save();
    ctx.set_text_align(position.text_align());
    ctx.set_text_baseline(LabelPosition::text_baseline());

    let total_offset = (radius + offset) * scale;

    if position.is_diagonal() {
        let _ = ctx.translate(pos.0, pos.1);
        let _ = ctx.rotate(position.rotation_angle());

        let cos45 = std::f64::consts::FRAC_1_SQRT_2;

        let (x_offset, y_offset) = match position {
            LabelPosition::Top => (total_offset * cos45, -total_offset * cos45),
            LabelPosition::Bottom => (total_offset * cos45, total_offset * cos45),
            LabelPosition::TopRight | LabelPosition::BottomRight => (total_offset, 0.0),
            LabelPosition::TopLeft | LabelPosition::BottomLeft => (-total_offset, 0.0),
            _ => (0.0, 0.0),
        };

        let _ = ctx.fill_text(station_name, x_offset, y_offset);
    } else {
        let (x, y) = pos;
        let (x_pos, y_pos) = match position {
            LabelPosition::Right => (x + total_offset, y),
            LabelPosition::Left => (x - total_offset, y),
            _ => (x, y),
        };
        let _ = ctx.fill_text(station_name, x_pos, y_pos);
    }

    ctx.restore();
}

fn get_node_positions_and_radii(graph: &RailwayGraph, junctions: &HashSet<NodeIndex>) -> Vec<(NodeIndex, (f64, f64), f64)> {
    let mut node_positions = Vec::new();

    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(node) = graph.graph.node_weight(idx) else { continue };

        if let Some(station) = node.as_station() {
            let radius = if station.passing_loop {
                NODE_RADIUS * 0.3
            } else {
                NODE_RADIUS
            };
            node_positions.push((idx, pos, radius));
        } else if junctions.contains(&idx) {
            node_positions.push((idx, pos, JUNCTION_LABEL_RADIUS));
        }
    }

    node_positions
}

fn get_conflicting_label_positions(
    node_idx: NodeIndex,
    adjacency: &HashMap<NodeIndex, Vec<(NodeIndex, EdgeIndex)>>,
    graph: &RailwayGraph,
    node_pos: (f64, f64),
) -> Vec<LabelPosition> {
    let mut conflicting_positions = Vec::new();

    // Use cached adjacency instead of graph traversal
    let Some(neighbors) = adjacency.get(&node_idx) else {
        return conflicting_positions;
    };

    for &(neighbor_idx, _) in neighbors {
        if let Some(neighbor_pos) = graph.get_station_position(neighbor_idx) {
            // Calculate angle from node to neighbor
            let dx = neighbor_pos.0 - node_pos.0;
            let dy = neighbor_pos.1 - node_pos.1;
            let angle = dy.atan2(dx).to_degrees();

            // Map angle to LabelPosition (with 22.5° tolerance on each side)
            let position = if (-22.5..22.5).contains(&angle) {
                LabelPosition::Right
            } else if (22.5..67.5).contains(&angle) {
                LabelPosition::BottomRight
            } else if (67.5..112.5).contains(&angle) {
                LabelPosition::Bottom
            } else if (112.5..157.5).contains(&angle) {
                LabelPosition::BottomLeft
            } else if !(-157.5..157.5).contains(&angle) {
                LabelPosition::Left
            } else if (-157.5..-112.5).contains(&angle) {
                LabelPosition::TopLeft
            } else if (-112.5..-67.5).contains(&angle) {
                LabelPosition::Top
            } else {
                LabelPosition::TopRight
            };

            conflicting_positions.push(position);
        }
    }

    conflicting_positions
}

fn identify_branches(
    adjacency: &HashMap<NodeIndex, Vec<(NodeIndex, EdgeIndex)>>,
    node_positions: &[(NodeIndex, (f64, f64), f64)]
) -> Vec<Vec<NodeIndex>> {
    use std::collections::HashSet;

    let mut visited_edges = HashSet::new();
    let mut branches = Vec::new();

    // Find all junction nodes (degree != 2) and endpoints (degree == 1)
    let mut junction_or_endpoint_nodes = Vec::new();
    for (idx, _, _) in node_positions {
        let degree = adjacency.get(idx).map_or(0, Vec::len);
        if degree != 2 {
            junction_or_endpoint_nodes.push(*idx);
        }
    }

    // For each junction/endpoint, trace paths to other junctions/endpoints
    for &start_node in &junction_or_endpoint_nodes {
        let Some(neighbors) = adjacency.get(&start_node) else { continue };

        for &(next_node, _) in neighbors {
            let edge_key = if start_node < next_node {
                (start_node, next_node)
            } else {
                (next_node, start_node)
            };

            if visited_edges.contains(&edge_key) {
                continue;
            }

            // Trace this branch
            let mut branch = vec![start_node];
            let mut current = next_node;
            let mut previous = start_node;

            visited_edges.insert(edge_key);

            loop {
                branch.push(current);

                let current_degree = adjacency.get(&current).map_or(0, Vec::len);

                // Stop if we hit a junction/endpoint
                if current_degree != 2 {
                    break;
                }

                // Continue along the linear path
                let Some(neighbors) = adjacency.get(&current) else { break };
                let next = neighbors.iter().map(|(n, _)| n).find(|&&n| n != previous);

                let Some(&next_node) = next else { break };

                let edge_key = if current < next_node {
                    (current, next_node)
                } else {
                    (next_node, current)
                };

                if visited_edges.contains(&edge_key) {
                    break;
                }

                visited_edges.insert(edge_key);
                previous = current;
                current = next_node;
            }

            if !branch.is_empty() {
                branches.push(branch);
            }
        }
    }

    // Handle any isolated nodes with no connections
    for (idx, _, _) in node_positions {
        if !adjacency.contains_key(idx) {
            // This node has no connections at all
            branches.push(vec![*idx]);
        }
    }

    // Handle any isolated linear segments (no junctions)
    for (idx, _, _) in node_positions {
        let Some(neighbors) = adjacency.get(idx) else { continue };

        for &(neighbor, _) in neighbors {
            let edge_key = if *idx < neighbor {
                (*idx, neighbor)
            } else {
                (neighbor, *idx)
            };

            if visited_edges.contains(&edge_key) {
                continue;
            }

            // Trace this isolated segment
            let mut branch = vec![*idx];
            let mut current = neighbor;
            let mut previous = *idx;

            visited_edges.insert(edge_key);

            loop {
                branch.push(current);

                let Some(neighbors) = adjacency.get(&current) else { break };
                let next = neighbors.iter().map(|(n, _)| n).find(|&&n| n != previous);

                let Some(&next_node) = next else { break };

                let edge_key = if current < next_node {
                    (current, next_node)
                } else {
                    (next_node, current)
                };

                if visited_edges.contains(&edge_key) {
                    break;
                }

                visited_edges.insert(edge_key);
                previous = current;
                current = next_node;
            }

            if !branch.is_empty() {
                branches.push(branch);
            }
        }
    }

    branches
}

fn process_node_group(
    nodes: &[NodeIndex],
    node_metadata: &HashMap<NodeIndex, (f64, f64, (f64, f64))>,
    node_positions: &[(NodeIndex, (f64, f64), f64)],
    track_segments: &[TrackSegment],
    font_size: f64,
    label_positions: &mut HashMap<NodeIndex, (LabelBounds, LabelPosition)>,
    graph: &RailwayGraph,
    adjacency: &HashMap<NodeIndex, Vec<(NodeIndex, EdgeIndex)>>,
) {
    if nodes.is_empty() {
        return;
    }

    // Build map of conflicting positions for each node
    let mut node_conflicts: HashMap<NodeIndex, Vec<LabelPosition>> = HashMap::new();
    for &node_idx in nodes {
        if let Some((_, _, pos)) = node_metadata.get(&node_idx) {
            node_conflicts.insert(node_idx, get_conflicting_label_positions(node_idx, adjacency, graph, *pos));
        }
    }

    // Try all orientations and find the one with minimum overlaps
    let mut best_orientation = LabelPosition::Right;
    let mut best_total_overlaps = usize::MAX;

    let mut best_conflict_count = usize::MAX;

    for orientation in LabelPosition::all() {
        let mut total_overlaps = 0;
        let mut conflict_count = 0;

        for &node_idx in nodes {
            if let Some((text_width, label_offset, pos)) = node_metadata.get(&node_idx) {
                let bounds = calculate_label_bounds(orientation, *pos, *text_width, font_size, *label_offset);
                let overlaps = count_label_overlaps(&bounds, node_idx, label_positions, node_positions, track_segments);

                let has_conflict = node_conflicts
                    .get(&node_idx)
                    .is_some_and(|conflicts| conflicts.contains(&orientation));

                if has_conflict {
                    conflict_count += 1;
                }

                total_overlaps += overlaps;
            }
        }

        // Prefer orientations with fewer track conflicts, then by overlaps
        let is_better = conflict_count < best_conflict_count
            || (conflict_count == best_conflict_count && total_overlaps < best_total_overlaps);

        if is_better {
            best_conflict_count = conflict_count;
            best_total_overlaps = total_overlaps;
            best_orientation = orientation;
            if conflict_count == 0 && total_overlaps == 0 {
                break;
            }
        }
    }

    // Apply this orientation to all nodes and track which ones still have conflicts
    let mut conflicting_nodes = Vec::new();

    for &node_idx in nodes {
        if let Some((text_width, label_offset, pos)) = node_metadata.get(&node_idx) {
            let bounds = calculate_label_bounds(best_orientation, *pos, *text_width, font_size, *label_offset);

            // Check if this placement has overlaps
            let overlaps = count_label_overlaps(&bounds, node_idx, label_positions, node_positions, track_segments);

            if overlaps == 0 {
                // No conflict, place it
                label_positions.insert(node_idx, (bounds, best_orientation));
            } else {
                // Has conflict, add to conflicting nodes
                conflicting_nodes.push(node_idx);
            }
        }
    }

    // Recursively process conflicting nodes, but only if we made progress
    // (i.e., some nodes were placed without conflicts)
    if !conflicting_nodes.is_empty() && conflicting_nodes.len() < nodes.len() {
        process_node_group(
            &conflicting_nodes,
            node_metadata,
            node_positions,
            track_segments,
            font_size,
            label_positions,
            graph,
            adjacency,
        );
    } else if !conflicting_nodes.is_empty() {
        // No progress made (all nodes still conflict), just place them with best orientation
        for &node_idx in &conflicting_nodes {
            if let Some((text_width, label_offset, pos)) = node_metadata.get(&node_idx) {
                let bounds = calculate_label_bounds(best_orientation, *pos, *text_width, font_size, *label_offset);
                label_positions.insert(node_idx, (bounds, best_orientation));
            }
        }
    }
}

#[must_use]
pub fn compute_label_positions(graph: &RailwayGraph, zoom: f64) -> HashMap<NodeIndex, (f64, f64, f64, f64)> {
    let font_size = 14.0 / zoom;
    let mut track_segments = track_renderer::get_track_segments(graph);
    track_segments.extend(junction_renderer::get_junction_segments(graph));

    // Build junctions set for this function (not using cache since this is standalone)
    let mut junctions = HashSet::new();
    for idx in graph.graph.node_indices() {
        if graph.is_junction(idx) {
            junctions.insert(idx);
        }
    }

    let node_positions = get_node_positions_and_radii(graph, &junctions);

    // Build node metadata (width, offset, position)
    let mut node_metadata: HashMap<NodeIndex, (f64, f64, (f64, f64))> = HashMap::new();
    for (idx, pos, _) in &node_positions {
        if let Some(node) = graph.graph.node_weight(*idx) {
            let name = node.display_name();
            #[allow(clippy::cast_precision_loss)]
            let text_width = name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;
            let is_junction = junctions.contains(idx);
            let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };
            node_metadata.insert(*idx, (text_width, label_offset, *pos));
        }
    }

    // Build adjacency map in the new format (with EdgeIndex)
    let mut adjacency: HashMap<NodeIndex, Vec<(NodeIndex, EdgeIndex)>> = HashMap::new();
    for edge in graph.graph.edge_references() {
        adjacency.entry(edge.source()).or_default().push((edge.target(), edge.id()));
        adjacency.entry(edge.target()).or_default().push((edge.source(), edge.id()));
    }

    // Identify branches using cached adjacency format
    let branches = identify_branches(&adjacency, &node_positions);

    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();

    // First, handle any nodes with manual label position overrides
    for (idx, pos, _) in &node_positions {
        if let Some(node) = graph.graph.node_weight(*idx) {
            let manual_position = match node {
                crate::models::Node::Station(station) => station.label_position,
                crate::models::Node::Junction(junction) => junction.label_position,
            };

            if let Some(position) = manual_position {
                if let Some((text_width, label_offset, _)) = node_metadata.get(idx) {
                    let bounds = calculate_label_bounds(position, *pos, *text_width, font_size, *label_offset);
                    label_positions.insert(*idx, (bounds, position));
                }
            }
        }
    }

    // First pass: process all stations (excluding junctions from branches and nodes with manual overrides)
    for branch_nodes in &branches {
        let station_only_nodes: Vec<NodeIndex> = branch_nodes.iter()
            .filter(|idx| !graph.is_junction(**idx) && !label_positions.contains_key(idx))
            .copied()
            .collect();

        if !station_only_nodes.is_empty() {
            process_node_group(
                &station_only_nodes,
                &node_metadata,
                &node_positions,
                &track_segments,
                font_size,
                &mut label_positions,
                graph,
                &adjacency,
            );
        }
    }

    // Second pass: process each junction individually to find best position (skip those with manual overrides)
    let junction_nodes: Vec<NodeIndex> = node_positions.iter()
        .filter(|(idx, _, _)| graph.is_junction(*idx) && !label_positions.contains_key(idx))
        .map(|(idx, _, _)| *idx)
        .collect();

    for junction_idx in junction_nodes {
        process_node_group(
            &[junction_idx],
            &node_metadata,
            &node_positions,
            &track_segments,
            font_size,
            &mut label_positions,
            graph,
            &adjacency,
        );
    }

    label_positions.into_iter()
        .map(|(idx, (bounds, _))| (idx, (bounds.x, bounds.y, bounds.width, bounds.height)))
        .collect()
}

/// Calculate the perpendicular extent of lines at each station for label positioning
/// Returns a map of `station_idx` -> (angle, `min_offset`, `max_offset`)
#[allow(clippy::cast_precision_loss)]
fn calculate_line_extents_at_stations(
    graph: &RailwayGraph,
    lines: &[Line],
    zoom: f64,
    junctions: &HashSet<NodeIndex>,
) -> HashMap<NodeIndex, (f64, f64, f64)> {
    use petgraph::visit::{EdgeRef, IntoEdgeReferences};

    const LINE_BASE_WIDTH: f64 = 3.0;

    // Compute section information (same as line_station_renderer)
    let sections = line_renderer::identify_sections(graph, junctions);
    let section_lines = line_renderer::get_lines_in_section(&sections, lines);

    // Build edge_to_lines map
    let mut edge_to_lines: IndexMap<EdgeIndex, Vec<&Line>> = IndexMap::new();
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
    let mut section_orderings: HashMap<line_renderer::SectionId, Vec<&Line>> = HashMap::new();
    for section in &sections {
        if let Some(lines_in_section) = section_lines.get(&section.id) {
            let ordered = line_renderer::order_lines_for_section(lines_in_section, &section.edges);
            section_orderings.insert(section.id, ordered);
        }
    }

    let mut section_visual_positions: HashMap<
        line_renderer::SectionId,
        HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>>,
    > = HashMap::new();
    for section in &sections {
        if let Some(ordering) = section_orderings.get(&section.id) {
            let visual_positions =
                line_renderer::assign_visual_positions_with_reuse(section, ordering, &edge_to_lines, graph);
            section_visual_positions.insert(section.id, visual_positions);
        }
    }

    // Build visual_positions_map
    let mut edge_to_section: HashMap<EdgeIndex, line_renderer::SectionId> = HashMap::new();
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

    // Now calculate extents for each station
    let mut extents: HashMap<NodeIndex, (f64, f64, f64)> = HashMap::new();

    for station_idx in graph.graph.node_indices() {
        if junctions.contains(&station_idx) {
            continue; // Skip junctions
        }

        let Some(station_pos) = graph.get_station_position(station_idx) else { continue };

        // Calculate angle of lines through station
        let mut angles = Vec::new();
        for edge in graph.graph.edges(station_idx) {
            let (source, target) = (edge.source(), edge.target());
            let other_node = if source == station_idx { target } else { source };

            if let Some(other_pos) = graph.get_station_position(other_node) {
                let dx = other_pos.0 - station_pos.0;
                let dy = other_pos.1 - station_pos.1;
                let angle = dy.atan2(dx);
                angles.push(angle);
            }
        }

        if angles.is_empty() {
            continue;
        }

        let avg_angle = angles.iter().sum::<f64>() / angles.len() as f64;

        // Calculate perpendicular direction
        let perp_x = -avg_angle.sin();
        let perp_y = avg_angle.cos();

        // Collect line positions and project onto perpendicular
        let mut perpendicular_offsets: Vec<f64> = Vec::new();

        for edge_ref in graph.graph.edge_references() {
            let edge_idx = edge_ref.id();
            let (source, target) = (edge_ref.source(), edge_ref.target());

            if source != station_idx && target != station_idx {
                continue;
            }

            let Some((section_ordering, visual_pos_map)) = visual_positions_map.get(&edge_idx) else {
                continue;
            };

            let Some(pos1) = graph.get_station_position(source) else {
                continue;
            };
            let Some(pos2) = graph.get_station_position(target) else {
                continue;
            };

            let dx = pos2.0 - pos1.0;
            let dy = pos2.1 - pos1.1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.1 {
                continue;
            }

            let nx = -dy / len;
            let ny = dx / len;

            let gap_width = (LINE_BASE_WIDTH + 2.0) / zoom;
            let section_line_widths: Vec<f64> = section_ordering
                .iter()
                .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
                .collect();

            let num_gaps = section_ordering.len().saturating_sub(1);
            let total_width: f64 = section_line_widths.iter().sum::<f64>() + (num_gaps as f64) * gap_width;

            for line in section_ordering {
                if let Some(visual_pos) = visual_pos_map.get(&line.id) {
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

                    let ox = nx * offset;
                    let oy = ny * offset;

                    let line_pos = (station_pos.0 + ox, station_pos.1 + oy);
                    let rel_x = line_pos.0 - station_pos.0;
                    let rel_y = line_pos.1 - station_pos.1;
                    let projected = rel_x * perp_x + rel_y * perp_y;
                    perpendicular_offsets.push(projected);
                }
            }
        }

        if !perpendicular_offsets.is_empty() {
            let min_offset = perpendicular_offsets.iter().copied().fold(f64::INFINITY, f64::min);
            let max_offset = perpendicular_offsets.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            extents.insert(station_idx, (avg_angle, min_offset, max_offset));
        }
    }

    extents
}

/// Draw stations with cached label positions for performance during zoom
#[allow(clippy::cast_precision_loss, clippy::too_many_arguments)]
pub fn draw_stations_with_cache(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    lines: &[crate::models::Line],
    zoom: f64,
    selected_stations: &[NodeIndex],
    highlighted_edges: &std::collections::HashSet<petgraph::stable_graph::EdgeIndex>,
    cache: &mut super::renderer::TopologyCache,
    is_zooming: bool,
    viewport_bounds: (f64, f64, f64, f64),
    show_lines: bool,
    theme: Theme,
) {
    let palette = get_palette(theme);
    let font_size = 14.0 / zoom;

    let node_positions = draw_station_nodes(ctx, graph, zoom, selected_stations, highlighted_edges, viewport_bounds, &cache.junctions, &cache.avoidance_offsets, &cache.orphaned_tracks, &cache.crossover_intersections, show_lines, palette);

    // Calculate line extents in line mode for label positioning
    let line_extents = if show_lines {
        calculate_line_extents_at_stations(graph, lines, zoom, &cache.junctions)
    } else {
        HashMap::new()
    };

    // Check if we can use cached label positions
    let use_cache = if let Some((cached_zoom, cached_positions)) = &cache.label_cache {
        // Use cache if zooming and zoom hasn't changed drastically (>50%)
        if is_zooming && (cached_zoom - zoom).abs() / cached_zoom < 0.5 {
            // Only use cache if all visible nodes have cached positions
            // (viewport may have expanded, revealing new nodes)
            node_positions.iter().all(|(idx, _, _)| cached_positions.contains_key(idx))
        } else {
            false
        }
    } else {
        false
    };

    if use_cache {
        // Use cached positions
        if let Some((_, cached_positions)) = &cache.label_cache {
            draw_cached_labels(ctx, graph, lines, &node_positions, cached_positions, font_size, &cache.junctions, show_lines, &line_extents, palette);
        }
        return;
    }

    // Full recomputation - compute optimal label positions
    // Reuse cached edge segments and add junction segments
    let mut track_segments: Vec<((f64, f64), (f64, f64))> = cache.edge_segments.values()
        .flat_map(|segs| segs.iter().copied())
        .collect();
    track_segments.extend(junction_renderer::get_junction_segments(graph));

    // Build node metadata using cached junction set
    let mut node_metadata: HashMap<NodeIndex, (f64, f64, (f64, f64))> = HashMap::new();
    for (idx, pos, _) in &node_positions {
        if let Some(node) = graph.graph.node_weight(*idx) {
            let name = node.display_name();
            #[allow(clippy::cast_precision_loss)]
            let text_width = name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;
            let is_junction = cache.junctions.contains(idx);
            let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };
            node_metadata.insert(*idx, (text_width, label_offset, *pos));
        }
    }

    // Compute optimal label positions using cached adjacency
    let branches = identify_branches(&cache.adjacency, &node_positions);
    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();

    // First, handle any nodes with manual label position overrides
    for (idx, pos, _) in &node_positions {
        if let Some(node) = graph.graph.node_weight(*idx) {
            let manual_position = match node {
                crate::models::Node::Station(station) => station.label_position,
                crate::models::Node::Junction(junction) => junction.label_position,
            };

            if let Some(position) = manual_position {
                if let Some((text_width, label_offset, _)) = node_metadata.get(idx) {
                    let bounds = calculate_label_bounds(position, *pos, *text_width, font_size, *label_offset);
                    label_positions.insert(*idx, (bounds, position));
                }
            }
        }
    }

    for branch_nodes in &branches {
        let station_only_nodes: Vec<NodeIndex> = branch_nodes.iter()
            .filter(|idx| !cache.junctions.contains(idx) && !label_positions.contains_key(idx))
            .copied()
            .collect();

        if !station_only_nodes.is_empty() {
            process_node_group(
                &station_only_nodes,
                &node_metadata,
                &node_positions,
                &track_segments,
                font_size,
                &mut label_positions,
                graph,
                &cache.adjacency,
            );
        }
    }

    let junction_nodes: Vec<NodeIndex> = node_positions.iter()
        .filter(|(idx, _, _)| cache.junctions.contains(idx) && !label_positions.contains_key(idx))
        .map(|(idx, _, _)| *idx)
        .collect();

    for junction_idx in junction_nodes {
        process_node_group(
            &[junction_idx],
            &node_metadata,
            &node_positions,
            &track_segments,
            font_size,
            &mut label_positions,
            graph,
            &cache.adjacency,
        );
    }

    // Update cache with computed positions
    let cached_positions: HashMap<NodeIndex, CachedLabelPosition> = label_positions.iter()
        .map(|(idx, (bounds, position))| (*idx, CachedLabelPosition { position: *position, bounds: *bounds }))
        .collect();
    cache.label_cache = Some((zoom, cached_positions));

    // Draw labels using computed positions
    ctx.set_fill_style_str(palette.label);
    ctx.set_font(&format!("{font_size}px sans-serif"));

    for (idx, pos, radius) in &node_positions {
        let Some(node) = graph.graph.node_weight(*idx) else { continue };
        let Some((_, position)) = label_positions.get(idx) else { continue };
        let is_junction = cache.junctions.contains(idx);

        // Skip junction labels
        if is_junction {
            continue;
        }

        // Check if this is a passing loop for scaled rendering
        let is_passing_loop = node.as_station().is_some_and(|s| s.passing_loop);
        let label_scale = if is_passing_loop { 0.7 } else { 1.0 };

        // Skip passing loop labels in line view mode
        if show_lines && is_passing_loop {
            continue;
        }

        // Skip stations with no lines going through them in line mode
        if show_lines {
            let lines_through = line_station_renderer::get_lines_through_station(*idx, lines, graph);
            if lines_through.is_empty() {
                continue;
            }
        }

        let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };

        // Adjust position for line extent in line mode
        let adjusted_pos = if show_lines {
            adjust_position_for_line_extent(*pos, *position, line_extents.get(idx).copied())
        } else {
            *pos
        };

        // Save and restore context for scaled text
        if label_scale == 1.0 {
            draw_station_label(ctx, &node.display_name(), adjusted_pos, *position, *radius, label_offset, label_scale);
        } else {
            ctx.save();
            let scaled_font_size = font_size * label_scale;
            // Use muted color for passing loops
            ctx.set_fill_style_str(palette.passing_loop);
            ctx.set_font(&format!("{scaled_font_size}px sans-serif"));
            draw_station_label(ctx, &node.display_name(), adjusted_pos, *position, *radius, label_offset, label_scale);
            ctx.restore();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_cached_labels(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    lines: &[Line],
    node_positions: &[(NodeIndex, (f64, f64), f64)],
    cached_positions: &HashMap<NodeIndex, CachedLabelPosition>,
    font_size: f64,
    junctions: &HashSet<NodeIndex>,
    show_lines: bool,
    line_extents: &HashMap<NodeIndex, (f64, f64, f64)>,
    palette: &Palette,
) {
    ctx.set_fill_style_str(palette.label);
    ctx.set_font(&format!("{font_size}px sans-serif"));

    for (idx, pos, radius) in node_positions {
        let Some(node) = graph.graph.node_weight(*idx) else { continue };
        let Some(cached) = cached_positions.get(idx) else { continue };

        let is_junction = junctions.contains(idx);

        // Skip junction labels
        if is_junction {
            continue;
        }

        // Check if this is a passing loop for scaled rendering
        let is_passing_loop = node.as_station().is_some_and(|s| s.passing_loop);
        let label_scale = if is_passing_loop { 0.7 } else { 1.0 };

        // Skip passing loop labels in line view mode
        if show_lines && is_passing_loop {
            continue;
        }

        // Skip stations with no lines going through them in line mode
        if show_lines {
            let lines_through = line_station_renderer::get_lines_through_station(*idx, lines, graph);
            if lines_through.is_empty() {
                continue;
            }
        }

        let label_offset = if is_junction {
            JUNCTION_LABEL_OFFSET
        } else {
            LABEL_OFFSET
        };

        // Adjust position for line extent in line mode
        let adjusted_pos = if show_lines {
            adjust_position_for_line_extent(*pos, cached.position, line_extents.get(idx).copied())
        } else {
            *pos
        };

        // Save and restore context for scaled text
        if label_scale == 1.0 {
            draw_station_label(ctx, &node.display_name(), adjusted_pos, cached.position, *radius, label_offset, label_scale);
        } else {
            ctx.save();
            let scaled_font_size = font_size * label_scale;
            // Use muted color for passing loops
            ctx.set_fill_style_str(palette.passing_loop);
            ctx.set_font(&format!("{scaled_font_size}px sans-serif"));
            draw_station_label(ctx, &node.display_name(), adjusted_pos, cached.position, *radius, label_offset, label_scale);
            ctx.restore();
        }
    }
}
