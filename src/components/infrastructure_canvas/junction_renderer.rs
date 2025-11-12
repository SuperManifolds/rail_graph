use crate::models::{RailwayGraph, Junctions, Stations};
use web_sys::CanvasRenderingContext2d;
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::Direction;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::{HashSet, HashMap};

const JUNCTION_TRACK_DISTANCE: f64 = 14.0; // Match JUNCTION_STOP_DISTANCE from track_renderer
const TRACK_SPACING: f64 = 3.0; // Match track_renderer
const TRACK_COLOR: &str = "#444";
const HIGHLIGHTED_TRACK_COLOR: &str = "#4a9eff";
const TRACK_LINE_WIDTH: f64 = 2.0;

/// Get all junction connection line segments for label overlap detection
#[must_use]
pub fn get_junction_segments(graph: &RailwayGraph) -> Vec<((f64, f64), (f64, f64))> {
    let mut segments = Vec::new();

    for idx in graph.graph.node_indices() {
        if !graph.is_junction(idx) {
            continue;
        }

        let Some(pos) = graph.get_station_position(idx) else { continue };

        // Collect all connected edges
        let mut all_edges: Vec<(EdgeIndex, (f64, f64))> = Vec::new();
        let mut seen_edges = std::collections::HashSet::new();

        for edge in graph.graph.edges_directed(idx, Direction::Incoming) {
            if seen_edges.insert(edge.id()) {
                if let Some(source_pos) = graph.get_station_position(edge.source()) {
                    all_edges.push((edge.id(), source_pos));
                }
            }
        }

        for edge in graph.graph.edges(idx) {
            if seen_edges.insert(edge.id()) {
                if let Some(target_pos) = graph.get_station_position(edge.target()) {
                    all_edges.push((edge.id(), target_pos));
                }
            }
        }

        let Some(junction) = graph.get_junction(idx) else { continue };

        // Generate actual connection segments between entry and exit points
        for (i, (from_edge, from_node_pos)) in all_edges.iter().enumerate() {
            for (j, (to_edge, to_node_pos)) in all_edges.iter().enumerate() {
                if i == j {
                    continue;
                }

                // Check if routing is allowed
                if !junction.is_routing_allowed(*from_edge, *to_edge) {
                    continue;
                }

                // Calculate entry and exit base points (simplified - doesn't need track-level detail)
                let entry_delta = (from_node_pos.0 - pos.0, from_node_pos.1 - pos.1);
                let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();

                let exit_delta = (to_node_pos.0 - pos.0, to_node_pos.1 - pos.1);
                let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();

                if entry_distance > 0.0 && exit_distance > 0.0 {
                    let entry_base = (
                        pos.0 + (entry_delta.0 / entry_distance) * JUNCTION_TRACK_DISTANCE,
                        pos.1 + (entry_delta.1 / entry_distance) * JUNCTION_TRACK_DISTANCE,
                    );
                    let exit_base = (
                        pos.0 + (exit_delta.0 / exit_distance) * JUNCTION_TRACK_DISTANCE,
                        pos.1 + (exit_delta.1 / exit_distance) * JUNCTION_TRACK_DISTANCE,
                    );

                    segments.push((entry_base, exit_base));
                }
            }
        }
    }

    segments
}

#[allow(clippy::too_many_arguments)]
fn draw_junction_track_connections(
    ctx: &CanvasRenderingContext2d,
    from_track_indices: &[usize],
    to_track_indices: &[usize],
    from_total_tracks: usize,
    to_total_tracks: usize,
    entry_base: (f64, f64),
    exit_base: (f64, f64),
    from_perp: (f64, f64),
    to_perp: (f64, f64),
    zoom: f64,
    track_color: &str,
) {
    // Calculate position offsets for all tracks on from edge
    #[allow(clippy::cast_precision_loss)]
    let from_total_width = (from_total_tracks - 1) as f64 * TRACK_SPACING;
    let from_start_offset = -from_total_width / 2.0;

    // Get positions with offsets for tracks that allow arrival
    let mut entry_points: Vec<(f64, (f64, f64))> = Vec::new();
    for &track_idx in from_track_indices {
        #[allow(clippy::cast_precision_loss)]
        let offset = from_start_offset + (track_idx as f64 * TRACK_SPACING);
        entry_points.push((
            offset,
            (
                entry_base.0 + from_perp.0 * offset,
                entry_base.1 + from_perp.1 * offset
            )
        ));
    }

    // Calculate position offsets for all tracks on to edge
    #[allow(clippy::cast_precision_loss)]
    let to_total_width = (to_total_tracks - 1) as f64 * TRACK_SPACING;
    let to_start_offset = -to_total_width / 2.0;

    // Get positions with offsets for tracks that allow departure
    let mut exit_points: Vec<(f64, (f64, f64))> = Vec::new();
    for &track_idx in to_track_indices {
        #[allow(clippy::cast_precision_loss)]
        let offset = to_start_offset + (track_idx as f64 * TRACK_SPACING);
        exit_points.push((
            offset,
            (
                exit_base.0 + to_perp.0 * offset,
                exit_base.1 + to_perp.1 * offset
            )
        ));
    }

    // Draw connections based on geometric proximity
    ctx.set_stroke_style_str(track_color);
    ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

    let num_connections = entry_points.len().min(exit_points.len());

    // Calculate all pairwise distances and sort by distance
    let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
    for (entry_idx, entry_data) in entry_points.iter().enumerate() {
        let entry_point = entry_data.1;
        for (exit_idx, exit_data) in exit_points.iter().enumerate() {
            let exit_point = exit_data.1;
            let dist = ((exit_point.0 - entry_point.0).powi(2) +
                        (exit_point.1 - entry_point.1).powi(2)).sqrt();
            pairs.push((entry_idx, exit_idx, dist));
        }
    }

    // Sort by distance (closest first)
    pairs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    // Greedy matching: process pairs in order of distance, skip if already used
    let mut used_entries = HashSet::new();
    let mut used_exits = HashSet::new();
    let mut connections_made = 0;

    for (entry_idx, exit_idx, _dist) in pairs {
        if connections_made >= num_connections {
            break;
        }

        if used_entries.contains(&entry_idx) || used_exits.contains(&exit_idx) {
            continue;
        }

        let entry_point = entry_points[entry_idx].1;
        let exit_point = exit_points[exit_idx].1;

        ctx.begin_path();
        ctx.move_to(entry_point.0, entry_point.1);
        ctx.line_to(exit_point.0, exit_point.1);
        ctx.stroke();

        used_entries.insert(entry_idx);
        used_exits.insert(exit_idx);
        connections_made += 1;
    }
}

/// Helper function to match tracks geometrically using greedy algorithm
/// Returns vector of (`entry_idx`, `exit_idx`) pairs that were matched
fn match_tracks_geometrically(
    entry_points: &[(f64, (f64, f64))],
    exit_points: &[(f64, (f64, f64))],
) -> Vec<(usize, usize)> {
    let num_connections = entry_points.len().min(exit_points.len());
    let mut pairs: Vec<(usize, usize, f64)> = Vec::new();

    for (entry_idx, entry_data) in entry_points.iter().enumerate() {
        let entry_point = entry_data.1;
        for (exit_idx, exit_data) in exit_points.iter().enumerate() {
            let exit_point = exit_data.1;
            let dist = ((exit_point.0 - entry_point.0).powi(2) +
                        (exit_point.1 - entry_point.1).powi(2)).sqrt();
            pairs.push((entry_idx, exit_idx, dist));
        }
    }

    pairs.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut used_entries = HashSet::new();
    let mut used_exits = HashSet::new();
    let mut matches = Vec::new();

    for (entry_idx, exit_idx, _dist) in pairs {
        if matches.len() >= num_connections {
            break;
        }

        if used_entries.contains(&entry_idx) || used_exits.contains(&exit_idx) {
            continue;
        }

        matches.push((entry_idx, exit_idx));
        used_entries.insert(entry_idx);
        used_exits.insert(exit_idx);
    }

    matches
}

/// Identify which tracks on each edge don't have any connections through junctions
/// Returns a map of (`edge_idx`, `junction_idx`) -> set of orphaned track indices
#[must_use]
pub fn get_orphaned_tracks_map(graph: &RailwayGraph) -> HashMap<(EdgeIndex, NodeIndex), HashSet<usize>> {
    let mut orphaned_map: HashMap<(EdgeIndex, NodeIndex), HashSet<usize>> = HashMap::new();

    for idx in graph.graph.node_indices() {
        if !graph.is_junction(idx) {
            continue;
        }

        // Collect all connected edges
        let mut all_edges: Vec<(EdgeIndex, (f64, f64))> = Vec::new();
        let mut seen_edges = HashSet::new();

        for edge in graph.graph.edges_directed(idx, Direction::Incoming) {
            if seen_edges.insert(edge.id()) {
                if let Some(source_pos) = graph.get_station_position(edge.source()) {
                    all_edges.push((edge.id(), source_pos));
                }
            }
        }

        for edge in graph.graph.edges(idx) {
            if seen_edges.insert(edge.id()) {
                if let Some(target_pos) = graph.get_station_position(edge.target()) {
                    all_edges.push((edge.id(), target_pos));
                }
            }
        }

        let Some(junction) = graph.get_junction(idx) else { continue };
        let Some(pos) = graph.get_station_position(idx) else { continue };

        // Initialize all tracks as orphaned for all edges at this junction
        for (edge_idx, _) in &all_edges {
            let Some(edge_ref) = graph.graph.edge_references().find(|e| e.id() == *edge_idx) else {
                continue;
            };
            let tracks = &edge_ref.weight().tracks;
            let all_track_indices: HashSet<usize> = (0..tracks.len()).collect();
            orphaned_map.insert((*edge_idx, idx), all_track_indices);
        }

        // Process each edge pair to find which tracks get matched
        for (i, (from_edge, from_node_pos)) in all_edges.iter().enumerate() {
            let Some(from_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *from_edge) else {
                continue;
            };
            let from_tracks = &from_edge_ref.weight().tracks;

            if from_tracks.is_empty() {
                continue;
            }

            let arriving_tracks: Vec<usize> = (0..from_tracks.len()).collect();

            for (j_idx, (to_edge, to_node_pos)) in all_edges.iter().enumerate() {
                if i >= j_idx {
                    continue;
                }

                if !junction.is_routing_allowed(*from_edge, *to_edge) {
                    continue;
                }

                let Some(to_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *to_edge) else {
                    continue;
                };
                let to_tracks = &to_edge_ref.weight().tracks;

                if to_tracks.is_empty() {
                    continue;
                }

                let departing_tracks: Vec<usize> = (0..to_tracks.len()).collect();

                // Calculate entry and exit points with offsets (same logic as draw_junction_track_connections)
                let Some(from_source_pos) = graph.get_station_position(from_edge_ref.source()) else { continue };
                let Some(from_target_pos) = graph.get_station_position(from_edge_ref.target()) else { continue };
                let from_edge_vec = (from_target_pos.0 - from_source_pos.0, from_target_pos.1 - from_source_pos.1);
                let from_edge_len = (from_edge_vec.0 * from_edge_vec.0 + from_edge_vec.1 * from_edge_vec.1).sqrt();
                let from_perp = (-from_edge_vec.1 / from_edge_len, from_edge_vec.0 / from_edge_len);

                let entry_delta = (from_node_pos.0 - pos.0, from_node_pos.1 - pos.1);
                let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();
                let entry_base = (
                    pos.0 + (entry_delta.0 / entry_distance) * JUNCTION_TRACK_DISTANCE,
                    pos.1 + (entry_delta.1 / entry_distance) * JUNCTION_TRACK_DISTANCE,
                );

                #[allow(clippy::cast_precision_loss)]
                let from_total_width = (from_tracks.len() - 1) as f64 * TRACK_SPACING;
                let from_start_offset = -from_total_width / 2.0;

                let mut entry_points: Vec<(f64, (f64, f64))> = Vec::new();
                for &track_idx in &arriving_tracks {
                    #[allow(clippy::cast_precision_loss)]
                    let offset = from_start_offset + (track_idx as f64 * TRACK_SPACING);
                    entry_points.push((
                        offset,
                        (
                            entry_base.0 + from_perp.0 * offset,
                            entry_base.1 + from_perp.1 * offset
                        )
                    ));
                }

                let Some(to_source_pos) = graph.get_station_position(to_edge_ref.source()) else { continue };
                let Some(to_target_pos) = graph.get_station_position(to_edge_ref.target()) else { continue };
                let to_edge_vec = (to_target_pos.0 - to_source_pos.0, to_target_pos.1 - to_source_pos.1);
                let to_edge_len = (to_edge_vec.0 * to_edge_vec.0 + to_edge_vec.1 * to_edge_vec.1).sqrt();
                let to_perp = (-to_edge_vec.1 / to_edge_len, to_edge_vec.0 / to_edge_len);

                let exit_delta = (to_node_pos.0 - pos.0, to_node_pos.1 - pos.1);
                let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();
                let exit_base = (
                    pos.0 + (exit_delta.0 / exit_distance) * JUNCTION_TRACK_DISTANCE,
                    pos.1 + (exit_delta.1 / exit_distance) * JUNCTION_TRACK_DISTANCE,
                );

                #[allow(clippy::cast_precision_loss)]
                let to_total_width = (to_tracks.len() - 1) as f64 * TRACK_SPACING;
                let to_start_offset = -to_total_width / 2.0;

                let mut exit_points: Vec<(f64, (f64, f64))> = Vec::new();
                for &track_idx in &departing_tracks {
                    #[allow(clippy::cast_precision_loss)]
                    let offset = to_start_offset + (track_idx as f64 * TRACK_SPACING);
                    exit_points.push((
                        offset,
                        (
                            exit_base.0 + to_perp.0 * offset,
                            exit_base.1 + to_perp.1 * offset
                        )
                    ));
                }

                // Run the same geometric matching logic
                let matched = match_tracks_geometrically(&entry_points, &exit_points);

                // Mark matched tracks as NOT orphaned
                for (entry_idx, exit_idx) in matched {
                    orphaned_map
                        .get_mut(&(*from_edge, idx))
                        .map(|set| set.remove(&arriving_tracks[entry_idx]));
                    orphaned_map
                        .get_mut(&(*to_edge, idx))
                        .map(|set| set.remove(&departing_tracks[exit_idx]));
                }
            }
        }
    }

    orphaned_map
}

/// Helper function to find line-line intersection
/// Returns parameter t for line1 where intersection occurs
/// Line1: p1 + t * d1, Line2: p2 + s * d2
fn line_intersection(
    p1: (f64, f64),
    d1: (f64, f64),
    p2: (f64, f64),
    d2: (f64, f64),
) -> Option<f64> {
    // Solve: p1 + t * d1 = p2 + s * d2
    // t * d1.x - s * d2.x = p2.x - p1.x
    // t * d1.y - s * d2.y = p2.y - p1.y

    let det = d1.0 * d2.1 - d1.1 * d2.0;
    if det.abs() < 1e-10 {
        return None; // Lines are parallel
    }

    let dx = p2.0 - p1.0;
    let dy = p2.1 - p1.1;

    let t = (dx * d2.1 - dy * d2.0) / det;
    Some(t)
}

/// Calculate where orphaned tracks should intersect with crossover X diagonals
/// Returns a map of (`edge_idx`, `junction_idx`, `track_idx`) -> intersection point
///
/// # Panics
/// Panics if `non_orphaned` is empty when calling `min()` or `max()`, but this is prevented by checking `is_empty()` first
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn get_crossover_intersection_points(
    graph: &RailwayGraph,
    orphaned_tracks: &HashMap<(EdgeIndex, NodeIndex), HashSet<usize>>,
    cached_avoidance: &HashMap<EdgeIndex, (f64, f64)>,
) -> HashMap<(EdgeIndex, NodeIndex, usize), (f64, f64)> {
    let mut intersections = HashMap::new();

    for ((edge_idx, junction_idx), orphaned_set) in orphaned_tracks {
        if orphaned_set.is_empty() {
            continue;
        }

        let Some(edge_ref) = graph.graph.edge_references().find(|e| e.id() == *edge_idx) else {
            continue;
        };
        let num_tracks = edge_ref.weight().tracks.len();

        if num_tracks < 2 {
            continue; // No crossover for single track
        }

        // Check if this edge has only one allowed connection with 1:1 track mapping
        // If so, skip crossover calculation since no track switching is needed
        let Some(junction) = graph.get_junction(*junction_idx) else { continue };

        // Collect all connected edges at this junction
        let mut all_edges_at_junction: Vec<EdgeIndex> = Vec::new();
        for edge in graph.graph.edges_directed(*junction_idx, Direction::Incoming) {
            all_edges_at_junction.push(edge.id());
        }
        for edge in graph.graph.edges(*junction_idx) {
            all_edges_at_junction.push(edge.id());
        }

        let allowed_connections: Vec<_> = all_edges_at_junction.iter()
            .filter(|other_edge| {
                **other_edge != *edge_idx &&
                (junction.is_routing_allowed(*edge_idx, **other_edge) ||
                 junction.is_routing_allowed(**other_edge, *edge_idx))
            })
            .collect();

        let should_skip_crossover = if allowed_connections.len() == 1 {
            let other_edge_idx = allowed_connections[0];
            if let Some(other_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *other_edge_idx) {
                let other_tracks = &other_edge_ref.weight().tracks;
                // Skip if both edges have the same number of tracks (1:1 mapping)
                num_tracks == other_tracks.len()
            } else {
                false
            }
        } else {
            false
        };

        if should_skip_crossover {
            continue;
        }

        let Some(junction_pos) = graph.get_station_position(*junction_idx) else { continue };

        // Get the other end of this edge
        let (source, target) = (edge_ref.source(), edge_ref.target());
        let other_node = if source == *junction_idx { target } else { source };
        let Some(other_pos) = graph.get_station_position(other_node) else { continue };

        // Calculate edge direction and perpendicular
        let Some(source_pos) = graph.get_station_position(source) else { continue };
        let Some(target_pos) = graph.get_station_position(target) else { continue };
        let edge_vec = (target_pos.0 - source_pos.0, target_pos.1 - source_pos.1);
        let edge_len = (edge_vec.0 * edge_vec.0 + edge_vec.1 * edge_vec.1).sqrt();
        if edge_len < 0.1 {
            continue;
        }
        let perp = (-edge_vec.1 / edge_len, edge_vec.0 / edge_len);

        // Calculate direction from junction toward other node
        let delta = (other_pos.0 - junction_pos.0, other_pos.1 - junction_pos.1);
        let distance = (delta.0 * delta.0 + delta.1 * delta.1).sqrt();
        if distance < 0.1 {
            continue;
        }
        let away_from_junction_dir = (delta.0 / distance, delta.1 / distance);

        let (avoid_x, avoid_y) = cached_avoidance.get(edge_idx).copied().unwrap_or((0.0, 0.0));
        let has_avoidance = avoid_x.abs() > 0.1 || avoid_y.abs() > 0.1;

        let base = if has_avoidance {
            (
                junction_pos.0 + away_from_junction_dir.0 * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_x,
                junction_pos.1 + away_from_junction_dir.1 * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_y,
            )
        } else {
            (
                junction_pos.0 + away_from_junction_dir.0 * JUNCTION_TRACK_DISTANCE,
                junction_pos.1 + away_from_junction_dir.1 * JUNCTION_TRACK_DISTANCE,
            )
        };

        // Calculate crossover geometry (same as draw_crossover_switches)
        #[allow(clippy::cast_precision_loss)]
        let total_width = (num_tracks - 1) as f64 * TRACK_SPACING;
        let start_offset = -total_width / 2.0;
        let crossover_length = total_width;
        let crossover_gap = 0.0;

        let crossover_end_base = (
            base.0 + away_from_junction_dir.0 * crossover_gap,
            base.1 + away_from_junction_dir.1 * crossover_gap,
        );

        // Outermost track offsets
        #[allow(clippy::cast_precision_loss)]
        let offset_first = start_offset;
        #[allow(clippy::cast_precision_loss)]
        let offset_last = start_offset + ((num_tracks - 1) as f64 * TRACK_SPACING);

        // X diagonal endpoints
        let end_first = (
            crossover_end_base.0 + perp.0 * offset_first,
            crossover_end_base.1 + perp.1 * offset_first,
        );
        let end_last = (
            crossover_end_base.0 + perp.0 * offset_last,
            crossover_end_base.1 + perp.1 * offset_last,
        );
        let start_first = (
            end_first.0 + away_from_junction_dir.0 * crossover_length,
            end_first.1 + away_from_junction_dir.1 * crossover_length,
        );
        let start_last = (
            end_last.0 + away_from_junction_dir.0 * crossover_length,
            end_last.1 + away_from_junction_dir.1 * crossover_length,
        );

        // Two X diagonals
        let diagonal1_start = start_first;
        let diagonal1_end = end_last;
        let diagonal1_dir = (diagonal1_end.0 - diagonal1_start.0, diagonal1_end.1 - diagonal1_start.1);

        let diagonal2_start = start_last;
        let diagonal2_end = end_first;
        let diagonal2_dir = (diagonal2_end.0 - diagonal2_start.0, diagonal2_end.1 - diagonal2_start.1);

        // Get non-orphaned track indices to determine which diagonal to use
        let non_orphaned: Vec<usize> = (0..num_tracks)
            .filter(|i| !orphaned_set.contains(i))
            .collect();

        // For each orphaned track, find intersection with the LAST diagonal that connects toward non-orphaned tracks
        #[allow(clippy::excessive_nesting)]
        for &track_idx in orphaned_set {
            #[allow(clippy::cast_precision_loss)]
            let track_offset = start_offset + (track_idx as f64 * TRACK_SPACING);

            // Track line starts at the edge (further from junction) and goes toward junction
            let track_start = (
                crossover_end_base.0 + perp.0 * track_offset + away_from_junction_dir.0 * crossover_length,
                crossover_end_base.1 + perp.1 * track_offset + away_from_junction_dir.1 * crossover_length,
            );

            // Track direction is toward junction
            let track_dir = (-away_from_junction_dir.0, -away_from_junction_dir.1);

            // Calculate intersections with both diagonals
            let t1 = line_intersection(track_start, track_dir, diagonal1_start, diagonal1_dir);
            let t2 = line_intersection(track_start, track_dir, diagonal2_start, diagonal2_dir);

            // Determine which diagonal connects this track toward the non-orphaned tracks
            // Diagonal 1 (start_first → end_last): connects track 0 at far end to track N-1 at near end
            // Diagonal 2 (start_last → end_first): connects track N-1 at far end to track 0 at near end

            let chosen_t = if non_orphaned.is_empty() {
                // No non-orphaned tracks - use whichever intersection comes first
                match (t1, t2) {
                    (Some(t1_val), Some(t2_val)) if t1_val >= -0.01 && t2_val >= -0.01 => {
                        Some(t1_val.max(0.0).min(t2_val.max(0.0)))
                    }
                    (Some(t1_val), _) if t1_val >= -0.01 => Some(t1_val.max(0.0)),
                    (_, Some(t2_val)) if t2_val >= -0.01 => Some(t2_val.max(0.0)),
                    _ => None,
                }
            } else {
                // Determine which side of center this orphaned track is on
                #[allow(clippy::cast_precision_loss)]
                let center = (num_tracks - 1) as f64 / 2.0;
                #[allow(clippy::cast_precision_loss)]
                let is_left_of_center = (track_idx as f64) < center;

                // Check if we need the second crossing to reach any non-orphaned tracks
                let needs_second_crossing = if is_left_of_center {
                    // LEFT of center: first crossing goes RIGHT
                    // Need second crossing if any non-orphaned tracks are to the LEFT of this track
                    non_orphaned.iter().any(|&idx| idx < track_idx)
                } else {
                    // RIGHT of center: first crossing goes LEFT
                    // Need second crossing if any non-orphaned tracks are to the RIGHT of this track
                    non_orphaned.iter().any(|&idx| idx > track_idx)
                };

                let mut valid_intersections = Vec::new();

                // Add any diagonal this track intersects (non-negative t value)
                // Include t=0 case where track starts at a diagonal endpoint
                if let Some(t1_val) = t1 {
                    if t1_val >= -0.01 {  // Small epsilon for floating point tolerance
                        valid_intersections.push(t1_val.max(0.0));  // Clamp to 0 if slightly negative
                    }
                }

                if let Some(t2_val) = t2 {
                    if t2_val >= -0.01 {  // Small epsilon for floating point tolerance
                        valid_intersections.push(t2_val.max(0.0));  // Clamp to 0 if slightly negative
                    }
                }

                // Choose which intersection to use:
                // - First crossing connects to OPPOSITE side of track position
                // - Second crossing connects to SAME side of track position
                // - If all non-orphaned tracks reachable via first crossing: use FIRST (min)
                // - If any non-orphaned tracks need second crossing: use SECOND (max)
                if needs_second_crossing {
                    // Need second crossing to reach some tracks
                    valid_intersections.into_iter().max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                } else {
                    // First crossing reaches all tracks
                    valid_intersections.into_iter().min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                }
            };

            if let Some(t) = chosen_t {
                let intersection = (
                    track_start.0 + track_dir.0 * t,
                    track_start.1 + track_dir.1 * t,
                );
                intersections.insert((*edge_idx, *junction_idx, track_idx), intersection);
            }
        }
    }

    intersections
}

/// Draw crossover switches (X-shaped connections) between all tracks on an edge
#[allow(clippy::too_many_arguments, clippy::excessive_nesting)]
fn draw_crossover_switches(
    ctx: &CanvasRenderingContext2d,
    edge_idx: EdgeIndex,
    num_tracks: usize,
    junction_pos: (f64, f64),
    junction_idx: NodeIndex,
    edge_pos: (f64, f64),
    zoom: f64,
    graph: &RailwayGraph,
    cached_avoidance: &HashMap<EdgeIndex, (f64, f64)>,
    orphaned_tracks: &HashMap<(EdgeIndex, NodeIndex), HashSet<usize>>,
    crossover_intersections: &HashMap<(EdgeIndex, NodeIndex, usize), (f64, f64)>,
) {
    // Get edge details
    let Some(edge_ref) = graph.graph.edge_references().find(|e| e.id() == edge_idx) else {
        return;
    };
    let (source, target) = (edge_ref.source(), edge_ref.target());

    // Get actual node positions
    let Some(source_pos) = graph.get_station_position(source) else { return };
    let Some(target_pos) = graph.get_station_position(target) else { return };

    // Calculate edge direction and perpendicular
    let edge_vec = (target_pos.0 - source_pos.0, target_pos.1 - source_pos.1);
    let edge_len = (edge_vec.0 * edge_vec.0 + edge_vec.1 * edge_vec.1).sqrt();
    if edge_len < 0.1 {
        return;
    }
    let perp = (-edge_vec.1 / edge_len, edge_vec.0 / edge_len);

    // Calculate base point at junction edge
    let delta = (edge_pos.0 - junction_pos.0, edge_pos.1 - junction_pos.1);
    let distance = (delta.0 * delta.0 + delta.1 * delta.1).sqrt();
    if distance < 0.1 {
        return;
    }

    // Direction away from junction (toward the other node)
    let away_from_junction_dir = (delta.0 / distance, delta.1 / distance);

    let (avoid_x, avoid_y) = cached_avoidance.get(&edge_idx).copied().unwrap_or((0.0, 0.0));
    let has_avoidance = avoid_x.abs() > 0.1 || avoid_y.abs() > 0.1;

    let base = if has_avoidance {
        (
            junction_pos.0 + away_from_junction_dir.0 * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_x,
            junction_pos.1 + away_from_junction_dir.1 * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_y,
        )
    } else {
        (
            junction_pos.0 + away_from_junction_dir.0 * JUNCTION_TRACK_DISTANCE,
            junction_pos.1 + away_from_junction_dir.1 * JUNCTION_TRACK_DISTANCE,
        )
    };

    // Calculate track spacing
    #[allow(clippy::cast_precision_loss)]
    let total_width = (num_tracks - 1) as f64 * TRACK_SPACING;
    let start_offset = -total_width / 2.0;

    // For 45° angle, crossover length equals perpendicular distance between outermost tracks
    let crossover_length = total_width;

    // Position the crossover right at the junction edge
    let crossover_gap = 0.0;

    ctx.set_stroke_style_str(TRACK_COLOR);
    ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

    // Draw one big X connecting outermost tracks (so all tracks can interconnect)
    // Track 0 offset (leftmost/topmost)
    #[allow(clippy::cast_precision_loss)]
    let offset_first = start_offset;
    // Track N-1 offset (rightmost/bottommost)
    #[allow(clippy::cast_precision_loss)]
    let offset_last = start_offset + ((num_tracks - 1) as f64 * TRACK_SPACING);

    // Calculate the base point for crossover end (at the junction edge)
    let crossover_end_base = (
        base.0 + away_from_junction_dir.0 * crossover_gap,
        base.1 + away_from_junction_dir.1 * crossover_gap,
    );

    // End points of X (closer to junction)
    let end_first = (
        crossover_end_base.0 + perp.0 * offset_first,
        crossover_end_base.1 + perp.1 * offset_first,
    );

    let end_last = (
        crossover_end_base.0 + perp.0 * offset_last,
        crossover_end_base.1 + perp.1 * offset_last,
    );

    // Start points of X (further from junction) - for 45° angle
    let start_first = (
        end_first.0 + away_from_junction_dir.0 * crossover_length,
        end_first.1 + away_from_junction_dir.1 * crossover_length,
    );

    let start_last = (
        end_last.0 + away_from_junction_dir.0 * crossover_length,
        end_last.1 + away_from_junction_dir.1 * crossover_length,
    );

    // Calculate which tracks use each diagonal to optimize diagonal endpoints
    // Diagonal 1: start_first (track 0, far) -> end_last (track N-1, near)
    // Diagonal 2: start_last (track N-1, far) -> end_first (track 0, near)

    let orphaned_set = orphaned_tracks.get(&(edge_idx, junction_idx));

    // Collect which tracks use diagonal 1 and diagonal 2
    let mut diagonal1_tracks = Vec::new();
    let mut diagonal2_tracks = Vec::new();

    if let Some(orphaned) = orphaned_set {
        for track_idx in 0..num_tracks {
            if orphaned.contains(&track_idx) {
                // This track is orphaned - check which diagonal it uses
                if crossover_intersections.contains_key(&(edge_idx, junction_idx, track_idx)) {
                    // Determine which diagonal this track intersects with
                    // Need to check which diagonal the intersection point lies on
                    let intersection = crossover_intersections[&(edge_idx, junction_idx, track_idx)];

                    // Check distance to each diagonal
                    let dist_to_d1 = point_to_line_segment_distance(intersection, start_first, end_last);
                    let dist_to_d2 = point_to_line_segment_distance(intersection, start_last, end_first);

                    if dist_to_d1 < dist_to_d2 {
                        diagonal1_tracks.push(track_idx);
                    } else {
                        diagonal2_tracks.push(track_idx);
                    }
                }
            } else {
                // Non-orphaned tracks use both diagonals (they connect through)
                diagonal1_tracks.push(track_idx);
                diagonal2_tracks.push(track_idx);
            }
        }
    } else {
        // No orphaned tracks - all tracks use both diagonals
        for track_idx in 0..num_tracks {
            diagonal1_tracks.push(track_idx);
            diagonal2_tracks.push(track_idx);
        }
    }

    // Calculate optimal endpoints for each diagonal based on which tracks use them
    // Diagonal 1: start_first (track 0) -> end_last (track N-1)
    let (actual_start_first, actual_end_last) = if diagonal1_tracks.is_empty() {
        // No tracks use this diagonal, don't draw it
        (start_first, start_first) // Same point = won't draw a line
    } else {
        let min_track = *diagonal1_tracks.iter().min().expect("diagonal1_tracks is not empty");
        let max_track = *diagonal1_tracks.iter().max().expect("diagonal1_tracks is not empty");

        #[allow(clippy::cast_precision_loss)]
        let min_offset = start_offset + (min_track as f64 * TRACK_SPACING);
        #[allow(clippy::cast_precision_loss)]
        let max_offset = start_offset + (max_track as f64 * TRACK_SPACING);

        let start = (
            crossover_end_base.0 + perp.0 * min_offset + away_from_junction_dir.0 * crossover_length,
            crossover_end_base.1 + perp.1 * min_offset + away_from_junction_dir.1 * crossover_length,
        );
        let end = (
            crossover_end_base.0 + perp.0 * max_offset,
            crossover_end_base.1 + perp.1 * max_offset,
        );
        (start, end)
    };

    // Diagonal 2: start_last (track N-1) -> end_first (track 0)
    let (actual_start_last, actual_end_first) = if diagonal2_tracks.is_empty() {
        // No tracks use this diagonal, don't draw it
        (start_last, start_last) // Same point = won't draw a line
    } else {
        let min_track = *diagonal2_tracks.iter().min().expect("diagonal2_tracks is not empty");
        let max_track = *diagonal2_tracks.iter().max().expect("diagonal2_tracks is not empty");

        #[allow(clippy::cast_precision_loss)]
        let min_offset = start_offset + (min_track as f64 * TRACK_SPACING);
        #[allow(clippy::cast_precision_loss)]
        let max_offset = start_offset + (max_track as f64 * TRACK_SPACING);

        let start = (
            crossover_end_base.0 + perp.0 * max_offset + away_from_junction_dir.0 * crossover_length,
            crossover_end_base.1 + perp.1 * max_offset + away_from_junction_dir.1 * crossover_length,
        );
        let end = (
            crossover_end_base.0 + perp.0 * min_offset,
            crossover_end_base.1 + perp.1 * min_offset,
        );
        (start, end)
    };

    // Draw X: first track from far to last track near junction, and vice versa
    ctx.begin_path();
    ctx.move_to(actual_start_first.0, actual_start_first.1);
    ctx.line_to(actual_end_last.0, actual_end_last.1);
    ctx.stroke();

    ctx.begin_path();
    ctx.move_to(actual_start_last.0, actual_start_last.1);
    ctx.line_to(actual_end_first.0, actual_end_first.1);
    ctx.stroke();
}

/// Calculate distance from a point to a line segment
fn point_to_line_segment_distance(
    point: (f64, f64),
    line_start: (f64, f64),
    line_end: (f64, f64),
) -> f64 {
    let dx = line_end.0 - line_start.0;
    let dy = line_end.1 - line_start.1;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-10 {
        // Line segment is a point
        let px = point.0 - line_start.0;
        let py = point.1 - line_start.1;
        return (px * px + py * py).sqrt();
    }

    // Calculate projection parameter
    let t = ((point.0 - line_start.0) * dx + (point.1 - line_start.1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);

    // Calculate closest point on line segment
    let closest_x = line_start.0 + t * dx;
    let closest_y = line_start.1 + t * dy;

    // Distance from point to closest point
    let px = point.0 - closest_x;
    let py = point.1 - closest_y;
    (px * px + py * py).sqrt()
}

#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn draw_junction(
    ctx: &CanvasRenderingContext2d,
    graph: &RailwayGraph,
    idx: NodeIndex,
    pos: (f64, f64),
    zoom: f64,
    highlighted_edges: &HashSet<EdgeIndex>,
    cached_avoidance: &HashMap<EdgeIndex, (f64, f64)>,
    orphaned_tracks: &HashMap<(EdgeIndex, NodeIndex), HashSet<usize>>,
    crossover_intersections: &HashMap<(EdgeIndex, NodeIndex, usize), (f64, f64)>,
) {
    // Collect all connected edges - we need ALL edges connected to the junction
    // because an edge can have tracks going in either direction
    let mut all_edges: Vec<(EdgeIndex, (f64, f64))> = Vec::new();
    let mut seen_edges = std::collections::HashSet::new();

    // Incoming edges (where junction is target)
    for edge in graph.graph.edges_directed(idx, Direction::Incoming) {
        if seen_edges.insert(edge.id()) {
            if let Some(source_pos) = graph.get_station_position(edge.source()) {
                all_edges.push((edge.id(), source_pos));
            }
        }
    }

    // Outgoing edges (where junction is source)
    for edge in graph.graph.edges(idx) {
        if seen_edges.insert(edge.id()) {
            if let Some(target_pos) = graph.get_station_position(edge.target()) {
                all_edges.push((edge.id(), target_pos));
            }
        }
    }

    if all_edges.is_empty() {
        return;
    }

    let junction = graph.get_junction(idx);
    let Some(j) = junction else { return };

    ctx.set_line_width(TRACK_LINE_WIDTH / zoom);

    // Draw crossover switches for multi-track edges
    for (edge_idx, edge_pos) in &all_edges {
        let Some(edge_ref) = graph.graph.edge_references().find(|e| e.id() == *edge_idx) else {
            continue;
        };
        let tracks = &edge_ref.weight().tracks;

        if tracks.len() >= 2 {
            // Check if this edge has only one allowed connection with 1:1 track mapping
            // If so, skip the crossover since no track switching is needed
            let allowed_connections: Vec<_> = all_edges.iter()
                .filter(|(other_edge, _)| {
                    *other_edge != *edge_idx &&
                    (j.is_routing_allowed(*edge_idx, *other_edge) ||
                     j.is_routing_allowed(*other_edge, *edge_idx))
                })
                .collect();

            let should_skip_crossover = if allowed_connections.len() == 1 {
                let (other_edge_idx, _) = allowed_connections[0];
                if let Some(other_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *other_edge_idx) {
                    let other_tracks = &other_edge_ref.weight().tracks;
                    // Skip if both edges have the same number of tracks (1:1 mapping)
                    tracks.len() == other_tracks.len()
                } else {
                    false
                }
            } else {
                false
            };

            if !should_skip_crossover {
                draw_crossover_switches(
                    ctx,
                    *edge_idx,
                    tracks.len(),
                    pos,
                    idx,
                    *edge_pos,
                    zoom,
                    graph,
                    cached_avoidance,
                    orphaned_tracks,
                    crossover_intersections,
                );
            }
        }
    }

    // Draw connections between edges, checking track-by-track directionality
    for (i, (from_edge, from_node_pos)) in all_edges.iter().enumerate() {
        let Some(from_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *from_edge) else {
            continue;
        };
        let from_tracks = &from_edge_ref.weight().tracks;

        if from_tracks.is_empty() {
            continue;
        }

        // For visual representation, use all tracks regardless of individual track directionality
        // (Junction routing rules are still respected via is_routing_allowed check)
        let arriving_tracks: Vec<usize> = (0..from_tracks.len()).collect();

        for (j_idx, (to_edge, to_node_pos)) in all_edges.iter().enumerate() {
            if i == j_idx {
                continue; // Skip same edge
            }

            // Only process each pair once (skip if we've already processed the reverse)
            if i > j_idx {
                continue;
            }

            // Check if this routing is allowed by junction
            let is_allowed = j.is_routing_allowed(*from_edge, *to_edge);
            if !is_allowed {
                continue;
            }

            let Some(to_edge_ref) = graph.graph.edge_references().find(|e| e.id() == *to_edge) else {
                continue;
            };
            let to_tracks = &to_edge_ref.weight().tracks;

            if to_tracks.is_empty() {
                continue;
            }

            // For visual representation, use all tracks regardless of individual track directionality
            // (Junction routing rules are still respected via is_routing_allowed check)
            let departing_tracks: Vec<usize> = (0..to_tracks.len()).collect();

            // Get the from edge details to calculate proper perpendicular
            let from_edge_ref = graph.graph.edge_references().find(|e| e.id() == *from_edge);
            let (from_source, from_target) = if let Some(e) = from_edge_ref {
                (e.source(), e.target())
            } else {
                continue;
            };

            let from_source_pos = graph.get_station_position(from_source).unwrap_or(pos);
            let from_target_pos = graph.get_station_position(from_target).unwrap_or(pos);

            // Edge direction vector (source -> target, matching track_renderer)
            let from_edge_vec = (from_target_pos.0 - from_source_pos.0, from_target_pos.1 - from_source_pos.1);
            let from_edge_len = (from_edge_vec.0 * from_edge_vec.0 + from_edge_vec.1 * from_edge_vec.1).sqrt();

            // Perpendicular to from edge (same calculation as track_renderer)
            let from_perp = (-from_edge_vec.1 / from_edge_len, from_edge_vec.0 / from_edge_len);

            // Calculate entry base point
            let entry_delta = (from_node_pos.0 - pos.0, from_node_pos.1 - pos.1);
            let entry_distance = (entry_delta.0 * entry_delta.0 + entry_delta.1 * entry_delta.1).sqrt();

            // Use cached avoidance offset for this edge
            let (avoid_from_x, avoid_from_y) = cached_avoidance.get(from_edge).copied().unwrap_or((0.0, 0.0));

            // Get the to edge details to calculate proper perpendicular
            let to_edge_ref = graph.graph.edge_references().find(|e| e.id() == *to_edge);
            let (to_source, to_target) = if let Some(e) = to_edge_ref {
                (e.source(), e.target())
            } else {
                continue;
            };

            let to_source_pos = graph.get_station_position(to_source).unwrap_or(pos);
            let to_target_pos = graph.get_station_position(to_target).unwrap_or(pos);

            // Edge direction vector (source -> target, matching track_renderer)
            let to_edge_vec = (to_target_pos.0 - to_source_pos.0, to_target_pos.1 - to_source_pos.1);
            let to_edge_len = (to_edge_vec.0 * to_edge_vec.0 + to_edge_vec.1 * to_edge_vec.1).sqrt();

            // Perpendicular to to edge (same calculation as track_renderer)
            let to_perp = (-to_edge_vec.1 / to_edge_len, to_edge_vec.0 / to_edge_len);

            // Calculate exit base point
            let exit_delta = (to_node_pos.0 - pos.0, to_node_pos.1 - pos.1);
            let exit_distance = (exit_delta.0 * exit_delta.0 + exit_delta.1 * exit_delta.1).sqrt();

            // Use cached avoidance offset for this edge
            let (avoid_to_x, avoid_to_y) = cached_avoidance.get(to_edge).copied().unwrap_or((0.0, 0.0));

            // For edges with avoidance offset, use half perimeter distance
            let from_has_avoidance = avoid_from_x.abs() > 0.1 || avoid_from_y.abs() > 0.1;
            let to_has_avoidance = avoid_to_x.abs() > 0.1 || avoid_to_y.abs() > 0.1;

            let entry_base = if from_has_avoidance {
                (
                    pos.0 + (entry_delta.0 / entry_distance) * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_from_x,
                    pos.1 + (entry_delta.1 / entry_distance) * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_from_y,
                )
            } else {
                (
                    pos.0 + (entry_delta.0 / entry_distance) * JUNCTION_TRACK_DISTANCE,
                    pos.1 + (entry_delta.1 / entry_distance) * JUNCTION_TRACK_DISTANCE,
                )
            };

            let exit_base = if to_has_avoidance {
                (
                    pos.0 + (exit_delta.0 / exit_distance) * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_to_x,
                    pos.1 + (exit_delta.1 / exit_distance) * (JUNCTION_TRACK_DISTANCE * 0.5) + avoid_to_y,
                )
            } else {
                (
                    pos.0 + (exit_delta.0 / exit_distance) * JUNCTION_TRACK_DISTANCE,
                    pos.1 + (exit_delta.1 / exit_distance) * JUNCTION_TRACK_DISTANCE,
                )
            };

            // Determine track color based on whether both edges are highlighted
            let is_highlighted = highlighted_edges.contains(from_edge) && highlighted_edges.contains(to_edge);
            let track_color = if is_highlighted { HIGHLIGHTED_TRACK_COLOR } else { TRACK_COLOR };

            if arriving_tracks.len() == 1 && departing_tracks.len() == 1 {
                // Single track to single track - draw simple connection
                // Calculate the actual positions of these specific tracks
                #[allow(clippy::cast_precision_loss)]
                let from_total_width = (from_tracks.len() - 1) as f64 * TRACK_SPACING;
                let from_start_offset = -from_total_width / 2.0;
                #[allow(clippy::cast_precision_loss)]
                let from_offset = from_start_offset + (arriving_tracks[0] as f64 * TRACK_SPACING);

                #[allow(clippy::cast_precision_loss)]
                let to_total_width = (to_tracks.len() - 1) as f64 * TRACK_SPACING;
                let to_start_offset = -to_total_width / 2.0;
                #[allow(clippy::cast_precision_loss)]
                let to_offset = to_start_offset + (departing_tracks[0] as f64 * TRACK_SPACING);

                let entry_point = (
                    entry_base.0 + from_perp.0 * from_offset,
                    entry_base.1 + from_perp.1 * from_offset
                );
                let exit_point = (
                    exit_base.0 + to_perp.0 * to_offset,
                    exit_base.1 + to_perp.1 * to_offset
                );

                ctx.set_stroke_style_str(track_color);
                ctx.begin_path();
                ctx.move_to(entry_point.0, entry_point.1);
                ctx.line_to(exit_point.0, exit_point.1);
                ctx.stroke();
            } else {
                // Multiple tracks - connect every valid track to every other valid track
                draw_junction_track_connections(
                    ctx,
                    &arriving_tracks,
                    &departing_tracks,
                    from_tracks.len(),
                    to_tracks.len(),
                    entry_base,
                    exit_base,
                    from_perp,
                    to_perp,
                    zoom,
                    track_color,
                );
            }
        }
    }
}
