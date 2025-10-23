use crate::models::{RailwayGraph, Stations, Junctions};
use crate::components::infrastructure_canvas::{track_renderer, junction_renderer};
use web_sys::CanvasRenderingContext2d;
use std::collections::HashMap;
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};

const NODE_RADIUS: f64 = 8.0;
const LABEL_OFFSET: f64 = 12.0;
const JUNCTION_LABEL_OFFSET: f64 = 28.0; // Larger offset for junctions to clear connection lines
const CHAR_WIDTH_ESTIMATE: f64 = 7.5;
const STATION_COLOR: &str = "#4a9eff";
const PASSING_LOOP_COLOR: &str = "#888";
const JUNCTION_LABEL_RADIUS: f64 = 22.0; // Match junction connection distance (14.0) + padding for label clearance
const NODE_FILL_COLOR: &str = "#2a2a2a";
const LABEL_COLOR: &str = "#fff";
const SELECTION_RING_COLOR: &str = "#ffaa00";
const SELECTION_RING_WIDTH: f64 = 3.0;
const SELECTION_RING_OFFSET: f64 = 4.0;

type TrackSegment = ((f64, f64), (f64, f64));

#[derive(Debug, Clone, Copy, PartialEq)]
enum LabelPosition {
    Right,
    Left,
    Top,
    Bottom,
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
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
            junction_renderer::draw_junction(ctx, graph, idx, pos, zoom);
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

#[must_use]
pub fn compute_label_positions(graph: &RailwayGraph, zoom: f64) -> HashMap<NodeIndex, (f64, f64, f64, f64)> {
    let font_size = 14.0 / zoom;
    let mut track_segments = track_renderer::get_track_segments(graph);
    track_segments.extend(junction_renderer::get_junction_segments(graph));

    let node_positions = get_node_positions_and_radii(graph);

    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();
    let mut node_neighbors: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();

    for edge in graph.graph.edge_references() {
        node_neighbors.entry(edge.source()).or_insert_with(Vec::new).push(edge.target());
        node_neighbors.entry(edge.target()).or_insert_with(Vec::new).push(edge.source());
    }

    let mut visited_for_traversal = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut bfs_parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut branch_positions: HashMap<NodeIndex, LabelPosition> = HashMap::new();

    if let Some((first_idx, _, _)) = node_positions.first() {
        queue.push_back(*first_idx);
        visited_for_traversal.insert(*first_idx);
    }

    while let Some(idx) = queue.pop_front() {
        let Some((_, pos, _radius)) = node_positions.iter().find(|(i, _, _)| *i == idx) else { continue };
        let Some(node) = graph.graph.node_weight(idx) else { continue };
        let name = node.display_name();
        #[allow(clippy::cast_precision_loss)]
        let text_width = name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;

        let is_junction = graph.is_junction(idx);
        let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };

        let preferred_position = bfs_parent.get(&idx)
            .and_then(|parent| branch_positions.get(parent))
            .copied();

        if let Some(neighbors) = node_neighbors.get(&idx) {
            for &neighbor in neighbors {
                if visited_for_traversal.insert(neighbor) {
                    queue.push_back(neighbor);
                    bfs_parent.insert(neighbor, idx);
                }
            }
        }

        let positions_to_try: Vec<LabelPosition> = if let Some(pref_pos) = preferred_position {
            let mut positions = vec![pref_pos];
            positions.extend(LabelPosition::all().into_iter().filter(|p| *p != pref_pos));
            positions
        } else {
            LabelPosition::all()
        };

        let mut best_position = LabelPosition::Right;
        let mut best_overlaps = usize::MAX;

        for position in positions_to_try {
            let bounds = calculate_label_bounds(position, *pos, text_width, font_size, label_offset);
            let overlaps = count_label_overlaps(&bounds, idx, &label_positions, &node_positions, &track_segments);

            if overlaps < best_overlaps {
                best_overlaps = overlaps;
                best_position = position;
                if overlaps == 0 {
                    break;
                }
            }
        }

        let bounds = calculate_label_bounds(best_position, *pos, text_width, font_size, label_offset);
        label_positions.insert(idx, (bounds, best_position));
        branch_positions.insert(idx, best_position);
    }

    label_positions.into_iter()
        .map(|(idx, (bounds, _))| (idx, (bounds.x, bounds.y, bounds.width, bounds.height)))
        .collect()
}

#[allow(clippy::cast_precision_loss)]
pub fn draw_stations(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
    selected_stations: &[NodeIndex],
) {
    let font_size = 14.0 / zoom;
    let mut track_segments = track_renderer::get_track_segments(graph);
    track_segments.extend(junction_renderer::get_junction_segments(graph));

    let node_positions = draw_station_nodes(ctx, graph, zoom, selected_stations);

    // Calculate optimal label positions using BFS traversal
    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();
    let mut node_neighbors: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();

    for edge in graph.graph.edge_references() {
        node_neighbors.entry(edge.source()).or_insert_with(Vec::new).push(edge.target());
        node_neighbors.entry(edge.target()).or_insert_with(Vec::new).push(edge.source());
    }

    let mut visited_for_traversal = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut bfs_parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();
    let mut branch_positions: HashMap<NodeIndex, LabelPosition> = HashMap::new();

    if let Some((first_idx, _, _)) = node_positions.first() {
        queue.push_back(*first_idx);
        visited_for_traversal.insert(*first_idx);
    }

    while let Some(idx) = queue.pop_front() {
        let Some((_, pos, _radius)) = node_positions.iter().find(|(i, _, _)| *i == idx) else { continue };
        let Some(node) = graph.graph.node_weight(idx) else { continue };
        let name = node.display_name();
        #[allow(clippy::cast_precision_loss)]
        let text_width = name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;

        // Use larger offset for junctions
        let is_junction = graph.is_junction(idx);
        let label_offset = if is_junction { JUNCTION_LABEL_OFFSET } else { LABEL_OFFSET };

        let preferred_position = bfs_parent.get(&idx)
            .and_then(|parent| branch_positions.get(parent))
            .copied();

        if let Some(neighbors) = node_neighbors.get(&idx) {
            for &neighbor in neighbors {
                if visited_for_traversal.insert(neighbor) {
                    queue.push_back(neighbor);
                    bfs_parent.insert(neighbor, idx);
                }
            }
        }

        let positions_to_try: Vec<LabelPosition> = if let Some(pref_pos) = preferred_position {
            let mut positions = vec![pref_pos];
            positions.extend(LabelPosition::all().into_iter().filter(|p| *p != pref_pos));
            positions
        } else {
            LabelPosition::all()
        };

        let mut best_position = LabelPosition::Right;
        let mut best_overlaps = usize::MAX;

        for position in positions_to_try {
            let bounds = calculate_label_bounds(position, *pos, text_width, font_size, label_offset);
            let overlaps = count_label_overlaps(&bounds, idx, &label_positions, &node_positions, &track_segments);

            if overlaps < best_overlaps {
                best_overlaps = overlaps;
                best_position = position;
                if overlaps == 0 {
                    break;
                }
            }
        }

        let bounds = calculate_label_bounds(best_position, *pos, text_width, font_size, label_offset);
        label_positions.insert(idx, (bounds, best_position));
        branch_positions.insert(idx, best_position);
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
