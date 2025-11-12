use crate::models::{Line, RailwayGraph, Stations};
use crate::theme::Theme;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use petgraph::visit::IntoEdgeReferences;
use std::collections::{HashMap, HashSet};
use indexmap::IndexMap;
use web_sys::CanvasRenderingContext2d;

const LINE_BASE_WIDTH: f64 = 3.0;
const AVOIDANCE_OFFSET_THRESHOLD: f64 = 0.1;
const TRANSITION_LENGTH: f64 = 30.0;
const JUNCTION_STOP_DISTANCE: f64 = 14.0;

/// Unique identifier for a section (group of edges between junctions)
pub type SectionId = usize;

/// Key for identifying a junction connection between two edges
#[derive(Hash, Eq, PartialEq, Clone, Copy)]
struct JunctionConnectionKey {
    junction: NodeIndex,
    from_edge: EdgeIndex,
    to_edge: EdgeIndex,
}

/// Section information: consecutive edges between junctions
#[derive(Clone)]
pub struct Section {
    pub id: SectionId,
    pub edges: Vec<EdgeIndex>,
}

/// Assign visual positions to lines within a section based on which lines conflict (share edges).
/// Lines that never share edges can reuse the same position.
/// Returns: `edge_index` -> (`line_id` -> `visual_position_index`)
#[must_use]
pub fn assign_visual_positions_with_reuse(
    section: &Section,
    section_ordering: &[&Line],
    edge_to_lines: &IndexMap<EdgeIndex, Vec<&Line>>,
    graph: &RailwayGraph,
) -> HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>> {
    // Build conflict map: which lines share at least one edge OR station
    let mut conflicts: HashMap<uuid::Uuid, HashSet<uuid::Uuid>> = HashMap::new();

    // Track which lines connect to each station in this section
    let mut station_to_lines: HashMap<NodeIndex, Vec<uuid::Uuid>> = HashMap::new();

    for edge_idx in &section.edges {
        let Some(lines_on_edge) = edge_to_lines.get(edge_idx) else { continue };

        // All lines on this edge conflict with each other
        for line_a in lines_on_edge {
            for line_b in lines_on_edge {
                if line_a.id != line_b.id {
                    conflicts.entry(line_a.id).or_default().insert(line_b.id);
                }
            }
        }

        // Track which lines touch which stations
        if let Some((source, target)) = graph.graph.edge_endpoints(*edge_idx) {
            for line in lines_on_edge {
                station_to_lines.entry(source).or_default().push(line.id);
                station_to_lines.entry(target).or_default().push(line.id);
            }
        }
    }

    // Add conflicts for lines that share stations
    for lines_at_station in station_to_lines.values() {
        for line_a_id in lines_at_station {
            for line_b_id in lines_at_station {
                if line_a_id != line_b_id {
                    conflicts.entry(*line_a_id).or_default().insert(*line_b_id);
                }
            }
        }
    }

    // Assign positions using greedy graph coloring based on section ordering
    let mut line_to_position: HashMap<uuid::Uuid, usize> = HashMap::new();

    for line in section_ordering {
        // Find the lowest position not used by any conflicting line
        let conflicting_positions: HashSet<usize> = conflicts
            .get(&line.id)
            .map(|conflict_ids| {
                conflict_ids
                    .iter()
                    .filter_map(|id| line_to_position.get(id).copied())
                    .collect()
            })
            .unwrap_or_default();

        // Find lowest available position
        let mut position = 0;
        while conflicting_positions.contains(&position) {
            position += 1;
        }

        line_to_position.insert(line.id, position);
    }

    // Build result: for each edge, map line IDs to their positions
    let mut result: HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>> = HashMap::new();

    for edge_idx in &section.edges {
        let Some(lines_on_edge) = edge_to_lines.get(edge_idx) else { continue };

        let mut edge_positions = HashMap::new();
        for line in lines_on_edge {
            if let Some(&position) = line_to_position.get(&line.id) {
                edge_positions.insert(line.id, position);
            }
        }

        result.insert(*edge_idx, edge_positions);
    }

    result
}

/// Identify sections: groups of consecutive edges between junctions
#[must_use]
pub fn identify_sections(graph: &RailwayGraph, junctions: &HashSet<NodeIndex>) -> Vec<Section> {
    use petgraph::visit::EdgeRef;

    let mut sections = Vec::new();
    let mut visited_edges: HashSet<EdgeIndex> = HashSet::new();
    let mut section_id = 0;

    // For each edge, if not visited, start a new section
    for edge_ref in graph.graph.edge_references() {
        let edge_idx = edge_ref.id();

        if visited_edges.contains(&edge_idx) {
            continue;
        }

        // Start new section with this edge
        let mut section_edges = vec![edge_idx];
        visited_edges.insert(edge_idx);

        // Try to extend section in both directions until hitting junctions
        let (source, target) = (edge_ref.source(), edge_ref.target());

        // Extend backwards from source (if source is not a junction)
        if !junctions.contains(&source) {
            extend_section_from_node(
                graph,
                source,
                edge_idx,
                junctions,
                &mut section_edges,
                &mut visited_edges,
                false, // backwards
            );
        }

        // Extend forwards from target (if target is not a junction)
        if !junctions.contains(&target) {
            extend_section_from_node(
                graph,
                target,
                edge_idx,
                junctions,
                &mut section_edges,
                &mut visited_edges,
                true, // forwards
            );
        }

        sections.push(Section {
            id: section_id,
            edges: section_edges,
        });
        section_id += 1;
    }

    sections
}

/// Extend a section from a node in one direction until hitting a junction
fn extend_section_from_node(
    graph: &RailwayGraph,
    start_node: NodeIndex,
    from_edge: EdgeIndex,
    junctions: &HashSet<NodeIndex>,
    section_edges: &mut Vec<EdgeIndex>,
    visited_edges: &mut HashSet<EdgeIndex>,
    forwards: bool,
) {
    use petgraph::visit::EdgeRef;

    let mut current_node = start_node;
    let mut previous_edge = from_edge;

    loop {
        // If current node is a junction, stop
        if junctions.contains(&current_node) {
            break;
        }

        // Find the next edge connected to this node (excluding the edge we came from)
        let mut next_edge: Option<(EdgeIndex, NodeIndex)> = None;

        for edge_ref in graph.graph.edges(current_node) {
            let edge_idx = edge_ref.id();
            if edge_idx == previous_edge || visited_edges.contains(&edge_idx) {
                continue;
            }

            let (src, tgt) = (edge_ref.source(), edge_ref.target());
            let other_node = if src == current_node { tgt } else { src };

            next_edge = Some((edge_idx, other_node));
            break;
        }

        // If no next edge, we've reached the end
        let Some((edge_idx, next_node)) = next_edge else {
            break;
        };

        // Add edge to section
        if forwards {
            section_edges.push(edge_idx);
        } else {
            section_edges.insert(0, edge_idx);
        }
        visited_edges.insert(edge_idx);

        // Move to next node
        previous_edge = edge_idx;
        current_node = next_node;
    }
}

/// Get which lines traverse each section
#[must_use]
pub fn get_lines_in_section<'a>(
    sections: &[Section],
    lines: &'a [Line],
) -> HashMap<SectionId, Vec<&'a Line>> {
    let mut section_lines: HashMap<SectionId, Vec<&Line>> = HashMap::new();

    for section in sections {
        let mut lines_in_section = Vec::new();

        for line in lines {
            if !line.visible {
                continue;
            }

            // Check if line uses any edge in this section
            let uses_section = section.edges.iter().any(|&edge_idx| {
                line.forward_route.iter().any(|seg| EdgeIndex::new(seg.edge_index) == edge_idx)
            });

            if uses_section {
                lines_in_section.push(line);
            }
        }

        section_lines.insert(section.id, lines_in_section);
    }

    section_lines
}

/// Get lines in each section, using pre-sorted line references
#[must_use]
pub fn get_lines_in_section_sorted<'a>(
    sections: &[Section],
    sorted_lines: &[&'a Line],
) -> HashMap<SectionId, Vec<&'a Line>> {
    let mut section_lines: HashMap<SectionId, Vec<&Line>> = HashMap::new();

    for section in sections {
        let mut lines_in_section = Vec::new();

        for &line in sorted_lines {
            if !line.visible {
                continue;
            }

            // Check if line uses any edge in this section
            let uses_section = section.edges.iter().any(|&edge_idx| {
                line.forward_route.iter().any(|seg| EdgeIndex::new(seg.edge_index) == edge_idx)
            });

            if uses_section {
                lines_in_section.push(line);
            }
        }

        section_lines.insert(section.id, lines_in_section);
    }

    section_lines
}

/// Compare two lines based on their stopping behavior in shared segments
/// Returns:
/// - `Ordering::Less` if `line_a` stops less in shared segments (more express)
/// - `Ordering::Greater` if `line_a` stops more in shared segments (more local)
/// - `Ordering::Equal` if they have the same stopping ratio in shared segments
fn compare_lines_by_shared_stops(line_a: &Line, line_b: &Line) -> std::cmp::Ordering {
    // Find all edge indices that both lines traverse
    let edges_a: HashSet<usize> = line_a.forward_route.iter()
        .map(|seg| seg.edge_index)
        .collect();
    let edges_b: HashSet<usize> = line_b.forward_route.iter()
        .map(|seg| seg.edge_index)
        .collect();

    let shared_edges: Vec<usize> = edges_a.intersection(&edges_b)
        .copied()
        .collect();

    // If no shared segments, compare by total route length (shorter = more express = LEFT)
    if shared_edges.is_empty() {
        return line_a.forward_route.len().cmp(&line_b.forward_route.len());
    }

    // Count how many stops each line makes in shared segments
    // A stop is indicated by wait_time > 0
    let shared_segs_a: Vec<_> = line_a.forward_route.iter()
        .filter(|seg| shared_edges.contains(&seg.edge_index))
        .collect();
    let shared_segs_b: Vec<_> = line_b.forward_route.iter()
        .filter(|seg| shared_edges.contains(&seg.edge_index))
        .collect();

    let stop_count_a = shared_segs_a.iter()
        .filter(|seg| !seg.wait_time.is_zero())
        .count();
    let stop_count_b = shared_segs_b.iter()
        .filter(|seg| !seg.wait_time.is_zero())
        .count();

    // Compare by stop count: fewer stops = more express = should come first (LEFT)
    // So ascending order of stop count
    stop_count_a.cmp(&stop_count_b)
}

/// Order lines within a section by comparing their stopping behavior in shared segments
/// Returns ordered list of lines where:
/// - For each pair of lines, the one that stops LESS in their shared segments is positioned LEFT
/// - Line ID used for tie-breaking when two lines have equal stopping behavior
#[must_use]
pub fn order_lines_for_section<'a>(
    section_lines: &[&'a Line],
    _section_edges: &[EdgeIndex],
) -> Vec<&'a Line> {
    let mut ordered_lines = section_lines.to_vec();

    ordered_lines.sort_by(|a, b| {
        // Primary: compare based on stopping behavior in shared segments
        // (fewer stops in shared segments = more express = LEFT)
        compare_lines_by_shared_stops(a, b)
            // Secondary: line ID for stability
            .then(a.id.cmp(&b.id))
    });

    ordered_lines
}

/// Draw lines through junctions
#[allow(clippy::cast_precision_loss, clippy::too_many_lines, clippy::too_many_arguments)]
fn draw_junction_connections(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    junction_connections: &IndexMap<JunctionConnectionKey, Vec<&Line>>,
    edge_to_lines: &IndexMap<EdgeIndex, Vec<&Line>>,
    edge_to_section: &HashMap<EdgeIndex, SectionId>,
    section_orderings: &HashMap<SectionId, Vec<&Line>>,
    section_visual_positions: &HashMap<SectionId, HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>>>,
    gap_width: f64,
    zoom: f64,
) {
    for (connection_key, connection_lines) in junction_connections {
        let Some(junction_pos) = graph.get_station_position(connection_key.junction) else {
            continue;
        };

        // Get the endpoints of both edges
        let Some((from_src, from_tgt)) = graph.graph.edge_endpoints(connection_key.from_edge) else {
            continue;
        };
        let Some((to_src, to_tgt)) = graph.graph.edge_endpoints(connection_key.to_edge) else {
            continue;
        };

        // Determine the node positions for entry and exit edges
        // We need to know if junction is source or target to calculate direction correctly
        let from_junction_is_target = from_tgt == connection_key.junction;
        let to_junction_is_source = to_src == connection_key.junction;

        let from_node_pos = if from_junction_is_target {
            graph.get_station_position(from_src)
        } else {
            graph.get_station_position(from_tgt)
        };

        let to_node_pos = if to_junction_is_source {
            graph.get_station_position(to_tgt)
        } else {
            graph.get_station_position(to_src)
        };

        let (Some(from_pos), Some(to_pos)) = (from_node_pos, to_node_pos) else {
            continue;
        };

        // Calculate edge directions FROM source TO target (matching edge drawing code)
        // For entry edge: if junction is target, edge goes from_pos -> junction
        //                 if junction is source, edge goes junction -> from_pos
        let entry_delta = if from_junction_is_target {
            (junction_pos.0 - from_pos.0, junction_pos.1 - from_pos.1)
        } else {
            (from_pos.0 - junction_pos.0, from_pos.1 - junction_pos.1)
        };
        let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();

        // For exit edge: if junction is source, edge goes junction -> to_pos
        //                if junction is target, edge goes to_pos -> junction
        let exit_delta = if to_junction_is_source {
            (to_pos.0 - junction_pos.0, to_pos.1 - junction_pos.1)
        } else {
            (junction_pos.0 - to_pos.0, junction_pos.1 - to_pos.1)
        };
        let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();

        if entry_distance < 0.1 || exit_distance < 0.1 {
            continue;
        }

        // Verify that both edges exist in the edge_to_lines map
        if !edge_to_lines.contains_key(&connection_key.from_edge) ||
           !edge_to_lines.contains_key(&connection_key.to_edge) {
            continue;
        }

        // Calculate base stop points without extension
        let entry_base_no_ext = if from_junction_is_target {
            (
                junction_pos.0 - (entry_delta.0 / entry_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 - (entry_delta.1 / entry_distance) * JUNCTION_STOP_DISTANCE,
            )
        } else {
            (
                junction_pos.0 + (entry_delta.0 / entry_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 + (entry_delta.1 / entry_distance) * JUNCTION_STOP_DISTANCE,
            )
        };

        let exit_base_no_ext = if to_junction_is_source {
            (
                junction_pos.0 + (exit_delta.0 / exit_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 + (exit_delta.1 / exit_distance) * JUNCTION_STOP_DISTANCE,
            )
        } else {
            (
                junction_pos.0 - (exit_delta.0 / exit_distance) * JUNCTION_STOP_DISTANCE,
                junction_pos.1 - (exit_delta.1 / exit_distance) * JUNCTION_STOP_DISTANCE,
            )
        };

        // Calculate perpendicular vectors for offsets (matching edge drawing code)
        // Perpendicular is (-dy/len, dx/len) where (dx, dy) is the edge direction
        let entry_perp = (-entry_delta.1 / entry_distance, entry_delta.0 / entry_distance);
        let mut exit_perp = (-exit_delta.1 / exit_distance, exit_delta.0 / exit_distance);

        // Check if perpendiculars point in opposite directions
        let dot_product = entry_perp.0 * exit_perp.0 + entry_perp.1 * exit_perp.1;
        let flip_exit_offsets = if dot_product < 0.0 {
            // Flip exit perpendicular so they point in the same global direction
            exit_perp = (-exit_perp.0, -exit_perp.1);
            true
        } else {
            false
        };

        // Get section orderings for entry and exit edges
        let Some(&entry_section_id) = edge_to_section.get(&connection_key.from_edge) else {
            continue;
        };
        let Some(&exit_section_id) = edge_to_section.get(&connection_key.to_edge) else {
            continue;
        };
        let Some(entry_section_ordering) = section_orderings.get(&entry_section_id) else {
            continue;
        };
        let Some(exit_section_ordering) = section_orderings.get(&exit_section_id) else {
            continue;
        };

        // Calculate widths for entry section
        let entry_section_widths: Vec<f64> = entry_section_ordering.iter()
            .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
            .collect();
        let entry_num_gaps = entry_section_ordering.len().saturating_sub(1);
        let entry_total_width: f64 = entry_section_widths.iter().sum::<f64>()
            + (entry_num_gaps as f64) * gap_width;

        // Calculate widths for exit section
        let exit_section_widths: Vec<f64> = exit_section_ordering.iter()
            .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
            .collect();
        let exit_num_gaps = exit_section_ordering.len().saturating_sub(1);
        let exit_total_width: f64 = exit_section_widths.iter().sum::<f64>()
            + (exit_num_gaps as f64) * gap_width;

        // Get visual position maps for entry and exit edges
        let entry_visual_map = section_visual_positions.get(&entry_section_id)
            .and_then(|section_map| section_map.get(&connection_key.from_edge));
        let exit_visual_map = section_visual_positions.get(&exit_section_id)
            .and_then(|section_map| section_map.get(&connection_key.to_edge));

        // Sort connection lines by (sort_index, id) for consistent z-order
        let mut sorted_connection_lines = connection_lines.clone();
        sorted_connection_lines.sort_by(|a, b| {
            match (a.sort_index, b.sort_index) {
                (Some(a_idx), Some(b_idx)) => a_idx.partial_cmp(&b_idx).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| a.id.cmp(&b.id))
        });

        // Draw each line through the junction from its position on entry edge to its position on exit edge
        for line in &sorted_connection_lines {
            // Find line's visual position in entry section
            let Some(&entry_visual_pos) = entry_visual_map.and_then(|map| map.get(&line.id)) else {
                continue;
            };

            // Calculate entry offset based on visual position (NOT compacted)
            let entry_start_offset = -entry_total_width / 2.0;
            let entry_offset_sum: f64 = entry_section_widths.iter().take(entry_visual_pos)
                .map(|&width| width + gap_width)
                .sum();
            let line_entry_width = entry_section_widths.get(entry_visual_pos)
                .copied()
                .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
            let mut entry_offset = entry_start_offset + entry_offset_sum + line_entry_width / 2.0;

            // Find line's visual position in exit section
            let Some(&exit_visual_pos) = exit_visual_map.and_then(|map| map.get(&line.id)) else {
                continue;
            };

            // Calculate exit offset based on visual position (NOT compacted)
            let exit_start_offset = -exit_total_width / 2.0;
            let exit_offset_sum: f64 = exit_section_widths.iter().take(exit_visual_pos)
                .map(|&width| width + gap_width)
                .sum();
            let line_exit_width = exit_section_widths.get(exit_visual_pos)
                .copied()
                .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
            let mut exit_offset = exit_start_offset + exit_offset_sum + line_exit_width / 2.0;

            // Apply geometric flip if needed (perpendiculars point in opposite directions)
            if flip_exit_offsets {
                entry_offset = -entry_offset;
                exit_offset = -exit_offset;
            }

            let line_world_width = (line_entry_width + line_exit_width) / 2.0; // Average width for junction curve

            // Apply perpendicular offsets to base points for line positioning
            let entry_point = (
                entry_base_no_ext.0 + entry_perp.0 * entry_offset,
                entry_base_no_ext.1 + entry_perp.1 * entry_offset
            );

            let exit_point = (
                exit_base_no_ext.0 + exit_perp.0 * exit_offset,
                exit_base_no_ext.1 + exit_perp.1 * exit_offset
            );

            ctx.set_line_width(line_world_width);
            ctx.set_stroke_style_str(&line.color);
            ctx.begin_path();
            ctx.move_to(entry_point.0, entry_point.1);

            // Find control point by intersecting straight lines along edge directions
            // Line 1: entry_point + t * entry_delta (continuing along entry edge)
            // Line 2: exit_point + s * (-exit_delta) (going back along exit edge)
            let entry_dir = (entry_delta.0 / entry_distance, entry_delta.1 / entry_distance);
            let exit_dir_back = (-exit_delta.0 / exit_distance, -exit_delta.1 / exit_distance);

            // Find intersection: entry_point + t * entry_dir = exit_point + s * exit_dir_back
            let det = entry_dir.0 * exit_dir_back.1 - entry_dir.1 * exit_dir_back.0;

            if det.abs() > 0.01 {
                // Lines intersect (not parallel)
                let dx = exit_point.0 - entry_point.0;
                let dy = exit_point.1 - entry_point.1;
                let t = (dx * exit_dir_back.1 - dy * exit_dir_back.0) / det;

                // Check if intersection is in forward direction (t > 0)
                if t > 0.0 {
                    // Use quadratic curve with intersection as control point
                    let control_point = (
                        entry_point.0 + t * entry_dir.0,
                        entry_point.1 + t * entry_dir.1
                    );
                    ctx.quadratic_curve_to(control_point.0, control_point.1, exit_point.0, exit_point.1);
                } else {
                    // Intersection behind us, use straight line
                    ctx.line_to(exit_point.0, exit_point.1);
                }
            } else {
                // Lines are parallel, use S-curve (bezier with two control points)
                let control_dist = 15.0;
                let cp1 = (
                    entry_point.0 + entry_dir.0 * control_dist,
                    entry_point.1 + entry_dir.1 * control_dist
                );
                let cp2 = (
                    exit_point.0 + exit_dir_back.0 * control_dist,
                    exit_point.1 + exit_dir_back.1 * control_dist
                );
                ctx.bezier_curve_to(cp1.0, cp1.1, cp2.0, cp2.1, exit_point.0, exit_point.1);
            }

            ctx.stroke();
        }
    }
}

/// Draw a line segment with optional avoidance transitions
fn draw_line_segment_with_avoidance(
    ctx: &CanvasRenderingContext2d,
    pos1: (f64, f64),
    pos2: (f64, f64),
    segment_length: f64,
    line_offset: (f64, f64),
    avoidance_offset: (f64, f64),
    transitions: (bool, bool),
) {
    let (ox, oy) = line_offset;
    let (avoid_x, avoid_y) = avoidance_offset;
    let (start_needs_transition, end_needs_transition) = transitions;

    if start_needs_transition {
        ctx.move_to(pos1.0 + ox, pos1.1 + oy);
        let t1 = TRANSITION_LENGTH / segment_length;
        let mid1_x = pos1.0 + (pos2.0 - pos1.0) * t1;
        let mid1_y = pos1.1 + (pos2.1 - pos1.1) * t1;
        ctx.line_to(mid1_x + ox + avoid_x, mid1_y + oy + avoid_y);
    } else {
        ctx.move_to(pos1.0 + ox + avoid_x, pos1.1 + oy + avoid_y);
    }

    if end_needs_transition {
        let t2 = (segment_length - TRANSITION_LENGTH) / segment_length;
        let mid2_x = pos1.0 + (pos2.0 - pos1.0) * t2;
        let mid2_y = pos1.1 + (pos2.1 - pos1.1) * t2;
        ctx.line_to(mid2_x + ox + avoid_x, mid2_y + oy + avoid_y);
        ctx.line_to(pos2.0 + ox, pos2.1 + oy);
    } else {
        ctx.line_to(pos2.0 + ox + avoid_x, pos2.1 + oy + avoid_y);
    }
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
pub fn draw_lines(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    lines: &[Line],
    zoom: f64,
    cached_avoidance: &HashMap<EdgeIndex, (f64, f64)>,
    viewport_bounds: (f64, f64, f64, f64),
    junctions: &HashSet<NodeIndex>,
    _theme: Theme,
) {
    let (left, top, right, bottom) = viewport_bounds;
    let margin = 200.0; // Buffer to include lines slightly outside viewport

    // Sort lines for consistent z-order to prevent z-fighting at junctions
    let mut sorted_lines: Vec<&Line> = lines.iter().collect();
    sorted_lines.sort_by(|a, b| {
        // First sort by sort_index (None values go to end)
        match (a.sort_index, b.sort_index) {
            (Some(a_idx), Some(b_idx)) => a_idx.partial_cmp(&b_idx).unwrap_or(std::cmp::Ordering::Equal),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        // Then sort by ID as tiebreaker
        .then_with(|| a.id.cmp(&b.id))
    });

    // Identify sections and order lines within each section
    let sections = identify_sections(graph, junctions);
    let section_lines = get_lines_in_section_sorted(&sections, &sorted_lines);

    // Gap width for spacing between lines
    let gap_width = (LINE_BASE_WIDTH + 2.0) / zoom;

    // Group lines by edge for rendering (needed for position assignment)
    let mut edge_to_lines: IndexMap<EdgeIndex, Vec<&Line>> = IndexMap::new();

    // Group lines by junction connections for drawing through junctions
    let mut junction_connections: IndexMap<JunctionConnectionKey, Vec<&Line>> = IndexMap::new();

    // Build mappings for visible lines
    for line in &sorted_lines {
        if !line.visible {
            continue;
        }

        for segment in &line.forward_route {
            let edge_idx = EdgeIndex::new(segment.edge_index);
            edge_to_lines.entry(edge_idx).or_default().push(line);
        }

        // Build junction connection map
        for i in 0..line.forward_route.len().saturating_sub(1) {
            let from_segment = &line.forward_route[i];
            let to_segment = &line.forward_route[i + 1];

            let from_edge_idx = EdgeIndex::new(from_segment.edge_index);
            let to_edge_idx = EdgeIndex::new(to_segment.edge_index);

            // Find the shared node between these edges
            let (Some((from_src, from_tgt)), Some((to_src, to_tgt))) = (
                graph.graph.edge_endpoints(from_edge_idx),
                graph.graph.edge_endpoints(to_edge_idx)
            ) else {
                continue;
            };

            let shared_node = [
                (from_tgt == to_src || from_tgt == to_tgt, from_tgt),
                (from_src == to_src || from_src == to_tgt, from_src),
            ]
            .into_iter()
            .find_map(|(is_shared, node)| is_shared.then_some(node));

            // If shared node is a junction, record this connection
            if let Some(junction) = shared_node.filter(|n| junctions.contains(n)) {
                let key = JunctionConnectionKey {
                    junction,
                    from_edge: from_edge_idx,
                    to_edge: to_edge_idx,
                };
                junction_connections.entry(key).or_default().push(line);
            }
        }
    }

    // Order lines within each section by stopping behavior in shared segments
    let mut section_orderings: HashMap<SectionId, Vec<&Line>> = HashMap::new();
    for section in &sections {
        if let Some(lines_in_section) = section_lines.get(&section.id) {
            let ordered = order_lines_for_section(lines_in_section, &section.edges);
            section_orderings.insert(section.id, ordered);
        }
    }

    // Assign visual positions with reuse for each section
    let mut section_visual_positions: HashMap<SectionId, HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>>> = HashMap::new();
    for section in &sections {
        if let Some(ordering) = section_orderings.get(&section.id) {
            let visual_positions = assign_visual_positions_with_reuse(section, ordering, &edge_to_lines, graph);
            section_visual_positions.insert(section.id, visual_positions);
        }
    }

    // Create mapping from edge to section
    let mut edge_to_section: HashMap<EdgeIndex, SectionId> = HashMap::new();
    for section in &sections {
        for &edge_idx in &section.edges {
            edge_to_section.insert(edge_idx, section.id);
        }
    }

    // Draw each edge's lines
    for (edge_idx, edge_lines) in &edge_to_lines {
        // Get edge endpoints
        let Some((source, target)) = graph.graph.edge_endpoints(*edge_idx) else {
            continue;
        };

        let Some(pos1) = graph.get_station_position(source) else { continue };
        let Some(pos2) = graph.get_station_position(target) else { continue };

        // Viewport culling: skip lines completely outside visible area
        let min_x = pos1.0.min(pos2.0);
        let max_x = pos1.0.max(pos2.0);
        let min_y = pos1.1.min(pos2.1);
        let max_y = pos1.1.max(pos2.1);

        if max_x < left - margin || min_x > right + margin ||
           max_y < top - margin || min_y > bottom + margin {
            continue;
        }

        // Check if source or target is a junction
        let source_is_junction = junctions.contains(&source);
        let target_is_junction = junctions.contains(&target);

        // Use cached avoidance offset
        let (avoid_x, avoid_y) = cached_avoidance.get(edge_idx).copied().unwrap_or((0.0, 0.0));
        let needs_avoidance = avoid_x.abs() > AVOIDANCE_OFFSET_THRESHOLD || avoid_y.abs() > AVOIDANCE_OFFSET_THRESHOLD;

        // Calculate actual start and end points, stopping before junctions
        let mut actual_pos1 = pos1;
        let mut actual_pos2 = pos2;

        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();

        // When there's avoidance offset, use half junction distance to match junction renderer
        let junction_distance = if needs_avoidance {
            JUNCTION_STOP_DISTANCE * 0.5
        } else {
            JUNCTION_STOP_DISTANCE
        };

        if source_is_junction && len > junction_distance {
            // Move start point away from junction
            let t = junction_distance / len;
            actual_pos1 = (pos1.0 + dx * t, pos1.1 + dy * t);
        }

        if target_is_junction && len > junction_distance {
            // Move end point away from junction
            let t = junction_distance / len;
            actual_pos2 = (pos2.0 - dx * t, pos2.1 - dy * t);
        }

        // Calculate perpendicular offset for parallel lines
        let nx = -dy / len;
        let ny = dx / len;

        // Get section and visual positions for this edge
        let Some(&section_id) = edge_to_section.get(edge_idx) else {
            continue;
        };
        let Some(section_ordering) = section_orderings.get(&section_id) else {
            continue;
        };
        let Some(edge_visual_positions_map) = section_visual_positions.get(&section_id)
            .and_then(|section_map| section_map.get(edge_idx)) else {
            continue;
        };

        // Build list of (visual_position, line) tuples for lines on this edge
        let mut positioned_lines: Vec<(usize, &Line)> = edge_lines.iter()
            .filter_map(|line| {
                edge_visual_positions_map.get(&line.id).map(|&pos| (pos, *line))
            })
            .collect();
        // Sort by (sort_index, id) for z-order, not by visual_position
        // Visual position determines lateral offset, not drawing order
        positioned_lines.sort_by(|(_, a), (_, b)| {
            match (a.sort_index, b.sort_index) {
                (Some(a_idx), Some(b_idx)) => a_idx.partial_cmp(&b_idx).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
            .then_with(|| a.id.cmp(&b.id))
        });

        let line_count = positioned_lines.len();

        if line_count == 1 {
            let (visual_pos, line) = positioned_lines[0];

            // Calculate widths for all lines in section ordering (to maintain proper spacing)
            let section_line_widths: Vec<f64> = section_ordering.iter()
                .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
                .collect();

            let num_gaps = section_ordering.len().saturating_sub(1);
            let total_section_width: f64 = section_line_widths.iter().sum::<f64>()
                + (num_gaps as f64) * gap_width;

            // Calculate offset based on visual position (NOT compacted)
            let start_offset = -total_section_width / 2.0;
            let offset_sum: f64 = section_line_widths.iter().take(visual_pos)
                .map(|&width| width + gap_width)
                .sum();
            let line_world_width = section_line_widths.get(visual_pos)
                .copied()
                .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
            let offset = start_offset + offset_sum + line_world_width / 2.0;

            let ox = nx * offset;
            let oy = ny * offset;

            ctx.set_line_width(line_world_width);
            ctx.set_stroke_style_str(&line.color);
            ctx.begin_path();

            if needs_avoidance {
                // Draw segmented path: start -> offset section -> end
                let segment_length = ((actual_pos2.0 - actual_pos1.0).powi(2) + (actual_pos2.1 - actual_pos1.1).powi(2)).sqrt();

                // Check if we're connecting to junctions
                let start_needs_transition = !source_is_junction;
                let end_needs_transition = !target_is_junction;

                draw_line_segment_with_avoidance(
                    ctx, actual_pos1, actual_pos2, segment_length,
                    (ox, oy), (avoid_x, avoid_y),
                    (start_needs_transition, end_needs_transition)
                );
            } else {
                ctx.move_to(actual_pos1.0 + ox, actual_pos1.1 + oy);
                ctx.line_to(actual_pos2.0 + ox, actual_pos2.1 + oy);
            }

            ctx.stroke();
        } else {
            // Multiple lines - position them using visual positions
            // Calculate widths for all lines in section ordering (to maintain proper spacing with gaps)
            let section_line_widths: Vec<f64> = section_ordering.iter()
                .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
                .collect();

            let num_gaps = section_ordering.len().saturating_sub(1);
            let total_section_width: f64 = section_line_widths.iter().sum::<f64>()
                + (num_gaps as f64) * gap_width;
            let start_offset = -total_section_width / 2.0;

            for (visual_pos, line) in &positioned_lines {
                // Calculate offset based on visual position (NOT compacted - maintains gaps)
                let offset_sum: f64 = section_line_widths.iter().take(*visual_pos)
                    .map(|&width| width + gap_width)
                    .sum();
                let line_world_width = section_line_widths.get(*visual_pos)
                    .copied()
                    .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
                let offset = start_offset + offset_sum + line_world_width / 2.0;

                let ox = nx * offset;
                let oy = ny * offset;

                // Set line width (already calculated with zoom adjustment)
                ctx.set_line_width(line_world_width);
                ctx.set_stroke_style_str(&line.color);
                ctx.begin_path();

                if needs_avoidance {
                    // Draw segmented path with offset
                    let segment_length = ((actual_pos2.0 - actual_pos1.0).powi(2) + (actual_pos2.1 - actual_pos1.1).powi(2)).sqrt();

                    // Check if we're connecting to junctions
                    let start_needs_transition = !source_is_junction;
                    let end_needs_transition = !target_is_junction;

                    draw_line_segment_with_avoidance(
                        ctx, actual_pos1, actual_pos2, segment_length,
                        (ox, oy), (avoid_x, avoid_y),
                        (start_needs_transition, end_needs_transition)
                    );
                } else {
                    ctx.move_to(actual_pos1.0 + ox, actual_pos1.1 + oy);
                    ctx.line_to(actual_pos2.0 + ox, actual_pos2.1 + oy);
                }

                ctx.stroke();
            }
        }
    }

    // Draw junction connections
    draw_junction_connections(
        ctx,
        graph,
        &junction_connections,
        &edge_to_lines,
        &edge_to_section,
        &section_orderings,
        &section_visual_positions,
        gap_width,
        zoom,
    );
}
