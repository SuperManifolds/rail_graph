use crate::models::RailwayGraph;
use web_sys::CanvasRenderingContext2d;
use std::collections::HashMap;
use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

const NODE_RADIUS: f64 = 8.0;
const LABEL_OFFSET: f64 = 12.0;
const CHAR_WIDTH_ESTIMATE: f64 = 7.5;

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

    fn calculate_label_pos(&self, node_pos: (f64, f64), text_width: f64, font_size: f64) -> (f64, f64) {
        let (x, y) = node_pos;
        match self {
            LabelPosition::Right => (x + LABEL_OFFSET, y + font_size / 3.0),
            LabelPosition::Left => (x - LABEL_OFFSET - text_width, y + font_size / 3.0),
            LabelPosition::Top => (x - text_width / 2.0, y - LABEL_OFFSET),
            LabelPosition::Bottom => (x - text_width / 2.0, y + LABEL_OFFSET + font_size),
            LabelPosition::TopRight => (x + LABEL_OFFSET * 0.7, y - LABEL_OFFSET * 0.7),
            LabelPosition::TopLeft => (x - LABEL_OFFSET * 0.7 - text_width, y - LABEL_OFFSET * 0.7),
            LabelPosition::BottomRight => (x + LABEL_OFFSET * 0.7, y + LABEL_OFFSET * 0.7 + font_size),
            LabelPosition::BottomLeft => (x - LABEL_OFFSET * 0.7 - text_width, y + LABEL_OFFSET * 0.7 + font_size),
        }
    }

    fn rotation_angle(&self) -> f64 {
        match self {
            LabelPosition::Top => -std::f64::consts::PI / 4.0,
            LabelPosition::Bottom => std::f64::consts::PI / 4.0,
            LabelPosition::TopRight => -std::f64::consts::PI / 4.0,
            LabelPosition::BottomRight => std::f64::consts::PI / 4.0,
            LabelPosition::TopLeft => std::f64::consts::PI / 4.0,
            LabelPosition::BottomLeft => -std::f64::consts::PI / 4.0,
            _ => 0.0,
        }
    }

    fn is_diagonal(&self) -> bool {
        matches!(self,
            LabelPosition::Top |
            LabelPosition::Bottom |
            LabelPosition::TopRight |
            LabelPosition::TopLeft |
            LabelPosition::BottomRight |
            LabelPosition::BottomLeft
        )
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

    t >= 0.0 && t <= 1.0 && u >= 0.0 && u <= 1.0
}

pub fn draw_stations(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    zoom: f64,
) {
    let font_size = 14.0 / zoom;

    // Collect all track segments
    let mut track_segments: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();
        if let (Some(pos1), Some(pos2)) = (graph.get_station_position(source), graph.get_station_position(target)) {
            track_segments.push((pos1, pos2));
        }
    }

    // First pass: draw all nodes and collect positions
    let mut node_positions: Vec<(NodeIndex, (f64, f64), f64)> = Vec::new();

    for idx in graph.graph.node_indices() {
        let Some(pos) = graph.get_station_position(idx) else { continue };
        let Some(station) = graph.graph.node_weight(idx) else { continue };

        let (border_color, radius) = if station.passing_loop {
            ("#888", NODE_RADIUS * 0.6)
        } else {
            ("#4a9eff", NODE_RADIUS)
        };

        // Draw node circle
        ctx.set_fill_style_str("#2a2a2a");
        ctx.set_stroke_style_str(border_color);
        ctx.set_line_width(2.0 / zoom);
        ctx.begin_path();
        let _ = ctx.arc(pos.0, pos.1, radius, 0.0, std::f64::consts::PI * 2.0);
        ctx.fill();
        ctx.stroke();

        node_positions.push((idx, pos, radius));
    }

    // Second pass: calculate optimal label positions
    let mut label_positions: HashMap<NodeIndex, (LabelBounds, LabelPosition)> = HashMap::new();

    // Build adjacency information to identify branches
    let mut node_neighbors: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
    for edge in graph.graph.edge_references() {
        node_neighbors.entry(edge.source()).or_insert_with(Vec::new).push(edge.target());
        node_neighbors.entry(edge.target()).or_insert_with(Vec::new).push(edge.source());
    }

    // Process nodes in BFS order to ensure we handle neighbors sequentially
    let mut visited_for_traversal = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    // Track parent in BFS tree to know which node we came from
    let mut bfs_parent: HashMap<NodeIndex, NodeIndex> = HashMap::new();

    // Start from the first node
    if let Some((first_idx, _, _)) = node_positions.first() {
        queue.push_back(*first_idx);
        visited_for_traversal.insert(*first_idx);
    }

    // Track the current position used on each branch for consistency
    let mut branch_positions: HashMap<NodeIndex, LabelPosition> = HashMap::new();

    while let Some(idx) = queue.pop_front() {
        let Some((_, pos, _radius)) = node_positions.iter().find(|(i, _, _)| *i == idx) else { continue };
        let Some(station) = graph.graph.node_weight(idx) else { continue };
        let text_width = station.name.len() as f64 * CHAR_WIDTH_ESTIMATE / zoom;

        // Always try to inherit from parent node in BFS tree
        let preferred_position: Option<LabelPosition> = bfs_parent.get(&idx)
            .and_then(|parent| branch_positions.get(parent))
            .copied();

        // Add unvisited neighbors to queue and track parent
        if let Some(neighbors) = node_neighbors.get(&idx) {
            for &neighbor in neighbors {
                if visited_for_traversal.insert(neighbor) {
                    queue.push_back(neighbor);
                    bfs_parent.insert(neighbor, idx);
                }
            }
        }

        let mut best_position = LabelPosition::Right;
        let mut best_overlaps = usize::MAX;

        // Always try preferred position first if we have one
        let positions_to_try: Vec<LabelPosition> = if let Some(pref_pos) = preferred_position {
            let mut positions = vec![pref_pos];
            positions.extend(LabelPosition::all().into_iter().filter(|p| *p != pref_pos));
            positions
        } else {
            LabelPosition::all()
        };

        for position in positions_to_try {
            let label_pos = position.calculate_label_pos(*pos, text_width, font_size);

            // For diagonal positions, calculate bounds for rotated text
            let bounds = if position.is_diagonal() {
                // For 45-degree rotated rectangle, the axis-aligned bounding box is:
                // new_width = width * |cos(45°)| + height * |sin(45°)|
                // new_height = width * |sin(45°)| + height * |cos(45°)|
                let cos45 = std::f64::consts::FRAC_1_SQRT_2; // 0.707...
                let text_height = font_size * 1.2;
                let rotated_width = text_width * cos45 + text_height * cos45;
                let rotated_height = text_width * cos45 + text_height * cos45;

                // Calculate offset in rotated coordinate system (matching draw code)
                let offset = LABEL_OFFSET;
                let angle = position.rotation_angle();

                let (x_offset_rotated, y_offset_rotated) = match position {
                    LabelPosition::Top => (offset * cos45, -offset * cos45),
                    LabelPosition::Bottom => (offset * cos45, offset * cos45),
                    LabelPosition::TopRight => (offset, 0.0),
                    LabelPosition::BottomRight => (offset, 0.0),
                    LabelPosition::TopLeft => (-offset, 0.0),
                    LabelPosition::BottomLeft => (-offset, 0.0),
                    _ => (0.0, 0.0),
                };

                // Transform from rotated coordinate system back to world coordinates
                let world_x = x_offset_rotated * angle.cos() - y_offset_rotated * angle.sin();
                let world_y = x_offset_rotated * angle.sin() + y_offset_rotated * angle.cos();

                // Center of the text in world coordinates
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
            };

            // Count overlaps with existing labels, nodes, and track segments
            let mut overlaps: usize = 0;

            for (other_bounds, _) in label_positions.values() {
                if bounds.overlaps(other_bounds) {
                    overlaps += 1;
                }
            }

            for (other_idx, other_pos, other_radius) in &node_positions {
                if *other_idx != idx && bounds.overlaps_node(*other_pos, *other_radius + 3.0) {
                    overlaps += 1;
                }
            }

            // Check for overlap with track segments
            for (p1, p2) in &track_segments {
                if bounds.intersects_line(*p1, *p2) {
                    overlaps += 1;
                }
            }

            if overlaps < best_overlaps {
                best_overlaps = overlaps;
                best_position = position;
                if overlaps == 0 {
                    break;
                }
            }
        }

        let label_pos = best_position.calculate_label_pos(*pos, text_width, font_size);
        let bounds = LabelBounds {
            x: label_pos.0,
            y: label_pos.1 - font_size,
            width: text_width,
            height: font_size * 1.2,
        };
        label_positions.insert(idx, (bounds, best_position));

        // Store the chosen position - this becomes the new default for connected nodes
        branch_positions.insert(idx, best_position);
    }

    // Third pass: draw labels at calculated positions with rotation
    ctx.set_fill_style_str("#fff");
    ctx.set_font(&format!("{}px sans-serif", font_size));

    for (idx, pos, _radius) in &node_positions {
        let Some(station) = graph.graph.node_weight(*idx) else { continue };
        let Some((bounds, position)) = label_positions.get(idx) else { continue };

        if position.is_diagonal() {
            // Draw rotated text for diagonal positions
            ctx.save();
            let _ = ctx.translate(pos.0, pos.1);
            let _ = ctx.rotate(position.rotation_angle());

            // After rotation, position text so first letter aligns with node center horizontally
            // but is offset vertically in world coordinates
            let offset = LABEL_OFFSET;
            let cos45 = std::f64::consts::FRAC_1_SQRT_2;

            // Calculate position in rotated coordinate system
            // For vertical offsets (Top/Bottom), transform world offset to rotated coords
            let (x_offset, y_offset) = match position {
                LabelPosition::Top => (offset * cos45, -offset * cos45),
                LabelPosition::Bottom => (offset * cos45, offset * cos45),
                LabelPosition::TopRight => (offset, 0.0),
                LabelPosition::BottomRight => (offset, 0.0),
                LabelPosition::TopLeft => (-offset, 0.0),
                LabelPosition::BottomLeft => (-offset, 0.0),
                _ => (0.0, 0.0),
            };

            let _ = ctx.fill_text(&station.name, x_offset, y_offset);
            ctx.restore();
        } else {
            // Draw normal text
            let _ = ctx.fill_text(&station.name, bounds.x, bounds.y + font_size);
        }
    }
}
