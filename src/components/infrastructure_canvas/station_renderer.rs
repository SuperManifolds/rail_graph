use crate::models::{RailwayGraph, Stations, Junctions};
use crate::components::infrastructure_canvas::{track_renderer, junction_renderer};
use web_sys::CanvasRenderingContext2d;
use std::collections::HashMap;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

type TrackSegment = ((f64, f64), (f64, f64));

const NODE_RADIUS: f64 = 8.0;
const LABEL_OFFSET: f64 = 12.0;
const JUNCTION_LABEL_OFFSET: f64 = 12.0; // Same as stations
const CHAR_WIDTH_ESTIMATE: f64 = 7.5;
const STATION_COLOR: &str = "#4a9eff";
const PASSING_LOOP_COLOR: &str = "#888";
const JUNCTION_LABEL_RADIUS: f64 = 22.0; // Match junction connection distance (14.0) + padding for label clearance
const NODE_FILL_COLOR: &str = "#2a2a2a";
const LABEL_COLOR: &str = "#fff";
const SELECTION_RING_COLOR: &str = "#ffaa00";
const SELECTION_RING_WIDTH: f64 = 3.0;
const SELECTION_RING_OFFSET: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
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

#[derive(Clone)]
struct LabelBounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
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
            if lines_intersect(p1, p2, c1, c2) {
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

fn lines_intersect(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), p4: (f64, f64)) -> bool {
    let d = (p2.0 - p1.0) * (p4.1 - p3.1) - (p2.1 - p1.1) * (p4.0 - p3.0);
    if d.abs() < 1e-10 {
        return false;
    }

    let t = ((p3.0 - p1.0) * (p4.1 - p3.1) - (p3.1 - p1.1) * (p4.0 - p3.0)) / d;
    let u = ((p3.0 - p1.0) * (p2.1 - p1.1) - (p3.1 - p1.1) * (p2.0 - p1.0)) / d;

    (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)
}

fn draw_station_nodes(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
    selected_stations: &[NodeIndex],
    highlighted_edges: &std::collections::HashSet<petgraph::stable_graph::EdgeIndex>,
) -> Vec<(NodeIndex, (f64, f64), f64)> {
    let mut node_positions = Vec::new();

    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(node) = graph.graph.node_weight(idx) else { continue };

        if let Some(station) = node.as_station() {
            // Draw stations as circles
            let (border_color, radius) = if station.passing_loop {
                (PASSING_LOOP_COLOR, NODE_RADIUS * 0.6)
            } else {
                (STATION_COLOR, NODE_RADIUS)
            };

            ctx.set_fill_style_str(NODE_FILL_COLOR);
            ctx.set_stroke_style_str(border_color);
            ctx.set_line_width(2.0 / zoom);
            ctx.begin_path();
            let _ = ctx.arc(pos.0, pos.1, radius, 0.0, std::f64::consts::PI * 2.0);
            ctx.fill();
            ctx.stroke();

            // Draw selection ring if this station is selected
            if selected_stations.contains(&idx) {
                ctx.set_stroke_style_str(SELECTION_RING_COLOR);
                ctx.set_line_width(SELECTION_RING_WIDTH / zoom);
                ctx.begin_path();
                let _ = ctx.arc(pos.0, pos.1, radius + SELECTION_RING_OFFSET, 0.0, std::f64::consts::PI * 2.0);
                ctx.stroke();
            }

            node_positions.push((idx, pos, radius));
        } else if graph.is_junction(idx) {
            // Draw junction
            junction_renderer::draw_junction(ctx, graph, idx, pos, zoom, highlighted_edges);
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

fn draw_station_label(
    ctx: &CanvasRenderingContext2d,
    station_name: &str,
    pos: (f64, f64),
    position: LabelPosition,
    radius: f64,
    offset: f64,
) {
    ctx.save();
    ctx.set_text_align(position.text_align());
    ctx.set_text_baseline(LabelPosition::text_baseline());

    let total_offset = radius + offset;

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

fn get_node_positions_and_radii(graph: &RailwayGraph) -> Vec<(NodeIndex, (f64, f64), f64)> {
    let mut node_positions = Vec::new();

    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(node) = graph.graph.node_weight(idx) else { continue };

        if let Some(station) = node.as_station() {
            let radius = if station.passing_loop {
                NODE_RADIUS * 0.6
            } else {
                NODE_RADIUS
            };
            node_positions.push((idx, pos, radius));
        } else if graph.is_junction(idx) {
            node_positions.push((idx, pos, JUNCTION_LABEL_RADIUS));
        }
    }

    node_positions
}

fn identify_branches(graph: &RailwayGraph, node_positions: &[(NodeIndex, (f64, f64), f64)]) -> Vec<Vec<NodeIndex>> {
    use std::collections::HashSet;

    // Build adjacency map and calculate degrees
    let mut adjacency: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
    for edge in graph.graph.edge_references() {
        adjacency.entry(edge.source()).or_insert_with(Vec::new).push(edge.target());
        adjacency.entry(edge.target()).or_insert_with(Vec::new).push(edge.source());
    }

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

        for &next_node in neighbors {
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
                let next = neighbors.iter().find(|&&n| n != previous);

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

        for &neighbor in neighbors {
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
                let next = neighbors.iter().find(|&&n| n != previous);

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
) {
    if nodes.is_empty() {
        return;
    }

    // Try all orientations and find the one with minimum overlaps
    let mut best_orientation = LabelPosition::Right;
    let mut best_total_overlaps = usize::MAX;

    for orientation in LabelPosition::all() {
        let mut total_overlaps = 0;

        for &node_idx in nodes {
            if let Some((text_width, label_offset, pos)) = node_metadata.get(&node_idx) {
                let bounds = calculate_label_bounds(orientation, *pos, *text_width, font_size, *label_offset);
                let overlaps = count_label_overlaps(&bounds, node_idx, label_positions, node_positions, track_segments);
                total_overlaps += overlaps;
            }
        }

        if total_overlaps < best_total_overlaps {
            best_total_overlaps = total_overlaps;
            best_orientation = orientation;
            if total_overlaps == 0 {
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

    let node_positions = get_node_positions_and_radii(graph);

    // Build node metadata (width, offset, position)
    let mut node_metadata: HashMap<NodeIndex, (f64, f64, (f64, f64))> = HashMap::new();
    for (idx, pos, _) in &node_positions {
        if let Some(node) = graph.graph.node_weight(*idx) {
            let name = node.display_name();
            #[allow(clippy::cast_precision_loss)]
            let text_width = name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;
            let is_junction = graph.is_junction(*idx);
            let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };
            node_metadata.insert(*idx, (text_width, label_offset, *pos));
        }
    }

    let mut node_neighbors: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
    for edge in graph.graph.edge_references() {
        node_neighbors.entry(edge.source()).or_insert_with(Vec::new).push(edge.target());
        node_neighbors.entry(edge.target()).or_insert_with(Vec::new).push(edge.source());
    }

    // Identify branches using BFS
    let branches = identify_branches(graph, &node_positions);

    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();

    // First pass: process all stations (excluding junctions from branches)
    for branch_nodes in &branches {
        let station_only_nodes: Vec<NodeIndex> = branch_nodes.iter()
            .filter(|idx| !graph.is_junction(**idx))
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
            );
        }
    }

    // Second pass: process each junction individually to find best position
    let junction_nodes: Vec<NodeIndex> = node_positions.iter()
        .filter(|(idx, _, _)| graph.is_junction(*idx))
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
        );
    }

    label_positions.into_iter()
        .map(|(idx, (bounds, _))| (idx, (bounds.x, bounds.y, bounds.width, bounds.height)))
        .collect()
}

/// Draw stations with cached label positions for performance during zoom
#[allow(clippy::cast_precision_loss)]
pub fn draw_stations_with_cache(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
    selected_stations: &[NodeIndex],
    highlighted_edges: &std::collections::HashSet<petgraph::stable_graph::EdgeIndex>,
    cache: &mut super::renderer::TopologyCache,
    is_zooming: bool,
) {
    let font_size = 14.0 / zoom;

    let node_positions = draw_station_nodes(ctx, graph, zoom, selected_stations, highlighted_edges);

    // Check if we can use cached label positions
    let use_cache = if let Some((cached_zoom, _)) = &cache.label_cache {
        // Use cache if zooming and zoom hasn't changed drastically (>20%)
        is_zooming && (cached_zoom - zoom).abs() / cached_zoom < 0.2
    } else {
        false
    };

    if use_cache {
        // Use cached positions
        if let Some((_, cached_positions)) = &cache.label_cache {
            draw_cached_labels(ctx, graph, &node_positions, cached_positions, font_size);
        }
        return;
    }

    // Full recomputation - compute optimal label positions
    let mut track_segments = track_renderer::get_track_segments(graph);
    track_segments.extend(junction_renderer::get_junction_segments(graph));

    // Build node metadata
    let mut node_metadata: HashMap<NodeIndex, (f64, f64, (f64, f64))> = HashMap::new();
    for (idx, pos, _) in &node_positions {
        if let Some(node) = graph.graph.node_weight(*idx) {
            let name = node.display_name();
            #[allow(clippy::cast_precision_loss)]
            let text_width = name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;
            let is_junction = graph.is_junction(*idx);
            let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };
            node_metadata.insert(*idx, (text_width, label_offset, *pos));
        }
    }

    // Compute optimal label positions
    let branches = identify_branches(graph, &node_positions);
    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();

    for branch_nodes in &branches {
        let station_only_nodes: Vec<NodeIndex> = branch_nodes.iter()
            .filter(|idx| !graph.is_junction(**idx))
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
            );
        }
    }

    let junction_nodes: Vec<NodeIndex> = node_positions.iter()
        .filter(|(idx, _, _)| graph.is_junction(*idx))
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
        );
    }

    // Update cache with computed positions
    let cached_positions: HashMap<NodeIndex, CachedLabelPosition> = label_positions.iter()
        .map(|(idx, (_, position))| (*idx, CachedLabelPosition { position: *position }))
        .collect();
    cache.label_cache = Some((zoom, cached_positions));

    // Draw labels using computed positions
    ctx.set_fill_style_str(LABEL_COLOR);
    ctx.set_font(&format!("{font_size}px sans-serif"));

    for (idx, pos, radius) in &node_positions {
        let Some(node) = graph.graph.node_weight(*idx) else { continue };
        let Some((_, position)) = label_positions.get(idx) else { continue };
        let is_junction = graph.is_junction(*idx);
        let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };
        draw_station_label(ctx, &node.display_name(), *pos, *position, *radius, label_offset);
    }
}

fn draw_cached_labels(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    node_positions: &[(NodeIndex, (f64, f64), f64)],
    cached_positions: &HashMap<NodeIndex, CachedLabelPosition>,
    font_size: f64,
) {
    ctx.set_fill_style_str(LABEL_COLOR);
    ctx.set_font(&format!("{font_size}px sans-serif"));

    for (idx, pos, radius) in node_positions {
        let Some(node) = graph.graph.node_weight(*idx) else { continue };
        if let Some(cached) = cached_positions.get(idx) {
            let is_junction = graph.is_junction(*idx);
            let label_offset = if is_junction {
                JUNCTION_LABEL_OFFSET
            } else {
                LABEL_OFFSET
            };
            draw_station_label(ctx, &node.display_name(), *pos, cached.position, *radius, label_offset);
        }
    }
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_stations(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
    selected_stations: &[NodeIndex],
    highlighted_edges: &std::collections::HashSet<petgraph::stable_graph::EdgeIndex>,
) {
    let font_size = 14.0 / zoom;
    let mut track_segments = track_renderer::get_track_segments(graph);
    track_segments.extend(junction_renderer::get_junction_segments(graph));

    let node_positions = draw_station_nodes(ctx, graph, zoom, selected_stations, highlighted_edges);

    // Build node metadata (width, offset, position)
    let mut node_metadata: HashMap<NodeIndex, (f64, f64, (f64, f64))> = HashMap::new();
    for (idx, pos, _) in &node_positions {
        if let Some(node) = graph.graph.node_weight(*idx) {
            let name = node.display_name();
            #[allow(clippy::cast_precision_loss)]
            let text_width = name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;
            let is_junction = graph.is_junction(*idx);
            let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };
            node_metadata.insert(*idx, (text_width, label_offset, *pos));
        }
    }

    // Identify branches and process each one
    let branches = identify_branches(graph, &node_positions);
    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();

    // First pass: process all stations (excluding junctions from branches)
    for branch_nodes in &branches {
        let station_only_nodes: Vec<NodeIndex> = branch_nodes.iter()
            .filter(|idx| !graph.is_junction(**idx))
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
            );
        }
    }

    // Second pass: process each junction individually to find best position
    let junction_nodes: Vec<NodeIndex> = node_positions.iter()
        .filter(|(idx, _, _)| graph.is_junction(*idx))
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
        );
    }

    // Draw all labels
    ctx.set_fill_style_str(LABEL_COLOR);
    ctx.set_font(&format!("{font_size}px sans-serif"));

    for (idx, pos, radius) in &node_positions {
        let Some(node) = graph.graph.node_weight(*idx) else { continue };
        let Some((_, position)) = label_positions.get(idx) else { continue };
        let is_junction = graph.is_junction(*idx);
        let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };
        draw_station_label(ctx, &node.display_name(), *pos, *position, *radius, label_offset);
    }
}
