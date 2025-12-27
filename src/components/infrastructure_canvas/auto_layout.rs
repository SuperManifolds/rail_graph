use crate::models::{RailwayGraph, Stations, Junctions, ProjectSettings};
use crate::geometry::{angle_difference, line_segment_distance};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::{HashMap, HashSet};

const GRID_SIZE: f64 = 30.0;

/// Maximum bonus for continuing in the same direction as incoming
const CONTINUITY_BONUS_MAX: f64 = 2000.0;

/// Maximum bonus for matching geographic direction
const GEOGRAPHIC_BONUS_MAX: f64 = 3000.0;

/// Geographic hints for layout - provides preferred directions based on real-world coordinates
#[derive(Debug, Clone, Default)]
pub struct GeographicHints {
    /// Map from `NodeIndex` to (longitude, latitude) coordinates
    lonlat_map: HashMap<NodeIndex, (f64, f64)>,
}

impl GeographicHints {
    /// Create empty hints (no geographic data)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            lonlat_map: HashMap::new(),
        }
    }

    /// Create hints from a lonlat map
    #[must_use]
    pub fn new(lonlat_map: HashMap<NodeIndex, (f64, f64)>) -> Self {
        Self { lonlat_map }
    }

    /// Check if hints are available
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lonlat_map.is_empty()
    }

    /// Get number of nodes with geographic hints
    #[must_use]
    pub fn len(&self) -> usize {
        self.lonlat_map.len()
    }

    /// Get preferred direction from one node to another based on geography
    /// Returns angle in radians where 0 = East, -π/2 = North (screen up)
    #[must_use]
    pub fn preferred_direction(&self, from: NodeIndex, to: NodeIndex) -> Option<f64> {
        let from_lonlat = self.lonlat_map.get(&from)?;
        let to_lonlat = self.lonlat_map.get(&to)?;

        let dx = to_lonlat.0 - from_lonlat.0;  // East is positive
        let dy = from_lonlat.1 - to_lonlat.1;  // Invert: North (higher lat) = negative Y (up on screen)

        let angle = dy.atan2(dx);

        // Debug: log geographic direction calculations for specific stations
        // Uncomment to debug geographic calculations
        // println!(
        //     "GEO: ({:.4}, {:.4}) -> ({:.4}, {:.4}), dx={:.4}, dy={:.4}, angle={:.0}°",
        //     from_lonlat.0, from_lonlat.1,
        //     to_lonlat.0, to_lonlat.1,
        //     dx, dy,
        //     angle.to_degrees()
        // );

        Some(angle)
    }
}

/// Snap angle to nearest 45° compass direction
fn snap_to_compass(angle: f64) -> f64 {
    DIRECTIONS.iter()
        .min_by(|&&a, &&b| {
            angle_difference(angle, a)
                .partial_cmp(&angle_difference(angle, b))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .unwrap_or(0.0)
}

// 8 compass directions (45° increments)
const DIRECTIONS: [f64; 8] = [
    0.0,                                    // E (0°)
    std::f64::consts::FRAC_PI_4,           // SE (45°)
    std::f64::consts::FRAC_PI_2,           // S (90°)
    3.0 * std::f64::consts::FRAC_PI_4,     // SW (135°)
    std::f64::consts::PI,                  // W (180°)
    -3.0 * std::f64::consts::FRAC_PI_4,    // NW (-135°)
    -std::f64::consts::FRAC_PI_2,          // N (-90°)
    -std::f64::consts::FRAC_PI_4,          // NE (-45°)
];

/// Filter compass directions to only those consistent with geographic relationship.
/// Returns directions within 90° of the true geographic direction.
/// This ensures stations are never placed opposite to their real-world positions.
fn filter_valid_directions(
    geo_hints: &GeographicHints,
    from_node: NodeIndex,
    to_node: NodeIndex,
) -> Vec<f64> {
    let Some(geo_dir) = geo_hints.preferred_direction(from_node, to_node) else {
        return DIRECTIONS.to_vec(); // No geo data, all directions valid
    };

    // Filter to directions within 90° of the true geographic direction
    DIRECTIONS
        .iter()
        .copied()
        .filter(|&dir| angle_difference(dir, geo_dir) <= std::f64::consts::FRAC_PI_2)
        .collect()
}

/// Get the best compass direction for spine placement based on geographic hints.
/// Returns the direction closest to the true geographic direction from valid options.
fn get_spine_direction(
    geo_hints: &GeographicHints,
    from_node: NodeIndex,
    to_node: NodeIndex,
    fallback: f64,
) -> f64 {
    let valid_dirs = filter_valid_directions(geo_hints, from_node, to_node);
    let Some(geo_dir) = geo_hints.preferred_direction(from_node, to_node) else {
        return fallback;
    };

    // Pick the valid direction closest to the geographic direction
    valid_dirs
        .iter()
        .copied()
        .min_by(|&a, &b| {
            angle_difference(a, geo_dir)
                .partial_cmp(&angle_difference(b, geo_dir))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(fallback)
}

/// Snap coordinates to grid intersections
#[must_use]
pub fn snap_to_grid(x: f64, y: f64) -> (f64, f64) {
    let snapped_x = (x / GRID_SIZE).round() * GRID_SIZE;
    let snapped_y = (y / GRID_SIZE).round() * GRID_SIZE;
    (snapped_x, snapped_y)
}

/// Get all nodes reachable from `start_node`, excluding path back through `exclude_node`
fn get_reachable_nodes(
    graph: &RailwayGraph,
    start_node: NodeIndex,
    exclude_node: Option<NodeIndex>,
) -> HashSet<NodeIndex> {
    let mut reachable = HashSet::new();
    let mut queue = std::collections::VecDeque::new();

    queue.push_back(start_node);
    reachable.insert(start_node);

    while let Some(current) = queue.pop_front() {
        // Get all neighbors (undirected)
        for neighbor in graph.graph.neighbors_undirected(current) {
            // Skip the excluded node
            if Some(neighbor) == exclude_node {
                continue;
            }

            // Skip already visited
            if reachable.contains(&neighbor) {
                continue;
            }

            reachable.insert(neighbor);
            queue.push_back(neighbor);
        }
    }

    reachable
}

/// Calculate how different two node sets are (0.0 = identical, 1.0 = completely different)
#[allow(clippy::cast_precision_loss)]
fn region_difference(set1: &HashSet<NodeIndex>, set2: &HashSet<NodeIndex>) -> f64 {
    if set1.is_empty() && set2.is_empty() {
        return 0.0;
    }

    let intersection_size = set1.intersection(set2).count();
    let union_size = set1.union(set2).count();

    if union_size == 0 {
        return 0.0;
    }

    // Jaccard distance
    1.0 - (intersection_size as f64 / union_size as f64)
}

/// Check if a position has node collision with existing nodes
fn has_node_collision_at(
    graph: &RailwayGraph,
    test_pos: (f64, f64),
    exclude_node: NodeIndex,
    base_station_spacing: f64,
) -> bool {
    for node_idx in graph.graph.node_indices() {
        if node_idx == exclude_node {
            continue;
        }
        if let Some(existing_pos) = graph.get_station_position(node_idx) {
            if existing_pos == (0.0, 0.0) {
                continue;
            }

            // CRITICAL: Never allow exact same position
            if test_pos == existing_pos {
                return true;
            }

            let dx = test_pos.0 - existing_pos.0;
            let dy = test_pos.1 - existing_pos.1;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < base_station_spacing * 0.9 {
                return true;
            }
        }
    }
    false
}

/// Check if a line segment would cross or come too close to any existing edges or nodes
#[allow(clippy::similar_names)]
fn would_overlap_existing_edges(
    graph: &RailwayGraph,
    pos1: (f64, f64),
    pos2: (f64, f64),
) -> bool {
    const MIN_DISTANCE: f64 = 50.0; // Minimum distance between parallel segments

    // Check all existing edges
    for edge in graph.graph.edge_references() {
        let source = edge.source();
        let target = edge.target();

        let Some(source_pos) = graph.get_station_position(source) else { continue };
        let Some(target_pos) = graph.get_station_position(target) else { continue };

        // Skip if either endpoint not placed yet
        if source_pos == (0.0, 0.0) || target_pos == (0.0, 0.0) {
            continue;
        }

        // Check if edges share an endpoint
        let shared_endpoint = if source_pos == pos1 || target_pos == pos1 {
            Some(pos1)
        } else if source_pos == pos2 || target_pos == pos2 {
            Some(pos2)
        } else {
            None
        };

        if let Some(shared_point) = shared_endpoint {
            // Edges share an endpoint - check angle between them
            let new_edge_other = if pos1 == shared_point { pos2 } else { pos1 };
            let existing_edge_other = if source_pos == shared_point { target_pos } else { source_pos };

            // Get direction vectors (normalized)
            let new_dx = new_edge_other.0 - shared_point.0;
            let new_dy = new_edge_other.1 - shared_point.1;
            let new_len = (new_dx * new_dx + new_dy * new_dy).sqrt();

            let exist_dx = existing_edge_other.0 - shared_point.0;
            let exist_dy = existing_edge_other.1 - shared_point.1;
            let exist_len = (exist_dx * exist_dx + exist_dy * exist_dy).sqrt();

            if new_len > 0.1 && exist_len > 0.1 {
                // Normalize direction vectors
                let new_dir_x = new_dx / new_len;
                let new_dir_y = new_dy / new_len;
                let exist_dir_x = exist_dx / exist_len;
                let exist_dir_y = exist_dy / exist_len;

                // Calculate angle between directions using atan2
                let new_angle = new_dir_y.atan2(new_dir_x);
                let exist_angle = exist_dir_y.atan2(exist_dir_x);

                let angle_diff = angle_difference(new_angle, exist_angle);

                // If angle difference is less than 45 degrees (π/4), edges are too close
                if angle_diff < std::f64::consts::FRAC_PI_4 {
                    return true;
                }
            }
        } else {
            // Edges don't share an endpoint - check normal segment distance
            let dist = line_segment_distance(pos1, pos2, source_pos, target_pos);
            if dist < MIN_DISTANCE {
                return true;
            }
        }
    }

    // NOTE: Edge-to-node collision detection disabled for now
    // The renderer has avoidance logic that handles visual overlaps
    // Enabling this check causes too many placement failures, forcing nodes into fallback
    // which creates horizontal lines

    false
}

/// Find a valid fallback position using a spiral search pattern
/// This prevents creating long horizontal lines by varying the search direction
fn find_fallback_position(
    graph: &RailwayGraph,
    current_pos: (f64, f64),
    neighbor: NodeIndex,
    base_station_spacing: f64,
    preferred_direction: f64,
    direction_offset: usize,
) -> Option<(f64, f64)> {
    // Use smaller spacing multipliers so nodes aren't placed too far away
    for &spacing_mult in &[2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0] {
        // Try the preferred direction first
        let test_pos = snap_to_grid(
            current_pos.0 + preferred_direction.cos() * base_station_spacing * spacing_mult,
            current_pos.1 + preferred_direction.sin() * base_station_spacing * spacing_mult,
        );
        if !has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
            return Some(test_pos);
        }

        // Then try perpendicular directions (90° rotations)
        for angle_offset in [std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2, std::f64::consts::PI] {
            let test_dir = preferred_direction + angle_offset;
            let test_pos = snap_to_grid(
                current_pos.0 + test_dir.cos() * base_station_spacing * spacing_mult,
                current_pos.1 + test_dir.sin() * base_station_spacing * spacing_mult,
            );
            if !has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
                return Some(test_pos);
            }
        }

        // Finally try all 8 compass directions, rotated by direction_offset
        for i in 0..DIRECTIONS.len() {
            let dir = DIRECTIONS[(i + direction_offset) % DIRECTIONS.len()];
            let test_pos = snap_to_grid(
                current_pos.0 + dir.cos() * base_station_spacing * spacing_mult,
                current_pos.1 + dir.sin() * base_station_spacing * spacing_mult,
            );
            if !has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
                return Some(test_pos);
            }
        }
    }
    None
}

/// Find best direction and spacing for a branch node
#[allow(clippy::too_many_arguments)]
fn find_best_direction_for_branch(
    graph: &RailwayGraph,
    current_node: NodeIndex,
    current_pos: (f64, f64),
    neighbor: NodeIndex,
    target_pos: Option<(f64, f64)>,
    neighbor_reachable: &HashSet<NodeIndex>,
    already_used: &[(f64, HashSet<NodeIndex>)],
    incoming_direction: f64,
    base_station_spacing: f64,
    is_through_path: bool,
    geo_hints: Option<&GeographicHints>,
) -> (f64, f64, i32) {
    let debug_this = graph.graph[neighbor].display_name() == "Upper Tyndrum";

    // If this is a through path at a junction, continue straight in the incoming direction
    if is_through_path {
        // Try spacing multipliers to avoid collisions
        for spacing_mult in [1.0, 1.5, 2.0, 2.5, 3.0] {
            let test_pos = snap_to_grid(
                current_pos.0 + incoming_direction.cos() * base_station_spacing * spacing_mult,
                current_pos.1 + incoming_direction.sin() * base_station_spacing * spacing_mult,
            );

            if !has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
                // Found a valid position continuing straight
                return (incoming_direction, spacing_mult, 1000);
            }
        }
        // If all straight positions have collisions, fall through to regular algorithm
    }

    let mut best_direction = DIRECTIONS[0];
    let mut best_score = i32::MIN;
    let mut best_spacing = 1.0;

    // Calculate direction to target if it exists
    let target_direction = target_pos.map(|target| {
        let dx = target.0 - current_pos.0;
        let dy = target.1 - current_pos.1;
        dy.atan2(dx)
    });

    // Filter directions to only those consistent with geography (hard constraint)
    let valid_directions: Vec<f64> = geo_hints.map_or_else(
        || DIRECTIONS.to_vec(),
        |h| filter_valid_directions(h, current_node, neighbor),
    );

    // Try spacing multipliers from 1.0 up to 10.0
    for spacing_mult in [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 7.0, 10.0] {
        let mut best_at_this_spacing = i32::MIN;

        if debug_this {
            leptos::logging::log!("  Trying spacing multiplier: {:.1}", spacing_mult);
        }

        for &direction in &valid_directions {
            let test_pos = snap_to_grid(
                current_pos.0 + direction.cos() * base_station_spacing * spacing_mult,
                current_pos.1 + direction.sin() * base_station_spacing * spacing_mult,
            );

            if has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
                if debug_this {
                    leptos::logging::log!("    {:.0}°: COLLISION", direction.to_degrees());
                }
                continue;
            }

            // If node has a target, apply constraint based on whether we have geography
            // With geography: soft constraint (penalty in scoring) - geography is ground truth
            // Without geography: hard constraint (reject) - target is our best guess
            if let Some(target_dir) = target_direction {
                let angle_to_target = angle_difference(direction, target_dir);

                // If we're moving away from target (> 90°)
                #[allow(clippy::excessive_nesting)]
                if angle_to_target > std::f64::consts::FRAC_PI_2 {
                    // With geographic hints, we trust geography over target position
                    // (target position comes from earlier layout decisions that might be wrong)
                    // So we only soft-penalize via score, not hard reject
                    if geo_hints.is_none() {
                        continue;
                    }
                    // With geo_hints, penalty is applied via score_direction_for_branch
                }
            }

            // Get geographic preferred direction if hints available
            let geo_preferred = geo_hints.and_then(|h| h.preferred_direction(current_node, neighbor));

            let score = score_direction_for_branch(
                graph,
                current_pos,
                direction,
                spacing_mult,
                target_direction,
                neighbor_reachable,
                already_used,
                incoming_direction,
                base_station_spacing,
                geo_preferred,
            );

            if debug_this {
                leptos::logging::log!("    {:.0}°: score={}", direction.to_degrees(), score);
            }

            if score > best_score {
                best_score = score;
                best_direction = direction;
                best_spacing = spacing_mult;
            }

            if score > best_at_this_spacing {
                best_at_this_spacing = score;
            }
        }

        // If we found any valid direction at this spacing level, return the best one
        if best_at_this_spacing > i32::MIN {
            if debug_this {
                leptos::logging::log!("  Found valid direction at spacing {:.1}: {:.0}° (score={})",
                    spacing_mult, best_direction.to_degrees(), best_score);
            }
            return (best_direction, best_spacing, best_score);
        }
    }

    (best_direction, best_spacing, best_score)
}

/// Score a direction for placing a branch node
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::too_many_arguments)]
fn score_direction_for_branch(
    graph: &RailwayGraph,
    current_pos: (f64, f64),
    direction: f64,
    spacing_multiplier: f64,
    target_direction: Option<f64>,
    neighbor_reachable: &HashSet<NodeIndex>,
    already_used: &[(f64, HashSet<NodeIndex>)],
    incoming_direction: f64,
    base_station_spacing: f64,
    geo_preferred_direction: Option<f64>,
) -> i32 {
    let mut score = 0;

    // DEBUG: Log scoring details
    let debug = false; // Set to true to enable debug logging

    // Calculate proposed position
    let neighbor_pos = snap_to_grid(
        current_pos.0 + direction.cos() * base_station_spacing * spacing_multiplier,
        current_pos.1 + direction.sin() * base_station_spacing * spacing_multiplier,
    );

    // CRITICAL: Check for geometric overlap with existing edges
    if would_overlap_existing_edges(graph, current_pos, neighbor_pos) {
        return i32::MIN;
    }

    // Check if this direction goes back where we came from (opposite of incoming)
    let reverse_direction = incoming_direction + std::f64::consts::PI;
    let reverse_angle_diff = angle_difference(direction, reverse_direction);

    // CRITICAL: Never go back in the direction we came from (causes overlap)
    if reverse_angle_diff < std::f64::consts::FRAC_PI_4 {
        return i32::MIN;
    }

    // Continuity bonus: reward directions similar to incoming direction
    // This prevents zigzagging when geography changes slightly between segments
    let continuity_angle_diff = angle_difference(direction, incoming_direction);
    let continuity_bonus = ((std::f64::consts::PI - continuity_angle_diff)
        / std::f64::consts::PI
        * CONTINUITY_BONUS_MAX) as i32;
    score += continuity_bonus;

    // If we have a target, strongly prefer moving towards it
    if let Some(target_dir) = target_direction {
        let angle_to_target = angle_difference(direction, target_dir);

        // Strong bonus for moving towards target
        score += ((std::f64::consts::PI - angle_to_target) * 2000.0) as i32;
    }

    // If we have geographic hints, add bonus for directions matching real-world geography
    if let Some(geo_dir) = geo_preferred_direction {
        // Snap geographic direction to nearest compass direction
        let compass_geo = snap_to_compass(geo_dir);
        let angle_diff = angle_difference(direction, compass_geo);

        // Strong bonus for directions matching geographic direction
        // This is stronger than the target bonus (2000) so geography has significant influence
        let geo_bonus = ((std::f64::consts::PI - angle_diff) / std::f64::consts::PI * GEOGRAPHIC_BONUS_MAX) as i32;
        score += geo_bonus;

        if debug {
            leptos::logging::log!("    geo_dir={:.0}°, compass_geo={:.0}°, angle_diff={:.0}°, geo_bonus=+{}",
                geo_dir.to_degrees(), compass_geo.to_degrees(), angle_diff.to_degrees(), geo_bonus);
        }
    }

    // Count branches in similar direction to apply crowding penalty
    let mut branches_in_hemisphere = 0;

    // For each already-used direction
    for (used_dir, used_reachable) in already_used {
        let angle_diff = angle_difference(direction, *used_dir);
        let region_diff = region_difference(neighbor_reachable, used_reachable);

        if debug {
            leptos::logging::log!("    existing branch: dir={:.0}°, angle_diff={:.0}°, region_diff={:.2}",
                used_dir.to_degrees(), angle_diff.to_degrees(), region_diff);
        }

        // Count how many branches are in same hemisphere (within 90°)
        if angle_diff < std::f64::consts::FRAC_PI_2 {
            branches_in_hemisphere += 1;
        }

        // If regions are SIMILAR and directions are SIMILAR = strongly encourage this
        // Branches that reconnect should be on the same side
        if region_diff < 0.3 && angle_diff < std::f64::consts::FRAC_PI_4 {
            let bonus = ((1.0 - region_diff) * 3000.0) as i32;
            if debug {
                leptos::logging::log!("      SIMILAR regions + SIMILAR direction: +{}", bonus);
            }
            score += bonus;
        }

        // If regions are DIFFERENT but directions are SIMILAR = bad
        if region_diff > 0.5 && angle_diff < std::f64::consts::FRAC_PI_2 {
            let penalty = ((1.0 - region_diff) * 5000.0) as i32;
            if debug {
                leptos::logging::log!("      DIFFERENT regions + SIMILAR direction: -{}", penalty);
            }
            score -= penalty;
        }

        // Prefer larger angular separation for DIFFERENT regions
        // But reduce this bonus for similar regions
        if region_diff > 0.3 {
            let bonus = (angle_diff * 500.0) as i32;
            if debug {
                leptos::logging::log!("      DIFFERENT regions angular sep: +{}", bonus);
            }
            score += bonus;
        }
    }

    // Apply crowding penalty: penalize directions with many existing branches
    // This naturally balances branches across sides
    let crowding_penalty = branches_in_hemisphere * 400;
    if debug {
        leptos::logging::log!("    branches_in_hemisphere={}, crowding_penalty=-{}",
            branches_in_hemisphere, crowding_penalty);
        leptos::logging::log!("    final score={}", score - crowding_penalty);
    }
    score -= crowding_penalty;

    score
}

/// Apply layout, preserving positions of pinned nodes
#[allow(clippy::too_many_lines, clippy::missing_panics_doc, clippy::cast_precision_loss)]
pub fn apply_layout_with_pinned(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &std::collections::HashSet<NodeIndex>,
) {
    apply_layout_internal(graph, height, settings, geo_hints, pinned_nodes);
}

#[allow(clippy::too_many_lines, clippy::missing_panics_doc, clippy::cast_precision_loss)]
pub fn apply_layout(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
) {
    apply_layout_internal(graph, height, settings, geo_hints, &std::collections::HashSet::new());
}

#[allow(clippy::too_many_lines, clippy::missing_panics_doc, clippy::cast_precision_loss)]
fn apply_layout_internal(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &std::collections::HashSet<NodeIndex>,
) {
    let base_station_spacing = settings.default_node_distance_grid_squares * GRID_SIZE;
    let start_x = 150.0;
    let start_y = height / 2.0;

    if graph.graph.node_count() == 0 {
        return; // Empty graph
    }

    // Clear positions for nodes that will be laid out (skip pinned and passing loops)
    let all_nodes: Vec<_> = graph.graph.node_indices().collect();
    for node_idx in all_nodes {
        // Skip pinned nodes - their positions are preserved
        if pinned_nodes.contains(&node_idx) {
            continue;
        }
        // Skip passing loops - they will be automatically positioned between adjacent stations
        if let Some(node) = graph.graph.node_weight(node_idx) {
            if let Some(station) = node.as_station() {
                if station.passing_loop {
                    continue;
                }
            }
        }
        graph.set_station_position(node_idx, (0.0, 0.0));
    }

    // Phase 1: Find longest path (the main spine)
    let spine = graph.find_longest_path();

    if spine.is_empty() {
        return;
    }

    // Phase 2: Place spine - each segment follows its own geographic direction
    let mut visited = HashSet::new();
    let default_direction = -std::f64::consts::FRAC_PI_2; // North (-90°) as fallback

    let mut current_pos = snap_to_grid(start_x, start_y);
    let mut prev_node: Option<NodeIndex> = None;
    let mut last_direction = default_direction;

    for &node in &spine {
        // Check if this is a passing loop
        let is_passing_loop = graph.graph.node_weight(node)
            .and_then(|n| n.as_station())
            .is_some_and(|s| s.passing_loop);

        // Check if this node is pinned (has existing position we should preserve)
        let is_pinned = pinned_nodes.contains(&node);

        if !is_passing_loop {
            if is_pinned {
                // Use pinned node's existing position as reference
                if let Some(pos) = graph.graph.node_weight(node)
                    .and_then(|n| n.as_station())
                    .and_then(|s| s.position)
                {
                    current_pos = pos;
                }
            } else if let Some(prev) = prev_node {
                // Get direction filtered by geography (hard constraint)
                let direction = geo_hints.map_or(last_direction, |hints| {
                    get_spine_direction(hints, prev, node, last_direction)
                });

                current_pos = snap_to_grid(
                    current_pos.0 + direction.cos() * base_station_spacing,
                    current_pos.1 + direction.sin() * base_station_spacing,
                );
                last_direction = direction;
                // Place spine node at current position
                graph.set_station_position(node, current_pos);
            } else {
                // First node, no previous - just place it
                graph.set_station_position(node, current_pos);
            }
            prev_node = Some(node);
        }
        visited.insert(node);
    }

    // Phase 3: Place branches from spine nodes
    let mut queue = std::collections::VecDeque::new();
    let mut fallback_direction_index: usize = 0; // Cycle through directions for fallback

    // Track ALL branch directions globally (not just per-parent-node)
    // This enables the crowding penalty to balance branches across the entire graph
    let mut global_branches: Vec<(f64, HashSet<NodeIndex>)> = Vec::new();

    // Add all spine nodes to queue with their positions, incoming direction, and incoming edge
    for (i, &node) in spine.iter().enumerate() {
        if let Some(pos) = graph.get_station_position(node) {
            // Find the incoming edge and direction (from previous spine node)
            let (incoming_edge, incoming_dir) = if i > 0 {
                let prev_node = spine[i - 1];
                let edge = graph
                    .graph
                    .edges_connecting(prev_node, node)
                    .next()
                    .or_else(|| graph.graph.edges_connecting(node, prev_node).next())
                    .map(|e| e.id());
                // Compute direction from previous node's position
                let dir = geo_hints
                    .and_then(|h| h.preferred_direction(prev_node, node))
                    .map_or(default_direction, snap_to_compass);
                (edge, dir)
            } else {
                (None, default_direction)
            };
            queue.push_back((node, pos, incoming_dir, incoming_edge));
        }
    }

    while let Some((current_node, current_pos, incoming_direction, incoming_edge)) = queue.pop_front() {
        // Get all unvisited neighbors
        let neighbors: Vec<_> = graph
            .graph
            .neighbors_undirected(current_node)
            .filter(|n| !visited.contains(n))
            .collect();

        if neighbors.is_empty() {
            continue;
        }

        // Track which directions we've assigned from this specific node
        let mut local_branches: Vec<(f64, HashSet<NodeIndex>)> = Vec::new();

        for &neighbor in &neighbors {
            // Check if neighbor has any edges to already-placed nodes (besides current)
            let target_pos = find_placed_target(graph, neighbor, current_node, &visited);

            let reachable = get_reachable_nodes(graph, neighbor, Some(current_node));

            // Find the edge from current_node to neighbor
            let edge_to_neighbor = graph
                .graph
                .edges_connecting(current_node, neighbor)
                .next()
                .or_else(|| graph.graph.edges_connecting(neighbor, current_node).next())
                .map(|e| e.id());

            // Check if this neighbor is on a "through path" at a junction
            // by checking if the incoming edge and outgoing edge form a bidirectional path
            let is_through_path = match (incoming_edge, edge_to_neighbor) {
                (Some(inc_edge), Some(out_edge)) if graph.is_junction(current_node) => {
                    graph.get_junction(current_node).is_some_and(|junction| {
                        // Check if both directions are allowed (bidirectional through path)
                        junction.is_routing_allowed(inc_edge, out_edge)
                            && junction.is_routing_allowed(out_edge, inc_edge)
                    })
                }
                _ => false,
            };

            let (best_direction, best_spacing, best_score) = find_best_direction_for_branch(
                graph,
                current_node,
                current_pos,
                neighbor,
                target_pos,
                &reachable,
                &global_branches,  // Use global branches, not local
                incoming_direction,
                base_station_spacing,
                is_through_path,
                geo_hints,
            );

            // DEBUG: Log when placing specific nodes
            if graph.graph[neighbor].display_name() == "Upper Tyndrum" {
                leptos::logging::log!("Placing {} from {} at ({:.1}, {:.1})",
                    graph.graph[neighbor].display_name(),
                    graph.graph[current_node].display_name(),
                    current_pos.0, current_pos.1);
                leptos::logging::log!("  Best direction: {:.0}°, spacing: {:.1}, score: {}",
                    best_direction.to_degrees(), best_spacing, best_score);
                leptos::logging::log!("  Global branches: {} total", global_branches.len());
                for (dir, _) in &global_branches {
                    leptos::logging::log!("    - {:.0}°", dir.to_degrees());
                }
            }

            let neighbor_pos = snap_to_grid(
                current_pos.0 + best_direction.cos() * base_station_spacing * best_spacing,
                current_pos.1 + best_direction.sin() * base_station_spacing * best_spacing,
            );

            // Verify the final position doesn't have collision before placing
            let final_pos = if has_node_collision_at(graph, neighbor_pos, neighbor, base_station_spacing) || best_score == i32::MIN {
                // All positions have collisions - try fallback positions
                let fallback_dir = if best_score == i32::MIN {
                    // Cycle through directions to prevent horizontal lines
                    let dir = DIRECTIONS[fallback_direction_index % DIRECTIONS.len()];
                    fallback_direction_index += 1;
                    dir
                } else {
                    best_direction
                };
                // Pass the direction offset to rotate through compass directions
                let result = find_fallback_position(
                    graph,
                    current_pos,
                    neighbor,
                    base_station_spacing,
                    fallback_dir,
                    fallback_direction_index
                );
                fallback_direction_index += 1;
                result
            } else {
                Some(neighbor_pos)
            };

            // Check if neighbor is a passing loop - skip positioning if so
            let is_passing_loop = graph.graph.node_weight(neighbor)
                .and_then(|n| n.as_station())
                .is_some_and(|s| s.passing_loop);

            // Check if neighbor is pinned (preserve its existing position)
            let is_pinned = pinned_nodes.contains(&neighbor);

            if is_passing_loop {
                // Passing loop - mark as visited but don't position it
                visited.insert(neighbor);
                // Still add to queue so we can process its children
                // Use parent position as placeholder for queue processing
                queue.push_back((neighbor, current_pos, incoming_direction, edge_to_neighbor));
            } else if is_pinned {
                // Pinned node - use existing position, don't reposition
                let pinned_pos = graph.graph.node_weight(neighbor)
                    .and_then(|n| n.as_station())
                    .and_then(|s| s.position)
                    .unwrap_or(current_pos);
                visited.insert(neighbor);
                let dir_to_pinned = (pinned_pos.1 - current_pos.1).atan2(pinned_pos.0 - current_pos.0);
                local_branches.push((dir_to_pinned, reachable.clone()));
                global_branches.push((dir_to_pinned, reachable.clone()));
                queue.push_back((neighbor, pinned_pos, dir_to_pinned, edge_to_neighbor));
            } else if let Some(pos) = final_pos {
                graph.set_station_position(neighbor, pos);
                visited.insert(neighbor);
                // Track both locally (for this parent) and globally (for crowding penalty)
                local_branches.push((best_direction, reachable.clone()));
                global_branches.push((best_direction, reachable.clone()));
                queue.push_back((neighbor, pos, best_direction, edge_to_neighbor));
            } else {
                // Absolutely no valid position found - this should be extremely rare
                // Use a varied emergency direction
                let emergency_dir = DIRECTIONS[fallback_direction_index % DIRECTIONS.len()];
                fallback_direction_index += 1;
                let emergency_pos = snap_to_grid(
                    current_pos.0 + emergency_dir.cos() * base_station_spacing * 20.0,
                    current_pos.1 + emergency_dir.sin() * base_station_spacing * 20.0,
                );
                graph.set_station_position(neighbor, emergency_pos);
                visited.insert(neighbor);
                local_branches.push((emergency_dir, reachable.clone()));
                global_branches.push((emergency_dir, reachable.clone()));
                queue.push_back((neighbor, emergency_pos, emergency_dir, edge_to_neighbor));
            }
        }
    }

    // Phase 4: Handle disconnected components
    let disconnected: Vec<_> = graph
        .graph
        .node_indices()
        .filter(|idx| !visited.contains(idx))
        .collect();

    if !disconnected.is_empty() {
        let mut offset_x = start_x + 400.0;

        for &node in &disconnected {
            if visited.contains(&node) {
                continue;
            }

            // Find longest path in this disconnected component
            let component_spine = graph.find_longest_path_from(node, &visited);

            let mut comp_non_passing_count = 0;
            for &comp_node in &component_spine {
                // Check if this is a passing loop
                let is_passing_loop = graph.graph.node_weight(comp_node)
                    .and_then(|n| n.as_station())
                    .is_some_and(|s| s.passing_loop);

                let is_pinned = pinned_nodes.contains(&comp_node);

                if !is_passing_loop && !is_pinned {
                    let offset = f64::from(comp_non_passing_count) * base_station_spacing;
                    let pos = snap_to_grid(
                        offset_x,
                        start_y + default_direction.sin() * offset,
                    );

                    // Place disconnected components without adjustment - they're offset far enough
                    graph.set_station_position(comp_node, pos);
                    comp_non_passing_count += 1;
                } else if !is_passing_loop {
                    comp_non_passing_count += 1;
                }
                visited.insert(comp_node);
            }

            offset_x += 600.0; // Increased spacing between disconnected components
        }
    }
}

/// Find if a node has any connections to already-placed nodes (excluding current)
fn find_placed_target(
    graph: &RailwayGraph,
    node: NodeIndex,
    exclude: NodeIndex,
    visited: &HashSet<NodeIndex>,
) -> Option<(f64, f64)> {
    for neighbor in graph.graph.neighbors_undirected(node) {
        if neighbor != exclude && visited.contains(&neighbor) {
            if let Some(pos) = graph.get_station_position(neighbor) {
                if pos != (0.0, 0.0) {
                    return Some(pos);
                }
            }
        }
    }
    None
}

pub fn adjust_layout(_graph: &mut RailwayGraph) {
    // TODO: Implement smart adjustment
}

/// Snap station to grid when manually dragging (with branch reorientation)
pub fn snap_to_angle(graph: &mut RailwayGraph, station_idx: NodeIndex, x: f64, y: f64) {
    let snapped = snap_to_grid(x, y);
    graph.set_station_position(station_idx, snapped);
}

/// Snap station to grid when manually dragging (along branch)
pub fn snap_station_along_branch(graph: &mut RailwayGraph, station_idx: NodeIndex, x: f64, y: f64) {
    let snapped = snap_to_grid(x, y);
    graph.set_station_position(station_idx, snapped);
}
