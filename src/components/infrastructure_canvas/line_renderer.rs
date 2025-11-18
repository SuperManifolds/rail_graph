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
const MIN_CURVE_RADIUS: f64 = 20.0;

/// Check if a station is a terminal (first or last stop) for a given line
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

/// Get the background color for the current theme
fn get_background_color(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "#0a0a0a",
        Theme::Light => "#fafafa",
    }
}

/// Get the selection highlight color for the current theme
fn get_selection_color(theme: Theme) -> &'static str {
    match theme {
        Theme::Dark => "#ffaa00",
        Theme::Light => "#ff8800",
    }
}

/// Stroke a line with a border to prevent color blending
/// If highlighted, uses selection color for border; otherwise uses background color
fn stroke_with_border(ctx: &CanvasRenderingContext2d, line_color: &str, line_width: f64, border_width: f64, theme: Theme, is_highlighted: bool) {
    // Only draw border if border_width > 0
    if border_width > 0.01 {
        // Draw border: wider stroke in background or selection color
        let border_color = if is_highlighted {
            get_selection_color(theme)
        } else {
            get_background_color(theme)
        };
        ctx.set_stroke_style_str(border_color);
        ctx.set_line_width(line_width + (2.0 * border_width));
        ctx.stroke();
    }

    // Draw actual line on top
    ctx.set_stroke_style_str(line_color);
    ctx.set_line_width(line_width);
    ctx.stroke();
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

/// Draw a single junction connection (helper for maintaining z-order)
#[allow(clippy::cast_precision_loss, clippy::too_many_lines, clippy::too_many_arguments)]
fn draw_single_junction_connection(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    connection_key: &JunctionConnectionKey,
    connection_lines: &[&Line],
    edge_to_section: &HashMap<EdgeIndex, SectionId>,
    section_orderings: &HashMap<SectionId, Vec<&Line>>,
    section_visual_positions: &HashMap<SectionId, HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>>>,
    gap_width: f64,
    zoom: f64,
    theme: Theme,
    highlighted_edges: &HashSet<EdgeIndex>,
) {
    let Some(junction_pos) = graph.get_station_position(connection_key.junction) else {
        return;
    };

    // Get the endpoints of both edges
    let Some((from_src, from_tgt)) = graph.graph.edge_endpoints(connection_key.from_edge) else {
        return;
    };
    let Some((to_src, to_tgt)) = graph.graph.edge_endpoints(connection_key.to_edge) else {
        return;
    };

    // Determine the node positions for entry and exit edges
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
        return;
    };

    // Calculate edge directions
    let entry_delta = if from_junction_is_target {
        (junction_pos.0 - from_pos.0, junction_pos.1 - from_pos.1)
    } else {
        (from_pos.0 - junction_pos.0, from_pos.1 - junction_pos.1)
    };
    let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();

    let exit_delta = if to_junction_is_source {
        (to_pos.0 - junction_pos.0, to_pos.1 - junction_pos.1)
    } else {
        (junction_pos.0 - to_pos.0, junction_pos.1 - to_pos.1)
    };
    let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();

    if entry_distance < 0.1 || exit_distance < 0.1 {
        return;
    }

    // Base curve stop distance (will be adjusted per-line based on offset for proper offset curves)
    let curve_stop_distance = JUNCTION_STOP_DISTANCE - 2.0;

    // Calculate perpendicular vectors
    let entry_perp = (-entry_delta.1 / entry_distance, entry_delta.0 / entry_distance);
    let mut exit_perp = (-exit_delta.1 / exit_distance, exit_delta.0 / exit_distance);

    let dot_product = entry_perp.0 * exit_perp.0 + entry_perp.1 * exit_perp.1;
    let flip_exit_offsets = if dot_product < 0.0 {
        exit_perp = (-exit_perp.0, -exit_perp.1);
        true
    } else {
        false
    };

    // Get section orderings
    let Some(&entry_section_id) = edge_to_section.get(&connection_key.from_edge) else {
        return;
    };
    let Some(&exit_section_id) = edge_to_section.get(&connection_key.to_edge) else {
        return;
    };
    let Some(entry_section_ordering) = section_orderings.get(&entry_section_id) else {
        return;
    };
    let Some(exit_section_ordering) = section_orderings.get(&exit_section_id) else {
        return;
    };

    // Calculate widths
    let entry_section_widths: Vec<f64> = entry_section_ordering.iter()
        .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
        .collect();
    let entry_num_gaps = entry_section_ordering.len().saturating_sub(1);
    let entry_actual_width: f64 = entry_section_widths.iter().sum::<f64>()
        + (entry_num_gaps as f64) * gap_width;
    // Always center as if there's an odd number of lines
    let entry_total_width = if entry_section_ordering.len() % 2 == 0 {
        // Even number: add phantom line width + gap for centering
        entry_actual_width + entry_section_widths.last().copied().unwrap_or(0.0) + gap_width
    } else {
        entry_actual_width
    };

    let exit_section_widths: Vec<f64> = exit_section_ordering.iter()
        .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
        .collect();
    let exit_num_gaps = exit_section_ordering.len().saturating_sub(1);
    let exit_actual_width: f64 = exit_section_widths.iter().sum::<f64>()
        + (exit_num_gaps as f64) * gap_width;
    // Always center as if there's an odd number of lines
    let exit_total_width = if exit_section_ordering.len() % 2 == 0 {
        // Even number: add phantom line width + gap for centering
        exit_actual_width + exit_section_widths.last().copied().unwrap_or(0.0) + gap_width
    } else {
        exit_actual_width
    };

    // Get visual position maps
    let entry_visual_map = section_visual_positions.get(&entry_section_id)
        .and_then(|section_map| section_map.get(&connection_key.from_edge));
    let exit_visual_map = section_visual_positions.get(&exit_section_id)
        .and_then(|section_map| section_map.get(&connection_key.to_edge));

    // Calculate normalized directions for curves
    let entry_dir = (entry_delta.0 / entry_distance, entry_delta.1 / entry_distance);
    let exit_dir = (exit_delta.0 / exit_distance, exit_delta.1 / exit_distance);

    // Sort connection lines by z-order
    let mut sorted_connection_lines = connection_lines.to_vec();
    sorted_connection_lines.sort_by(|a, b| {
        match (a.sort_index, b.sort_index) {
            (Some(a_idx), Some(b_idx)) => a_idx.partial_cmp(&b_idx).unwrap_or(std::cmp::Ordering::Equal),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
        .then_with(|| a.id.cmp(&b.id))
    });

    // Draw each line through the junction
    for line in &sorted_connection_lines {
        let Some(&entry_visual_pos) = entry_visual_map.and_then(|map| map.get(&line.id)) else {
            continue;
        };

        let entry_start_offset = -entry_total_width / 2.0;
        let entry_offset_sum: f64 = entry_section_widths.iter().take(entry_visual_pos)
            .map(|&width| width + gap_width)
            .sum();
        let line_entry_width = entry_section_widths.get(entry_visual_pos)
            .copied()
            .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
        let mut entry_offset = entry_start_offset + entry_offset_sum + line_entry_width / 2.0;

        let Some(&exit_visual_pos) = exit_visual_map.and_then(|map| map.get(&line.id)) else {
            continue;
        };

        let exit_start_offset = -exit_total_width / 2.0;
        let exit_offset_sum: f64 = exit_section_widths.iter().take(exit_visual_pos)
            .map(|&width| width + gap_width)
            .sum();
        let line_exit_width = exit_section_widths.get(exit_visual_pos)
            .copied()
            .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
        let mut exit_offset = exit_start_offset + exit_offset_sum + line_exit_width / 2.0;

        if flip_exit_offsets {
            entry_offset = -entry_offset;
            exit_offset = -exit_offset;
        }

        let line_world_width = (line_entry_width + line_exit_width) / 2.0;

        // For parallel directions (S-curves), adjust stop distance based on offset
        // Lines with larger offsets need more room to transition smoothly
        let det_test = entry_dir.0 * (-exit_dir.1) - entry_dir.1 * (-exit_dir.0);
        let is_parallel = det_test.abs() <= 0.01;
        let avg_offset = (entry_offset.abs() + exit_offset.abs()) / 2.0;

        // Calculate base adjusted distance
        let base_adjusted = if is_parallel {
            curve_stop_distance + avg_offset * 0.75
        } else {
            curve_stop_distance
        };

        // Calculate minimum stop distance to maintain minimum curve radius
        let min_stop_distance = calculate_min_stop_distance_for_radius(
            entry_dir,
            exit_dir,
            MIN_CURVE_RADIUS,
            avg_offset
        );

        // Use the larger of the two to ensure both smooth transitions and minimum radius
        let adjusted_stop_distance = base_adjusted.max(min_stop_distance);

        // Calculate base points with adjusted stop distance
        let entry_base = if from_junction_is_target {
            (
                junction_pos.0 - entry_dir.0 * adjusted_stop_distance,
                junction_pos.1 - entry_dir.1 * adjusted_stop_distance,
            )
        } else {
            (
                junction_pos.0 + entry_dir.0 * adjusted_stop_distance,
                junction_pos.1 + entry_dir.1 * adjusted_stop_distance,
            )
        };

        let entry_point = (
            entry_base.0 + entry_perp.0 * entry_offset,
            entry_base.1 + entry_perp.1 * entry_offset
        );

        let exit_base = if to_junction_is_source {
            (
                junction_pos.0 + exit_dir.0 * adjusted_stop_distance,
                junction_pos.1 + exit_dir.1 * adjusted_stop_distance,
            )
        } else {
            (
                junction_pos.0 - exit_dir.0 * adjusted_stop_distance,
                junction_pos.1 - exit_dir.1 * adjusted_stop_distance,
            )
        };

        let exit_point = (
            exit_base.0 + exit_perp.0 * exit_offset,
            exit_base.1 + exit_perp.1 * exit_offset
        );

        // Junction connection is highlighted if both connecting edges are highlighted
        let is_highlighted = highlighted_edges.contains(&connection_key.from_edge)
            && highlighted_edges.contains(&connection_key.to_edge);

        // Use unified curve drawing function
        draw_curve(
            ctx,
            entry_point,
            exit_point,
            entry_dir,
            exit_dir,
            &line.color,
            line_world_width,
            gap_width * 0.2,
            theme,
            is_highlighted,
            false,  // draw_exit_cap - junctions don't draw exit cap (handled by outgoing edge)
            entry_offset,
            exit_offset,
        );
    }
}

/// Draw lines through junctions (OLD - replaced by `draw_single_junction_connection`)
#[allow(clippy::cast_precision_loss, clippy::too_many_lines, clippy::too_many_arguments, dead_code)]
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
    theme: Theme,
    highlighted_edges: &HashSet<EdgeIndex>,
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
        // Match edge stop distance to align perpendicular offsets
        let curve_stop_distance = JUNCTION_STOP_DISTANCE - 2.0;

        let entry_base_no_ext = if from_junction_is_target {
            (
                junction_pos.0 - (entry_delta.0 / entry_distance) * curve_stop_distance,
                junction_pos.1 - (entry_delta.1 / entry_distance) * curve_stop_distance,
            )
        } else {
            (
                junction_pos.0 + (entry_delta.0 / entry_distance) * curve_stop_distance,
                junction_pos.1 + (entry_delta.1 / entry_distance) * curve_stop_distance,
            )
        };

        let exit_base_no_ext = if to_junction_is_source {
            (
                junction_pos.0 + (exit_delta.0 / exit_distance) * curve_stop_distance,
                junction_pos.1 + (exit_delta.1 / exit_distance) * curve_stop_distance,
            )
        } else {
            (
                junction_pos.0 - (exit_delta.0 / exit_distance) * curve_stop_distance,
                junction_pos.1 - (exit_delta.1 / exit_distance) * curve_stop_distance,
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
        let entry_actual_width: f64 = entry_section_widths.iter().sum::<f64>()
            + (entry_num_gaps as f64) * gap_width;
        // Always center as if there's an odd number of lines
        let entry_total_width = if entry_section_ordering.len() % 2 == 0 {
            entry_actual_width + entry_section_widths.last().copied().unwrap_or(0.0) + gap_width
        } else {
            entry_actual_width
        };

        // Calculate widths for exit section
        let exit_section_widths: Vec<f64> = exit_section_ordering.iter()
            .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
            .collect();
        let exit_num_gaps = exit_section_ordering.len().saturating_sub(1);
        let exit_actual_width: f64 = exit_section_widths.iter().sum::<f64>()
            + (exit_num_gaps as f64) * gap_width;
        // Always center as if there's an odd number of lines
        let exit_total_width = if exit_section_ordering.len() % 2 == 0 {
            exit_actual_width + exit_section_widths.last().copied().unwrap_or(0.0) + gap_width
        } else {
            exit_actual_width
        };

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

            // Junction connection is highlighted if both connecting edges are highlighted
            let is_highlighted = highlighted_edges.contains(&connection_key.from_edge)
                && highlighted_edges.contains(&connection_key.to_edge);
            stroke_with_border(ctx, &line.color, line_world_width, gap_width * 0.2, theme, is_highlighted);
        }
    }
}

/// Calculate radially-scaled control point for 90-degree turns
/// Returns the offset control point, or None if radial scaling fails
fn calculate_radial_control_point(
    base_entry: (f64, f64),
    base_exit: (f64, f64),
    base_control: (f64, f64),
    entry_dir: (f64, f64),
    exit_dir: (f64, f64),
    perp_offset: f64,
) -> Option<(f64, f64)> {
    // Calculate arc center (intersection of perpendiculars from base entry/exit points)
    let entry_perp = (-entry_dir.1, entry_dir.0);
    let exit_perp = (-exit_dir.1, exit_dir.0);
    let perp_det = entry_perp.0 * exit_perp.1 - entry_perp.1 * exit_perp.0;

    if perp_det.abs() <= 0.01 {
        return None; // Perpendiculars are parallel
    }

    // Find intersection of perpendicular lines from base points
    let pdx = base_exit.0 - base_entry.0;
    let pdy = base_exit.1 - base_entry.1;
    let s = (pdx * exit_perp.1 - pdy * exit_perp.0) / perp_det;

    let arc_center = (
        base_entry.0 + s * entry_perp.0,
        base_entry.1 + s * entry_perp.1
    );

    // Calculate base radius
    let rdx = base_entry.0 - arc_center.0;
    let rdy = base_entry.1 - arc_center.1;
    let base_radius = (rdx * rdx + rdy * rdy).sqrt();

    if base_radius <= 0.1 {
        return None; // Radius too small
    }

    // Scale control point radially from arc center
    let offset_radius = base_radius + perp_offset;
    let scale = offset_radius / base_radius;

    Some((
        arc_center.0 + (base_control.0 - arc_center.0) * scale,
        arc_center.1 + (base_control.1 - arc_center.1) * scale
    ))
}

/// Calculate minimum stop distance required to achieve minimum curve radius
/// Uses geometry of circular arc approximation for quadratic Bezier curves
fn calculate_min_stop_distance_for_radius(
    entry_dir: (f64, f64),
    exit_dir: (f64, f64),
    min_radius: f64,
    avg_offset_abs: f64,
) -> f64 {
    // Calculate angle between entry and exit directions
    let cos_angle = entry_dir.0 * exit_dir.0 + entry_dir.1 * exit_dir.1;
    let angle = cos_angle.acos();

    // For very small angles (nearly straight), no constraint needed
    if angle < 0.01 {
        return 0.0;
    }

    // For a circular arc, radius r relates to stop distance d by:
    // r ≈ d / tan(θ/2)
    // Therefore: d ≈ r * tan(θ/2)
    // Account for perpendicular offset reducing effective radius
    let half_angle = angle / 2.0;
    let required_base_radius = min_radius + avg_offset_abs;
    required_base_radius * half_angle.tan()
}

#[allow(clippy::cast_precision_loss)]
fn calculate_max_offset_for_station_curve(
    sorted_lines: &[&Line],
    prev_edge: EdgeIndex,
    next_edge: EdgeIndex,
    edge_to_section: &HashMap<EdgeIndex, SectionId>,
    section_visual_positions: &HashMap<SectionId, HashMap<EdgeIndex, HashMap<uuid::Uuid, usize>>>,
    section_orderings: &HashMap<SectionId, Vec<&Line>>,
    gap_width: f64,
    zoom: f64,
) -> f64 {
    let mut max_offset = 0.0_f64;

    for check_line in sorted_lines {
        // Check if this line goes through the same curve
        let Some(i) = check_line.forward_route.iter().position(|seg| EdgeIndex::new(seg.edge_index) == prev_edge) else {
            continue;
        };
        if i + 1 >= check_line.forward_route.len() || EdgeIndex::new(check_line.forward_route[i + 1].edge_index) != next_edge {
            continue;
        }

        // This line goes through this curve - calculate its offset
        let Some(&check_section_id) = edge_to_section.get(&prev_edge) else {
            continue;
        };
        let Some(&check_visual_pos) = section_visual_positions.get(&check_section_id)
            .and_then(|section_map| section_map.get(&prev_edge))
            .and_then(|edge_map| edge_map.get(&check_line.id)) else {
            continue;
        };
        let Some(check_section_ordering) = section_orderings.get(&check_section_id) else {
            continue;
        };

        let check_section_widths: Vec<f64> = check_section_ordering.iter()
            .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
            .collect();
        let check_num_gaps = check_section_ordering.len().saturating_sub(1);
        let check_actual_width: f64 = check_section_widths.iter().sum::<f64>()
            + (check_num_gaps as f64) * gap_width;
        let check_total_width = if check_section_ordering.len() % 2 == 0 {
            check_actual_width + check_section_widths.last().copied().unwrap_or(0.0) + gap_width
        } else {
            check_actual_width
        };
        let check_start_offset = -check_total_width / 2.0;
        let check_offset_sum: f64 = check_section_widths.iter().take(check_visual_pos)
            .map(|&width| width + gap_width)
            .sum();
        let check_line_width = check_section_widths.get(check_visual_pos)
            .copied()
            .unwrap_or((LINE_BASE_WIDTH + check_line.thickness) / zoom);
        let check_perp_offset = check_start_offset + check_offset_sum + check_line_width / 2.0;
        max_offset = max_offset.max(check_perp_offset.abs());
    }

    max_offset
}

/// Draw a curve for a line at a station where direction changes
/// Uses the same curve algorithm as junction connections
#[allow(clippy::too_many_arguments)]
fn draw_curve(
    ctx: &CanvasRenderingContext2d,
    entry_point: (f64, f64),
    exit_point: (f64, f64),
    entry_dir: (f64, f64),
    exit_dir: (f64, f64),
    line_color: &str,
    line_width: f64,
    border_width: f64,
    theme: Theme,
    is_highlighted: bool,
    draw_exit_cap: bool,
    entry_offset: f64,  // Perpendicular offset at entry
    exit_offset: f64,   // Perpendicular offset at exit
) {
    ctx.set_line_width(line_width);
    ctx.set_stroke_style_str(line_color);
    ctx.begin_path();
    ctx.move_to(entry_point.0, entry_point.1);

    // Calculate exit direction going backwards (same as junction logic)
    let exit_dir_back = (-exit_dir.0, -exit_dir.1);
    let det = entry_dir.0 * exit_dir_back.1 - entry_dir.1 * exit_dir_back.0;

    // Calculate perpendicular vectors (same as junction code)
    let entry_perp = (-entry_dir.1, entry_dir.0);
    let mut exit_perp = (-exit_dir.1, exit_dir.0);

    // Check if perpendiculars point in opposite directions and flip if needed
    let dot_product = entry_perp.0 * exit_perp.0 + entry_perp.1 * exit_perp.1;
    if dot_product < 0.0 {
        exit_perp = (-exit_perp.0, -exit_perp.1);
    }

    // Check if perpendiculars are well-aligned (stations) or different (junctions)
    // If they're well-aligned and offsets are similar, we can use radial scaling
    let perps_aligned = (entry_perp.0 - exit_perp.0).abs() < 0.01 && (entry_perp.1 - exit_perp.1).abs() < 0.01;
    let offsets_similar = (entry_offset - exit_offset).abs() < 0.1;

    if det.abs() > 0.01 {
        // Directions not parallel - use quadratic curve
        // For proper offset curves, calculate base control point then offset it

        if perps_aligned && offsets_similar {
            // Stations: use radial scaling for proper offset curves
            let avg_offset = (entry_offset + exit_offset) / 2.0;

            // Calculate base points by removing offset
            let base_entry = (entry_point.0 - entry_perp.0 * avg_offset, entry_point.1 - entry_perp.1 * avg_offset);
            let base_exit = (exit_point.0 - exit_perp.0 * avg_offset, exit_point.1 - exit_perp.1 * avg_offset);

            let dx = base_exit.0 - base_entry.0;
            let dy = base_exit.1 - base_entry.1;
            let t = (dx * exit_dir_back.1 - dy * exit_dir_back.0) / det;

            if t > 0.0 {
                let base_control = (
                    base_entry.0 + t * entry_dir.0,
                    base_entry.1 + t * entry_dir.1
                );

                let cos_angle = entry_dir.0 * exit_dir.0 + entry_dir.1 * exit_dir.1;
                let angle = cos_angle.acos();
                let use_radial_scaling = angle > 0.35 && angle < 2.79 && avg_offset.abs() > 0.1;

                let control_point = if use_radial_scaling {
                    calculate_radial_control_point(
                        base_entry,
                        base_exit,
                        base_control,
                        entry_dir,
                        exit_dir,
                        avg_offset
                    ).unwrap_or((
                        base_control.0 + entry_perp.0 * avg_offset,
                        base_control.1 + entry_perp.1 * avg_offset
                    ))
                } else {
                    (
                        base_control.0 + entry_perp.0 * avg_offset,
                        base_control.1 + entry_perp.1 * avg_offset
                    )
                };

                ctx.quadratic_curve_to(control_point.0, control_point.1, exit_point.0, exit_point.1);
            } else {
                ctx.line_to(exit_point.0, exit_point.1);
            }
        } else {
            // Junctions with different perpendiculars: use simple intersection
            let dx = exit_point.0 - entry_point.0;
            let dy = exit_point.1 - entry_point.1;
            let t = (dx * exit_dir_back.1 - dy * exit_dir_back.0) / det;

            if t > 0.0 {
                let control_point = (
                    entry_point.0 + t * entry_dir.0,
                    entry_point.1 + t * entry_dir.1
                );
                ctx.quadratic_curve_to(control_point.0, control_point.1, exit_point.0, exit_point.1);
            } else {
                ctx.line_to(exit_point.0, exit_point.1);
            }
        }
    } else {
        // Directions parallel - use S-curve (cubic bezier)
        // For proper offset curves, calculate base control points then offset them perpendicular
        let avg_offset = (entry_offset + exit_offset) / 2.0;
        let control_dist = 15.0;

        // Check if we can use offset control points for S-curves
        if perps_aligned && offsets_similar {
            // Calculate base control points (centerline)
            let base_entry = (entry_point.0 - entry_perp.0 * avg_offset, entry_point.1 - entry_perp.1 * avg_offset);
            let base_exit = (exit_point.0 - exit_perp.0 * avg_offset, exit_point.1 - exit_perp.1 * avg_offset);

            let base_cp1 = (
                base_entry.0 + entry_dir.0 * control_dist,
                base_entry.1 + entry_dir.1 * control_dist
            );
            let base_cp2 = (
                base_exit.0 + exit_dir_back.0 * control_dist,
                base_exit.1 + exit_dir_back.1 * control_dist
            );

            // Offset control points perpendicular
            let cp1 = (
                base_cp1.0 + entry_perp.0 * avg_offset,
                base_cp1.1 + entry_perp.1 * avg_offset
            );
            let cp2 = (
                base_cp2.0 + exit_perp.0 * avg_offset,
                base_cp2.1 + exit_perp.1 * avg_offset
            );

            ctx.bezier_curve_to(cp1.0, cp1.1, cp2.0, cp2.1, exit_point.0, exit_point.1);
        } else {
            // Simple S-curve from offset points
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
    }

    stroke_with_border(ctx, line_color, line_width, border_width, theme, is_highlighted);

    // Draw caps at entry and exit points to cover gaps
    let cap_radius = line_width / 2.0;
    ctx.set_fill_style_str(line_color);

    // Cap at entry point
    ctx.begin_path();
    let _ = ctx.arc(entry_point.0, entry_point.1, cap_radius, 0.0, 2.0 * std::f64::consts::PI);
    ctx.fill();

    // Cap at exit point (conditional)
    if draw_exit_cap {
        ctx.begin_path();
        let _ = ctx.arc(exit_point.0, exit_point.1, cap_radius, 0.0, 2.0 * std::f64::consts::PI);
        ctx.fill();
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

#[allow(clippy::cast_precision_loss, clippy::too_many_lines, clippy::too_many_arguments)]
pub fn draw_lines(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    lines: &[Line],
    zoom: f64,
    cached_avoidance: &HashMap<EdgeIndex, (f64, f64)>,
    viewport_bounds: (f64, f64, f64, f64),
    junctions: &HashSet<NodeIndex>,
    theme: Theme,
    highlighted_edges: &HashSet<EdgeIndex>,
    line_gap_width: f64,
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
    let gap_width = line_gap_width / zoom;

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

    // Track which junction connections have been drawn to avoid duplicates
    let mut drawn_junctions: HashSet<JunctionConnectionKey> = HashSet::new();

    // Pre-calculate adjusted stop distances for each line at each junction
    // Maps (edge, line_id, junction_node) -> adjusted_stop_distance
    let mut junction_stop_distances: HashMap<(EdgeIndex, uuid::Uuid, NodeIndex), f64> = HashMap::new();

    for (connection_key, connection_lines) in &junction_connections {
        let Some(junction_pos) = graph.get_station_position(connection_key.junction) else {
            continue;
        };

        let Some((from_src, from_tgt)) = graph.graph.edge_endpoints(connection_key.from_edge) else {
            continue;
        };
        let Some((to_src, to_tgt)) = graph.graph.edge_endpoints(connection_key.to_edge) else {
            continue;
        };

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

        let entry_delta = if from_junction_is_target {
            (junction_pos.0 - from_pos.0, junction_pos.1 - from_pos.1)
        } else {
            (from_pos.0 - junction_pos.0, from_pos.1 - junction_pos.1)
        };
        let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();

        let exit_delta = if to_junction_is_source {
            (to_pos.0 - junction_pos.0, to_pos.1 - junction_pos.1)
        } else {
            (junction_pos.0 - to_pos.0, junction_pos.1 - to_pos.1)
        };
        let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();

        if entry_distance < 0.1 || exit_distance < 0.1 {
            continue;
        }

        let entry_dir = (entry_delta.0 / entry_distance, entry_delta.1 / entry_distance);
        let exit_dir = (exit_delta.0 / exit_distance, exit_delta.1 / exit_distance);

        // Get section info for calculating offsets
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

        let entry_section_widths: Vec<f64> = entry_section_ordering.iter()
            .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
            .collect();
        let exit_section_widths: Vec<f64> = exit_section_ordering.iter()
            .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
            .collect();

        let entry_visual_map = section_visual_positions.get(&entry_section_id)
            .and_then(|section_map| section_map.get(&connection_key.from_edge));
        let exit_visual_map = section_visual_positions.get(&exit_section_id)
            .and_then(|section_map| section_map.get(&connection_key.to_edge));

        let entry_num_gaps = entry_section_ordering.len().saturating_sub(1);
        let entry_actual_width: f64 = entry_section_widths.iter().sum::<f64>()
            + (entry_num_gaps as f64) * gap_width;
        let entry_total_width = if entry_section_ordering.len() % 2 == 0 {
            entry_actual_width + entry_section_widths.last().copied().unwrap_or(0.0) + gap_width
        } else {
            entry_actual_width
        };

        let exit_num_gaps = exit_section_ordering.len().saturating_sub(1);
        let exit_actual_width: f64 = exit_section_widths.iter().sum::<f64>()
            + (exit_num_gaps as f64) * gap_width;
        let exit_total_width = if exit_section_ordering.len() % 2 == 0 {
            exit_actual_width + exit_section_widths.last().copied().unwrap_or(0.0) + gap_width
        } else {
            exit_actual_width
        };

        // Calculate adjusted stop distance for each line
        for line in connection_lines {
            let Some(&entry_visual_pos) = entry_visual_map.and_then(|map| map.get(&line.id)) else {
                continue;
            };
            let Some(&exit_visual_pos) = exit_visual_map.and_then(|map| map.get(&line.id)) else {
                continue;
            };

            let entry_start_offset = -entry_total_width / 2.0;
            let entry_offset_sum: f64 = entry_section_widths.iter().take(entry_visual_pos)
                .map(|&width| width + gap_width)
                .sum();
            let line_entry_width = entry_section_widths.get(entry_visual_pos)
                .copied()
                .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
            let mut entry_offset = entry_start_offset + entry_offset_sum + line_entry_width / 2.0;

            let exit_start_offset = -exit_total_width / 2.0;
            let exit_offset_sum: f64 = exit_section_widths.iter().take(exit_visual_pos)
                .map(|&width| width + gap_width)
                .sum();
            let line_exit_width = exit_section_widths.get(exit_visual_pos)
                .copied()
                .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
            let mut exit_offset = exit_start_offset + exit_offset_sum + line_exit_width / 2.0;

            // Check if we need to flip offsets
            let flip_exit_offsets = {
                let entry_perp = (-entry_dir.1, entry_dir.0);
                let exit_perp = (-exit_dir.1, exit_dir.0);
                let dot_product = entry_perp.0 * exit_perp.0 + entry_perp.1 * exit_perp.1;
                dot_product < 0.0
            };

            if flip_exit_offsets {
                entry_offset = -entry_offset;
                exit_offset = -exit_offset;
            }

            // Calculate adjusted stop distance for parallel directions (S-curves)
            // Match the curve_stop_distance calculation in draw_single_junction_connection
            let curve_stop_distance = JUNCTION_STOP_DISTANCE - 2.0;
            let det_test = entry_dir.0 * (-exit_dir.1) - entry_dir.1 * (-exit_dir.0);
            let is_parallel = det_test.abs() <= 0.01;
            let avg_offset = (entry_offset.abs() + exit_offset.abs()) / 2.0;

            // Calculate base adjusted distance
            let base_adjusted = if is_parallel {
                curve_stop_distance + avg_offset * 0.75
            } else {
                curve_stop_distance
            };

            // Calculate minimum stop distance to maintain minimum curve radius
            let min_stop_distance = calculate_min_stop_distance_for_radius(
                entry_dir,
                exit_dir,
                MIN_CURVE_RADIUS,
                avg_offset
            );

            // Use the larger of the two to ensure both smooth transitions and minimum radius
            let adjusted_stop_distance = base_adjusted.max(min_stop_distance);

            // Store adjusted stop distance for both edges
            junction_stop_distances.insert(
                (connection_key.from_edge, line.id, connection_key.junction),
                adjusted_stop_distance
            );
            junction_stop_distances.insert(
                (connection_key.to_edge, line.id, connection_key.junction),
                adjusted_stop_distance
            );
        }
    }

    // Pre-calculate station curve stop distances for track segment alignment
    // Maps (edge, line_id, station_node) -> curve_stop_distance
    let mut station_curve_stop_distances: HashMap<(EdgeIndex, uuid::Uuid, NodeIndex), f64> = HashMap::new();

    // Cache for station curve stop distances per curve (not per line)
    // Maps (prev_edge, next_edge, station_node) -> curve_stop_distance
    let mut station_curve_stops: HashMap<(EdgeIndex, EdgeIndex, NodeIndex), f64> = HashMap::new();

    // Pre-identify edges that will have station curves (edge, station_node)
    // Used to shorten edges appropriately before drawing curves
    let mut edges_with_station_curves: HashSet<(EdgeIndex, NodeIndex)> = HashSet::new();
    for line in &sorted_lines {
        if line.forward_route.len() < 2 {
            continue;
        }

        for i in 0..line.forward_route.len() - 1 {
            let prev_segment = &line.forward_route[i];
            let next_segment = &line.forward_route[i + 1];

            let prev_edge = EdgeIndex::new(prev_segment.edge_index);
            let next_edge = EdgeIndex::new(next_segment.edge_index);

            let Some((prev_src, prev_tgt)) = graph.graph.edge_endpoints(prev_edge) else {
                continue;
            };
            let Some((next_src, next_tgt)) = graph.graph.edge_endpoints(next_edge) else {
                continue;
            };

            // Find the connecting station
            let station_idx = if prev_tgt == next_src {
                prev_tgt
            } else if prev_src == next_tgt {
                prev_src
            } else {
                continue;
            };

            // Skip if it's a junction
            if junctions.contains(&station_idx) {
                continue;
            }

            // Skip if it's a passing loop (curves should extend through them)
            let is_passing_loop = graph.graph.node_weight(station_idx)
                .and_then(|n| n.as_station())
                .is_some_and(|s| s.passing_loop);
            if is_passing_loop {
                continue;
            }

            // Mark both edges as having curves at this station
            edges_with_station_curves.insert((prev_edge, station_idx));
            edges_with_station_curves.insert((next_edge, station_idx));
        }
    }

    // Draw station curves for lines with direction changes (before edges/junctions for z-order)
    for line in &sorted_lines {
        if line.forward_route.len() < 2 {
            continue;
        }

        // Check consecutive edges for direction changes at stations
        for i in 0..line.forward_route.len() - 1 {
            let prev_segment = &line.forward_route[i];
            let next_segment = &line.forward_route[i + 1];

            let prev_edge = EdgeIndex::new(prev_segment.edge_index);
            let next_edge = EdgeIndex::new(next_segment.edge_index);

            let Some((prev_src, prev_tgt)) = graph.graph.edge_endpoints(prev_edge) else {
                continue;
            };
            let Some((next_src, next_tgt)) = graph.graph.edge_endpoints(next_edge) else {
                continue;
            };

            // Find the connecting station
            let station_idx = if prev_tgt == next_src {
                prev_tgt
            } else if prev_src == next_tgt {
                prev_src
            } else {
                continue; // Edges don't connect
            };

            // Skip if it's a junction (already handled)
            if junctions.contains(&station_idx) {
                continue;
            }

            // Skip if it's a passing loop (curves should extend through them)
            let is_passing_loop = graph.graph.node_weight(station_idx)
                .and_then(|n| n.as_station())
                .is_some_and(|s| s.passing_loop);
            if is_passing_loop {
                continue;
            }

            let Some(station_pos) = graph.get_station_position(station_idx) else {
                continue;
            };

            // Calculate entry and exit positions (other end of each edge)
            let entry_node = if prev_tgt == station_idx { prev_src } else { prev_tgt };
            let exit_node = if next_src == station_idx { next_tgt } else { next_src };

            let Some(entry_pos) = graph.get_station_position(entry_node) else {
                continue;
            };
            let Some(exit_pos) = graph.get_station_position(exit_node) else {
                continue;
            };

            // Calculate entry direction (towards station)
            let entry_delta_x = station_pos.0 - entry_pos.0;
            let entry_delta_y = station_pos.1 - entry_pos.1;
            let entry_len = (entry_delta_x * entry_delta_x + entry_delta_y * entry_delta_y).sqrt();
            if entry_len == 0.0 {
                continue;
            }
            let entry_dir = (entry_delta_x / entry_len, entry_delta_y / entry_len);

            // Calculate exit direction (away from station)
            let exit_delta_x = exit_pos.0 - station_pos.0;
            let exit_delta_y = exit_pos.1 - station_pos.1;
            let exit_len = (exit_delta_x * exit_delta_x + exit_delta_y * exit_delta_y).sqrt();
            if exit_len == 0.0 {
                continue;
            }
            let exit_dir = (exit_delta_x / exit_len, exit_delta_y / exit_len);

            // Calculate perpendicular vectors for entry and exit
            let entry_perp = (-entry_delta_y / entry_len, entry_delta_x / entry_len);
            let mut exit_perp = (-exit_delta_y / exit_len, exit_delta_x / exit_len);

            // Check if perpendiculars point in opposite directions and flip if needed
            let dot_product = entry_perp.0 * exit_perp.0 + entry_perp.1 * exit_perp.1;
            if dot_product < 0.0 {
                exit_perp = (-exit_perp.0, -exit_perp.1);
            }

            // Get perpendicular offset for this line (use prev_edge for consistency)
            let Some(&section_id) = edge_to_section.get(&prev_edge) else {
                continue;
            };
            let visual_pos = section_visual_positions.get(&section_id)
                .and_then(|section_map| section_map.get(&prev_edge))
                .and_then(|edge_map| edge_map.get(&line.id))
                .copied()
                .unwrap_or(0);

            let Some(section_ordering) = section_orderings.get(&section_id) else {
                continue;
            };

            let section_line_widths: Vec<f64> = section_ordering.iter()
                .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
                .collect();

            let num_gaps = section_ordering.len().saturating_sub(1);
            let actual_section_width: f64 = section_line_widths.iter().sum::<f64>()
                + (num_gaps as f64) * gap_width;
            let total_section_width = if section_ordering.len() % 2 == 0 {
                actual_section_width + section_line_widths.last().copied().unwrap_or(0.0) + gap_width
            } else {
                actual_section_width
            };

            let start_offset = -total_section_width / 2.0;
            let offset_sum: f64 = section_line_widths.iter().take(visual_pos)
                .map(|&width| width + gap_width)
                .sum();
            let line_world_width = section_line_widths.get(visual_pos)
                .copied()
                .unwrap_or((LINE_BASE_WIDTH + line.thickness) / zoom);
            let perp_offset = start_offset + offset_sum + line_world_width / 2.0;

            // Calculate curve stop distance (once per unique station curve)
            let curve_key = (prev_edge, next_edge, station_idx);
            let curve_stop = *station_curve_stops.entry(curve_key).or_insert_with(|| {
                // Calculate maximum offset for all lines going through this curve
                let max_offset = calculate_max_offset_for_station_curve(
                    &sorted_lines,
                    prev_edge,
                    next_edge,
                    &edge_to_section,
                    &section_visual_positions,
                    &section_orderings,
                    gap_width,
                    zoom
                );

                // Calculate stop distance based on maximum offset (innermost curve)
                let base_curve_stop = JUNCTION_STOP_DISTANCE - 2.0;
                let min_stop_distance = calculate_min_stop_distance_for_radius(
                    entry_dir,
                    exit_dir,
                    MIN_CURVE_RADIUS,
                    max_offset
                );
                base_curve_stop.max(min_stop_distance)
            });

            // Store adjusted stop distance for both edges
            station_curve_stop_distances.insert((prev_edge, line.id, station_idx), curve_stop);
            station_curve_stop_distances.insert((next_edge, line.id, station_idx), curve_stop);

            let entry_point = (
                station_pos.0 - entry_dir.0 * curve_stop + entry_perp.0 * perp_offset,
                station_pos.1 - entry_dir.1 * curve_stop + entry_perp.1 * perp_offset
            );

            let exit_point = (
                station_pos.0 + exit_dir.0 * curve_stop + exit_perp.0 * perp_offset,
                station_pos.1 + exit_dir.1 * curve_stop + exit_perp.1 * perp_offset
            );

            // Check if highlighted (both edges must be highlighted)
            let is_highlighted = highlighted_edges.contains(&prev_edge)
                && highlighted_edges.contains(&next_edge);

            // Draw the curve
            draw_curve(
                ctx,
                entry_point,
                exit_point,
                entry_dir,
                exit_dir,
                &line.color,
                line_world_width,
                gap_width * 0.2,
                theme,
                is_highlighted,
                true,  // draw_exit_cap - stations need both entry and exit caps
                perp_offset,  // entry_offset
                perp_offset,  // exit_offset - same for radial scaling
            );
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

        // Check if this edge has station curves at either end
        let source_has_curve = edges_with_station_curves.contains(&(*edge_idx, source));
        let target_has_curve = edges_with_station_curves.contains(&(*edge_idx, target));

        // Use cached avoidance offset
        let (avoid_x, avoid_y) = cached_avoidance.get(edge_idx).copied().unwrap_or((0.0, 0.0));
        let needs_avoidance = avoid_x.abs() > AVOIDANCE_OFFSET_THRESHOLD || avoid_y.abs() > AVOIDANCE_OFFSET_THRESHOLD;

        let dx = pos2.0 - pos1.0;
        let dy = pos2.1 - pos1.1;
        let len = (dx * dx + dy * dy).sqrt();

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
            let actual_section_width: f64 = section_line_widths.iter().sum::<f64>()
                + (num_gaps as f64) * gap_width;
            // Always center as if there's an odd number of lines
            let total_section_width = if section_ordering.len() % 2 == 0 {
                actual_section_width + section_line_widths.last().copied().unwrap_or(0.0) + gap_width
            } else {
                actual_section_width
            };

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

            // Calculate actual start and end points, stopping before junctions or station curves
            let mut line_pos1 = pos1;
            let mut line_pos2 = pos2;

            // Get line-specific adjusted stop distance at source junction/station
            // Note: stored distances already have -2.0 applied
            let source_stop_distance = if source_is_junction {
                junction_stop_distances.get(&(*edge_idx, line.id, source))
                    .copied()
                    .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
            } else if source_has_curve {
                station_curve_stop_distances.get(&(*edge_idx, line.id, source))
                    .copied()
                    .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
            } else {
                JUNCTION_STOP_DISTANCE - 2.0
            };

            // Get line-specific adjusted stop distance at target junction/station
            // Note: stored distances already have -2.0 applied
            let target_stop_distance = if target_is_junction {
                junction_stop_distances.get(&(*edge_idx, line.id, target))
                    .copied()
                    .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
            } else if target_has_curve {
                station_curve_stop_distances.get(&(*edge_idx, line.id, target))
                    .copied()
                    .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
            } else {
                JUNCTION_STOP_DISTANCE - 2.0
            };

            // When there's avoidance offset, use half junction distance
            let source_distance = if needs_avoidance && source_is_junction {
                source_stop_distance * 0.5
            } else {
                source_stop_distance
            };
            let target_distance = if needs_avoidance && target_is_junction {
                target_stop_distance * 0.5
            } else {
                target_stop_distance
            };

            if (source_is_junction || source_has_curve) && len > source_distance {
                // Move start point away from junction or station curve
                let t = source_distance / len;
                line_pos1 = (pos1.0 + dx * t, pos1.1 + dy * t);
            }

            if (target_is_junction || target_has_curve) && len > target_distance {
                // Move end point away from junction or station curve
                let t = target_distance / len;
                line_pos2 = (pos2.0 - dx * t, pos2.1 - dy * t);
            }

            // Check if source is terminal for this line
            // Only extend to center if this specific line doesn't have a curve at this station
            let source_has_curve_for_this_line = station_curve_stop_distances.contains_key(&(*edge_idx, line.id, source));
            if !source_is_junction && !source_has_curve_for_this_line && is_line_terminal(source, line, graph) {
                // Extend back to station center
                line_pos1 = (pos1.0, pos1.1);
            }

            // Check if target is terminal for this line
            // Only extend to center if this specific line doesn't have a curve at this station
            let target_has_curve_for_this_line = station_curve_stop_distances.contains_key(&(*edge_idx, line.id, target));
            if !target_is_junction && !target_has_curve_for_this_line && is_line_terminal(target, line, graph) {
                // Extend back to station center
                line_pos2 = (pos2.0, pos2.1);
            }

            ctx.set_line_width(line_world_width);
            ctx.set_stroke_style_str(&line.color);
            ctx.begin_path();

            if needs_avoidance {
                // Draw segmented path: start -> offset section -> end
                let segment_length = ((line_pos2.0 - line_pos1.0).powi(2) + (line_pos2.1 - line_pos1.1).powi(2)).sqrt();

                // Check if we're connecting to junctions
                let start_needs_transition = !source_is_junction;
                let end_needs_transition = !target_is_junction;

                draw_line_segment_with_avoidance(
                    ctx, line_pos1, line_pos2, segment_length,
                    (ox, oy), (avoid_x, avoid_y),
                    (start_needs_transition, end_needs_transition)
                );
            } else {
                ctx.move_to(line_pos1.0 + ox, line_pos1.1 + oy);
                ctx.line_to(line_pos2.0 + ox, line_pos2.1 + oy);
            }

            let is_highlighted = highlighted_edges.contains(edge_idx);
            stroke_with_border(ctx, &line.color, line_world_width, gap_width * 0.2, theme, is_highlighted);

            // Draw caps at junction/station endpoints to cover rendering gaps
            let cap_radius = line_world_width / 2.0;
            // Draw cap at source (whether junction or station)
            ctx.begin_path();
            let _ = ctx.arc(line_pos1.0 + ox, line_pos1.1 + oy, cap_radius, 0.0, 2.0 * std::f64::consts::PI);
            ctx.set_fill_style_str(&line.color);
            ctx.fill();

            // Draw cap at target (whether junction or station)
            ctx.begin_path();
            let _ = ctx.arc(line_pos2.0 + ox, line_pos2.1 + oy, cap_radius, 0.0, 2.0 * std::f64::consts::PI);
            ctx.set_fill_style_str(&line.color);
            ctx.fill();
        } else {
            // Multiple lines - position them using visual positions
            // Calculate widths for all lines in section ordering (to maintain proper spacing with gaps)
            let section_line_widths: Vec<f64> = section_ordering.iter()
                .map(|l| (LINE_BASE_WIDTH + l.thickness) / zoom)
                .collect();

            let num_gaps = section_ordering.len().saturating_sub(1);
            let actual_section_width: f64 = section_line_widths.iter().sum::<f64>()
                + (num_gaps as f64) * gap_width;
            // Always center as if there's an odd number of lines
            let total_section_width = if section_ordering.len() % 2 == 0 {
                actual_section_width + section_line_widths.last().copied().unwrap_or(0.0) + gap_width
            } else {
                actual_section_width
            };
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

                // Calculate actual start and end points, stopping before junctions or station curves
                let mut line_pos1 = pos1;
                let mut line_pos2 = pos2;

                // Get line-specific adjusted stop distance at source junction/station
                // Note: stored distances already have -2.0 applied
                let source_stop_distance = if source_is_junction {
                    junction_stop_distances.get(&(*edge_idx, line.id, source))
                        .copied()
                        .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
                } else if source_has_curve {
                    station_curve_stop_distances.get(&(*edge_idx, line.id, source))
                        .copied()
                        .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
                } else {
                    JUNCTION_STOP_DISTANCE - 2.0
                };

                // Get line-specific adjusted stop distance at target junction/station
                // Note: stored distances already have -2.0 applied
                let target_stop_distance = if target_is_junction {
                    junction_stop_distances.get(&(*edge_idx, line.id, target))
                        .copied()
                        .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
                } else if target_has_curve {
                    station_curve_stop_distances.get(&(*edge_idx, line.id, target))
                        .copied()
                        .unwrap_or(JUNCTION_STOP_DISTANCE - 2.0)
                } else {
                    JUNCTION_STOP_DISTANCE - 2.0
                };

                // When there's avoidance offset, use half junction distance
                let source_distance = if needs_avoidance && source_is_junction {
                    source_stop_distance * 0.5
                } else {
                    source_stop_distance
                };
                let target_distance = if needs_avoidance && target_is_junction {
                    target_stop_distance * 0.5
                } else {
                    target_stop_distance
                };

                if (source_is_junction || source_has_curve) && len > source_distance {
                    // Move start point away from junction or station curve
                    let t = source_distance / len;
                    line_pos1 = (pos1.0 + dx * t, pos1.1 + dy * t);
                }

                if (target_is_junction || target_has_curve) && len > target_distance {
                    // Move end point away from junction or station curve
                    let t = target_distance / len;
                    line_pos2 = (pos2.0 - dx * t, pos2.1 - dy * t);
                }

                // Check if source is terminal for this line
                // Only extend to center if this specific line doesn't have a curve at this station
                let source_has_curve_for_this_line = station_curve_stop_distances.contains_key(&(*edge_idx, line.id, source));
                if !source_is_junction && !source_has_curve_for_this_line && is_line_terminal(source, line, graph) {
                    // Extend back to station center
                    line_pos1 = (pos1.0, pos1.1);
                }

                // Check if target is terminal for this line
                // Only extend to center if this specific line doesn't have a curve at this station
                let target_has_curve_for_this_line = station_curve_stop_distances.contains_key(&(*edge_idx, line.id, target));
                if !target_is_junction && !target_has_curve_for_this_line && is_line_terminal(target, line, graph) {
                    // Extend back to station center
                    line_pos2 = (pos2.0, pos2.1);
                }

                // Set line width (already calculated with zoom adjustment)
                ctx.set_line_width(line_world_width);
                ctx.set_stroke_style_str(&line.color);
                ctx.begin_path();

                if needs_avoidance {
                    // Draw segmented path with offset
                    let segment_length = ((line_pos2.0 - line_pos1.0).powi(2) + (line_pos2.1 - line_pos1.1).powi(2)).sqrt();

                    // Check if we're connecting to junctions
                    let start_needs_transition = !source_is_junction;
                    let end_needs_transition = !target_is_junction;

                    draw_line_segment_with_avoidance(
                        ctx, line_pos1, line_pos2, segment_length,
                        (ox, oy), (avoid_x, avoid_y),
                        (start_needs_transition, end_needs_transition)
                    );
                } else {
                    ctx.move_to(line_pos1.0 + ox, line_pos1.1 + oy);
                    ctx.line_to(line_pos2.0 + ox, line_pos2.1 + oy);
                }

                let is_highlighted = highlighted_edges.contains(edge_idx);
                stroke_with_border(ctx, &line.color, line_world_width, gap_width * 0.2, theme, is_highlighted);

                // Draw caps at junction/station endpoints to cover rendering gaps
                let cap_radius = line_world_width / 2.0;
                // Draw cap at source (whether junction or station)
                ctx.begin_path();
                let _ = ctx.arc(line_pos1.0 + ox, line_pos1.1 + oy, cap_radius, 0.0, 2.0 * std::f64::consts::PI);
                ctx.set_fill_style_str(&line.color);
                ctx.fill();

                // Draw cap at target (whether junction or station)
                ctx.begin_path();
                let _ = ctx.arc(line_pos2.0 + ox, line_pos2.1 + oy, cap_radius, 0.0, 2.0 * std::f64::consts::PI);
                ctx.set_fill_style_str(&line.color);
                ctx.fill();
            }
        }

        // Draw junction connections involving this edge (to maintain z-order)
        for (connection_key, connection_lines) in &junction_connections {
            // Only draw if this edge is involved and we haven't drawn this connection yet
            if (connection_key.from_edge == *edge_idx || connection_key.to_edge == *edge_idx)
                && !drawn_junctions.contains(connection_key)
            {
                // Draw this junction connection
                draw_single_junction_connection(
                    ctx,
                    graph,
                    connection_key,
                    connection_lines,
                    &edge_to_section,
                    &section_orderings,
                    &section_visual_positions,
                    gap_width,
                    zoom,
                    theme,
                    highlighted_edges,
                );
                drawn_junctions.insert(*connection_key);
            }
        }
    }

    // All junction connections and station curves drawn above to maintain z-order
}
