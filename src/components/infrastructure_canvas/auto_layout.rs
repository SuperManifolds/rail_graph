use crate::models::{RailwayGraph, Stations, Junctions, ProjectSettings};
use crate::geometry::angle_difference;
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::collections::{HashMap, HashSet};

pub const GRID_SIZE: f64 = 30.0;

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
    let neighbor_name = graph.graph[neighbor].display_name();
    let debug_this = neighbor_name == "Ski" || neighbor_name == "Upper Tyndrum";

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

    // Get geographic preferred direction and snap to nearest compass direction
    let geo_preferred = geo_hints.and_then(|h| h.preferred_direction(current_node, neighbor));

    // Filter directions to only those consistent with geography (hard constraint)
    let valid_directions: Vec<f64> = geo_hints.map_or_else(
        || DIRECTIONS.to_vec(),
        |h| filter_valid_directions(h, current_node, neighbor),
    );

    // Directions to avoid: both incoming (would overlap spine) and reverse (would go back)
    // This ensures branches go PERPENDICULAR to the spine, not along it
    let reverse_direction = incoming_direction + std::f64::consts::PI;

    // Sort valid directions by closeness to geographic direction, excluding spine directions
    let priority_directions: Vec<f64> = if let Some(geo_dir) = geo_preferred {
        let mut sorted_dirs: Vec<_> = valid_directions.iter()
            .filter(|&&d| {
                // Exclude directions too close to incoming (would continue along spine)
                let too_close_to_incoming = angle_difference(d, incoming_direction) < std::f64::consts::FRAC_PI_4;
                // Exclude directions too close to reverse (would go back)
                let too_close_to_reverse = angle_difference(d, reverse_direction) < std::f64::consts::FRAC_PI_4;
                !too_close_to_incoming && !too_close_to_reverse
            })
            .copied()
            .collect();
        sorted_dirs.sort_by(|&a, &b| {
            angle_difference(a, geo_dir)
                .partial_cmp(&angle_difference(b, geo_dir))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted_dirs
    } else {
        Vec::new()
    };

    // PRIORITY 1: Try geographic-priority directions in order at all spacings
    // This ensures stations are placed in their geographic direction even if it requires more spacing
    for &pref_dir in &priority_directions {
        if debug_this {
            leptos::logging::log!("  [PRIORITY] Trying direction {:.0}° (geo preferred)", pref_dir.to_degrees());
        }
        for spacing_mult in [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 7.0, 10.0] {
            let test_pos = snap_to_grid(
                current_pos.0 + pref_dir.cos() * base_station_spacing * spacing_mult,
                current_pos.1 + pref_dir.sin() * base_station_spacing * spacing_mult,
            );

            if debug_this {
                leptos::logging::log!("    spacing {:.1}: testing ({:.0}, {:.0})", spacing_mult, test_pos.0, test_pos.1);
            }

            if has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
                if debug_this {
                    leptos::logging::log!("      COLLISION");
                }
                continue;
            }

            // Found a valid position in this direction
            let score = score_direction_for_branch(
                graph,
                current_pos,
                pref_dir,
                spacing_mult,
                target_direction,
                neighbor_reachable,
                already_used,
                incoming_direction,
                base_station_spacing,
                geo_preferred,
            );

            if debug_this {
                leptos::logging::log!("      SUCCESS, score={}", score);
            }

            return (pref_dir, spacing_mult, score);
        }
        if debug_this {
            leptos::logging::log!("    Direction {:.0}° blocked at all spacings", pref_dir.to_degrees());
        }
    }

    // PRIORITY 2: Try remaining valid directions at increasing spacings
    // (skip those already exhausted in priority phase)
    for spacing_mult in [1.0, 1.5, 2.0, 2.5, 3.0, 4.0, 5.0, 7.0, 10.0] {
        let mut best_at_this_spacing = i32::MIN;

        if debug_this {
            leptos::logging::log!("  Trying spacing multiplier: {:.1}", spacing_mult);
        }

        for &direction in &valid_directions {
            // Skip directions already tried exhaustively in priority phase
            if priority_directions.iter().any(|&d| (d - direction).abs() < 0.01) {
                continue;
            }

            let test_pos = snap_to_grid(
                current_pos.0 + direction.cos() * base_station_spacing * spacing_mult,
                current_pos.1 + direction.sin() * base_station_spacing * spacing_mult,
            );

            if has_node_collision_at(graph, test_pos, neighbor, base_station_spacing) {
                if debug_this {
                    leptos::logging::log!("    {:.0}°: COLLISION at ({:.0}, {:.0})", direction.to_degrees(), test_pos.0, test_pos.1);
                }
                continue;
            }

            if debug_this {
                leptos::logging::log!("    {:.0}°: no collision at ({:.0}, {:.0})", direction.to_degrees(), test_pos.0, test_pos.1);
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
    _graph: &RailwayGraph,
    _current_pos: (f64, f64),
    direction: f64,
    _spacing_multiplier: f64,
    target_direction: Option<f64>,
    neighbor_reachable: &HashSet<NodeIndex>,
    already_used: &[(f64, HashSet<NodeIndex>)],
    incoming_direction: f64,
    _base_station_spacing: f64,
    geo_preferred_direction: Option<f64>,
) -> i32 {
    let mut score = 0;

    // DEBUG: Log scoring details
    let debug = false; // Set to true to enable debug logging

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

/// Represents a pair of stations with both a direct edge and an alternative path with intermediate stations
#[derive(Debug)]
struct ParallelRouteGroup {
    /// First hub station
    hub_a: NodeIndex,
    /// Second hub station
    hub_b: NodeIndex,
    /// Intermediate stations on the local route (excluding hubs)
    local_intermediates: Vec<NodeIndex>,
}

/// Maximum perpendicular distance (in degrees) from direct line for parallel route stations
const MAX_PARALLEL_DEVIATION_DEG: f64 = 0.05; // ~5km at mid-latitudes

/// Find all edges that have an alternative path with intermediate stations.
/// These represent express (direct) vs local (with stops) routes.
/// Uses geographic hints to verify the alternative path stays close to the direct line.
fn detect_parallel_routes(
    graph: &RailwayGraph,
    geo_hints: Option<&GeographicHints>,
) -> Vec<ParallelRouteGroup> {
    let mut parallel_routes = Vec::new();
    let mut processed_pairs: HashSet<(NodeIndex, NodeIndex)> = HashSet::new();

    // Look at each edge in the graph
    for edge in graph.graph.edge_references() {
        let hub_a = edge.source();
        let hub_b = edge.target();

        // Skip if already processed this pair (in either direction)
        let pair = if hub_a < hub_b { (hub_a, hub_b) } else { (hub_b, hub_a) };
        if processed_pairs.contains(&pair) {
            continue;
        }
        processed_pairs.insert(pair);

        // Only consider edges where both endpoints have degree >= 2 (potential hubs)
        let degree_a = graph.graph.neighbors_undirected(hub_a).count();
        let degree_b = graph.graph.neighbors_undirected(hub_b).count();
        if degree_a < 2 || degree_b < 2 {
            continue;
        }

        // Try to find alternative path from hub_a to hub_b that doesn't use the direct edge
        if let Some(alt_path) = find_alternative_path(graph, hub_a, hub_b) {
            // alt_path includes hub_a and hub_b, extract intermediates
            if alt_path.len() > 2 {
                let local_intermediates: Vec<NodeIndex> = alt_path[1..alt_path.len()-1].to_vec();

                // Verify all intermediates are geographically close to the direct line
                if is_path_geographically_parallel(geo_hints, hub_a, hub_b, &local_intermediates) {
                    parallel_routes.push(ParallelRouteGroup {
                        hub_a,
                        hub_b,
                        local_intermediates,
                    });
                }
            }
        }
    }

    parallel_routes
}

/// Check if all intermediate stations are geographically close to the line between hubs
fn is_path_geographically_parallel(
    geo_hints: Option<&GeographicHints>,
    hub_a: NodeIndex,
    hub_b: NodeIndex,
    intermediates: &[NodeIndex],
) -> bool {
    let Some(hints) = geo_hints else {
        // No geographic data - can't verify, so reject
        return false;
    };

    // Get hub coordinates
    let Some(&(lon_a, lat_a)) = hints.lonlat_map.get(&hub_a) else { return false };
    let Some(&(lon_b, lat_b)) = hints.lonlat_map.get(&hub_b) else { return false };

    // Direction vector from A to B
    let dx = lon_b - lon_a;
    let dy = lat_b - lat_a;
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-10 {
        return false; // Hubs too close
    }

    // Check each intermediate
    for &node in intermediates {
        let Some(&(lon, lat)) = hints.lonlat_map.get(&node) else { return false };

        // Vector from A to this point
        let px = lon - lon_a;
        let py = lat - lat_a;

        // Project onto line AB to find closest point
        let t = (px * dx + py * dy) / len_sq;

        // Closest point on line
        let closest_x = lon_a + t * dx;
        let closest_y = lat_a + t * dy;

        // Perpendicular distance
        let dist = ((lon - closest_x).powi(2) + (lat - closest_y).powi(2)).sqrt();

        if dist > MAX_PARALLEL_DEVIATION_DEG {
            return false; // Too far from direct line
        }
    }

    true
}

/// Find an alternative path between two nodes that doesn't use the direct edge.
/// Returns the path as a vector of node indices including start and end.
fn find_alternative_path(
    graph: &RailwayGraph,
    from: NodeIndex,
    to: NodeIndex,
) -> Option<Vec<NodeIndex>> {
    use std::collections::VecDeque;

    let mut visited: HashSet<NodeIndex> = HashSet::new();
    let mut queue: VecDeque<(NodeIndex, Vec<NodeIndex>)> = VecDeque::new();

    queue.push_back((from, vec![from]));
    visited.insert(from);

    while let Some((current, path)) = queue.pop_front() {
        for neighbor in graph.graph.neighbors_undirected(current) {
            // Skip the direct edge from source to destination
            if current == from && neighbor == to {
                continue;
            }

            if neighbor == to {
                // Found alternative path
                let mut full_path = path.clone();
                full_path.push(to);
                return Some(full_path);
            }

            if !visited.contains(&neighbor) {
                visited.insert(neighbor);
                let mut new_path = path.clone();
                new_path.push(neighbor);
                queue.push_back((neighbor, new_path));
            }
        }
    }

    None
}

/// Maximum geographic distance (in degrees) for stations to be considered part of the same cluster
const MAX_CLUSTER_DISTANCE_DEG: f64 = 0.01; // ~1km at mid-latitudes

/// Cluster spacing in grid squares (1-2 grid squares apart)
const CLUSTER_SPACING_GRIDS: f64 = 1.5;

/// Represents a group of stations with the same name that should be placed close together
#[derive(Debug)]
struct StationCluster {
    nodes: Vec<NodeIndex>,
}

/// Detect stations with the same name that are geographically close.
/// These represent transfer points (e.g., Bryn metro and Bryn tram).
fn detect_station_clusters(
    graph: &RailwayGraph,
    geo_hints: Option<&GeographicHints>,
) -> Vec<StationCluster> {
    let hints = match geo_hints {
        Some(h) if !h.is_empty() => h,
        _ => return Vec::new(), // No geographic data, can't verify proximity
    };

    // Group stations by lowercase name
    let mut name_groups: HashMap<String, Vec<NodeIndex>> = HashMap::new();

    for node_idx in graph.graph.node_indices() {
        let name = graph.graph[node_idx].display_name().to_lowercase();
        name_groups.entry(name).or_default().push(node_idx);
    }

    let mut clusters = Vec::new();

    // For each name group with 2+ stations, verify geographic proximity
    for (_name, nodes) in name_groups {
        if nodes.len() < 2 {
            continue;
        }

        // Check that all nodes are geographically close to each other
        let all_close = nodes.iter().all(|&node_a| {
            let Some(&(lon_a, lat_a)) = hints.lonlat_map.get(&node_a) else {
                return false;
            };
            nodes.iter().all(|&node_b| {
                if node_a == node_b {
                    return true;
                }
                let Some(&(lon_b, lat_b)) = hints.lonlat_map.get(&node_b) else {
                    return false;
                };
                let dist = ((lon_a - lon_b).powi(2) + (lat_a - lat_b).powi(2)).sqrt();
                dist <= MAX_CLUSTER_DISTANCE_DEG
            })
        });

        if all_close {
            clusters.push(StationCluster { nodes });
        }
    }

    clusters
}

/// Build a lookup from node to its cluster index
fn build_cluster_lookup(clusters: &[StationCluster]) -> HashMap<NodeIndex, usize> {
    let mut lookup = HashMap::new();
    for (cluster_idx, cluster) in clusters.iter().enumerate() {
        for &node in &cluster.nodes {
            lookup.insert(node, cluster_idx);
        }
    }
    lookup
}

/// Place secondary nodes of a cluster near the primary (already placed) node
fn place_cluster_secondaries(
    graph: &mut RailwayGraph,
    cluster: &StationCluster,
    primary_node: NodeIndex,
    visited: &mut HashSet<NodeIndex>,
    base_station_spacing: f64,
    pinned_nodes: &HashSet<NodeIndex>,
) {
    let Some(primary_pos) = graph.get_station_position(primary_node) else {
        return;
    };
    if primary_pos == (0.0, 0.0) {
        return;
    }

    let cluster_offset = GRID_SIZE * CLUSTER_SPACING_GRIDS;

    // Place each secondary node around the primary
    for (i, &node) in cluster.nodes.iter().enumerate() {
        if node == primary_node || visited.contains(&node) || pinned_nodes.contains(&node) {
            continue;
        }

        // Offset in different directions for each secondary
        let angle = DIRECTIONS[i % DIRECTIONS.len()];
        let preferred = (
            primary_pos.0 + angle.cos() * cluster_offset,
            primary_pos.1 + angle.sin() * cluster_offset,
        );

        let final_pos = find_non_colliding_position(graph, preferred, node, base_station_spacing);
        graph.set_station_position(node, final_pos);
        visited.insert(node);
    }
}

/// Find a non-colliding position near the preferred position
fn find_non_colliding_position(
    graph: &RailwayGraph,
    preferred: (f64, f64),
    node: NodeIndex,
    base_station_spacing: f64,
) -> (f64, f64) {
    let snapped = snap_to_grid(preferred.0, preferred.1);

    // Check if preferred position is clear
    if !has_node_collision_at(graph, snapped, node, base_station_spacing) {
        return snapped;
    }

    // Try positions in expanding circles around preferred
    for radius_mult in 1..=10 {
        let radius = GRID_SIZE * f64::from(radius_mult);
        for &dir in &DIRECTIONS {
            let test_pos = snap_to_grid(
                preferred.0 + dir.cos() * radius,
                preferred.1 + dir.sin() * radius,
            );
            if !has_node_collision_at(graph, test_pos, node, base_station_spacing) {
                return test_pos;
            }
        }
    }

    // Last resort: return snapped position anyway (shouldn't happen)
    snapped
}

/// Place intermediate stations for parallel routes, offset from the direct line between hubs
fn place_parallel_routes(
    graph: &mut RailwayGraph,
    parallel_routes: &[ParallelRouteGroup],
    visited: &mut HashSet<NodeIndex>,
    base_station_spacing: f64,
    pinned_nodes: &HashSet<NodeIndex>,
) {
    const OFFSET_FACTOR: f64 = 0.5; // Perpendicular offset as fraction of base spacing

    for group in parallel_routes {
        // Get hub positions
        let Some(pos_a) = graph.get_station_position(group.hub_a) else { continue };
        let Some(pos_b) = graph.get_station_position(group.hub_b) else { continue };

        // Skip if hubs not yet positioned
        if pos_a == (0.0, 0.0) || pos_b == (0.0, 0.0) {
            continue;
        }

        // Calculate direction vector from hub_a to hub_b
        let dx = pos_b.0 - pos_a.0;
        let dy = pos_b.1 - pos_a.1;
        let dist = (dx * dx + dy * dy).sqrt();
        if dist < 1.0 {
            continue; // Hubs too close
        }

        // Perpendicular offset vector (rotate 90° clockwise for offset "below")
        let perp_x = dy / dist;
        let perp_y = -dx / dist;

        let offset = base_station_spacing * OFFSET_FACTOR;
        let intermediate_count = group.local_intermediates.len();

        // Place each intermediate station
        for (i, &node) in group.local_intermediates.iter().enumerate() {
            // Skip if already visited or pinned
            if visited.contains(&node) || pinned_nodes.contains(&node) {
                visited.insert(node);
                continue;
            }

            // Interpolate position along hub_a -> hub_b line
            // t goes from 1/(n+1) to n/(n+1) for n intermediates
            #[allow(clippy::cast_precision_loss)]
            let t = (i + 1) as f64 / (intermediate_count + 1) as f64;
            let base_x = pos_a.0 + dx * t;
            let base_y = pos_a.1 + dy * t;

            // Apply perpendicular offset
            let preferred_x = base_x + perp_x * offset;
            let preferred_y = base_y + perp_y * offset;

            // Find non-colliding position
            let final_pos = find_non_colliding_position(
                graph,
                (preferred_x, preferred_y),
                node,
                base_station_spacing,
            );

            graph.set_station_position(node, final_pos);
            visited.insert(node);
        }
    }
}

/// Analyze branches from the hub to determine optimal spine direction.
/// Returns the angle for the spine direction.
///
/// Strategy: Find which axis has the heaviest OFF-spine branches.
/// Place spine perpendicular to that axis so branches have room to spread.
#[allow(clippy::cast_precision_loss)]
fn determine_spine_direction(
    graph: &RailwayGraph,
    hub: NodeIndex,
    spine_nodes: &HashSet<NodeIndex>,
    geo_hints: Option<&GeographicHints>,
    edge_weights: Option<&HashMap<EdgeIndex, usize>>,
) -> f64 {
    // Weight for each of the 8 compass directions
    let mut dir_weights: [usize; 8] = [0; 8];
    // DIRECTIONS: E(0), SE(1), S(2), SW(3), W(4), NW(5), N(6), NE(7)

    // For each neighbor of the hub that's NOT on the spine
    for neighbor in graph.graph.neighbors_undirected(hub) {
        if spine_nodes.contains(&neighbor) {
            continue;
        }

        // Get geographic direction to this neighbor
        let Some(geo_dir) = geo_hints.and_then(|h| h.preferred_direction(hub, neighbor)) else {
            continue;
        };

        // Snap to nearest compass direction
        let compass_dir = snap_to_compass(geo_dir);
        let dir_idx = DIRECTIONS.iter().position(|&d| (d - compass_dir).abs() < 0.01).unwrap_or(0);

        // Count reachable nodes from this neighbor (excluding hub)
        let reachable = get_reachable_nodes(graph, neighbor, Some(hub));
        let mut weight = reachable.len();

        // Add edge weights if available
        if let Some(weights) = edge_weights {
            for node in &reachable {
                for edge in graph.graph.edges(*node) {
                    weight += weights.get(&edge.id()).copied().unwrap_or(0);
                }
            }
        }

        dir_weights[dir_idx] += weight;
    }

    // Calculate weight for each axis (opposite directions combined)
    // Axis 0: E-W (indices 0,4)
    // Axis 1: SE-NW (indices 1,5)
    // Axis 2: S-N (indices 2,6)
    // Axis 3: SW-NE (indices 3,7)
    let axis_weights = [
        dir_weights[0] + dir_weights[4], // E-W
        dir_weights[1] + dir_weights[5], // SE-NW
        dir_weights[2] + dir_weights[6], // S-N
        dir_weights[3] + dir_weights[7], // SW-NE
    ];

    // Find the heaviest axis - this is where branches want to go
    let heaviest_axis = axis_weights.iter().enumerate()
        .max_by_key(|(_, &w)| w)
        .map_or(0, |(i, _)| i);

    // Spine should be PERPENDICULAR to heaviest branch axis
    // Axis 0 (E-W) -> spine N-S (direction -90° or 90°)
    // Axis 1 (SE-NW) -> spine SW-NE (direction 135° or -45°)
    // Axis 2 (S-N) -> spine E-W (direction 0° or 180°)
    // Axis 3 (SW-NE) -> spine SE-NW (direction 45° or -135°)
    match heaviest_axis {
        1 => -std::f64::consts::FRAC_PI_4, // SW-NE spine
        2 => 0.0,                           // E-W spine (horizontal)
        3 => std::f64::consts::FRAC_PI_4,  // SE-NW spine
        _ => -std::f64::consts::FRAC_PI_2, // N-S spine (vertical) - default for axis 0 and any other
    }
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
    apply_layout_internal(graph, height, settings, geo_hints, pinned_nodes, None);
}

/// Apply layout with edge weights for spine detection (NIMBY import)
#[allow(clippy::too_many_lines, clippy::missing_panics_doc, clippy::cast_precision_loss)]
pub fn apply_layout_with_edge_weights(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &std::collections::HashSet<NodeIndex>,
    edge_weights: &HashMap<EdgeIndex, usize>,
) {
    apply_layout_internal(graph, height, settings, geo_hints, pinned_nodes, Some(edge_weights));
}

#[allow(clippy::too_many_lines, clippy::missing_panics_doc, clippy::cast_precision_loss)]
pub fn apply_layout(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
) {
    apply_layout_internal(graph, height, settings, geo_hints, &std::collections::HashSet::new(), None);
}

#[allow(clippy::too_many_lines, clippy::missing_panics_doc, clippy::cast_precision_loss)]
fn apply_layout_internal(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
    pinned_nodes: &std::collections::HashSet<NodeIndex>,
    edge_weights: Option<&HashMap<EdgeIndex, usize>>,
) {
    let base_station_spacing = settings.default_node_distance_grid_squares * GRID_SIZE;
    let start_x = 150.0;
    let start_y = height / 2.0;

    if graph.graph.node_count() == 0 {
        return; // Empty graph
    }

    // Detect parallel routes (express vs local) for offset placement
    let parallel_routes = detect_parallel_routes(graph, geo_hints);

    // Detect same-name station clusters for proximity placement
    let station_clusters = detect_station_clusters(graph, geo_hints);
    let cluster_lookup = build_cluster_lookup(&station_clusters);

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

    // Phase 1: Find main spine
    // Use edge weights (line usage) if available, otherwise longest path
    let spine = match edge_weights {
        Some(weights) if !weights.is_empty() => graph.find_heaviest_path(weights),
        _ => graph.find_longest_path(),
    };

    if spine.is_empty() {
        return;
    }

    // Find the hub node on the spine (highest degree = most connections)
    // This node will be placed at the center, with the spine extending in both directions
    let hub_index = spine
        .iter()
        .enumerate()
        .max_by_key(|(_, &node)| graph.graph.neighbors_undirected(node).count())
        .map_or(spine.len() / 2, |(idx, _)| idx);

    let hub_node = spine[hub_index];
    let spine_set: HashSet<NodeIndex> = spine.iter().copied().collect();

    // Determine optimal spine direction based on branch analysis
    let base_spine_direction = determine_spine_direction(graph, hub_node, &spine_set, geo_hints, edge_weights);

    // Orient the spine correctly based on geography of endpoints
    // "Before hub" end should go in the direction that matches its geographic position
    let spine_direction = if let Some(hints) = geo_hints {
        let first_node = spine.first().copied();
        let last_node = spine.last().copied();

        // Get geographic directions from hub to both ends
        let dir_to_first = first_node.and_then(|n| hints.preferred_direction(hub_node, n));
        let dir_to_last = last_node.and_then(|n| hints.preferred_direction(hub_node, n));

        // Check which end is more aligned with base_spine_direction
        // "Before hub" nodes are placed going in spine_reverse, so first_node should be in spine_reverse direction
        let should_swap = match (dir_to_first, dir_to_last) {
            (Some(first_dir), Some(last_dir)) => {
                // If first_node is more aligned with spine_direction (not reverse), swap
                let first_alignment = angle_difference(first_dir, base_spine_direction);
                let last_alignment = angle_difference(last_dir, base_spine_direction);
                first_alignment < last_alignment
            }
            _ => false,
        };

        if should_swap {
            base_spine_direction + std::f64::consts::PI // Flip the direction
        } else {
            base_spine_direction
        }
    } else {
        base_spine_direction
    };
    let spine_reverse = spine_direction + std::f64::consts::PI;

    // Phase 2: Place spine from hub outward in both directions
    let mut visited = HashSet::new();
    let center_pos = snap_to_grid(start_x, start_y);

    // Place the hub node at the center first
    if !pinned_nodes.contains(&hub_node) {
        graph.set_station_position(hub_node, center_pos);
    }
    visited.insert(hub_node);

    // Place nodes BEFORE the hub - go in spine_reverse direction (away from hub)
    let mut current_pos = center_pos;
    for i in (0..hub_index).rev() {
        let node = spine[i];

        let is_passing_loop = graph.graph.node_weight(node)
            .and_then(|n| n.as_station())
            .is_some_and(|s| s.passing_loop);

        if is_passing_loop {
            visited.insert(node);
            continue;
        }

        if pinned_nodes.contains(&node) {
            if let Some(pos) = graph.graph.node_weight(node)
                .and_then(|n| n.as_station())
                .and_then(|s| s.position)
            {
                current_pos = pos;
            }
            visited.insert(node);
            continue;
        }

        let preferred_pos = snap_to_grid(
            current_pos.0 + spine_reverse.cos() * base_station_spacing,
            current_pos.1 + spine_reverse.sin() * base_station_spacing,
        );
        current_pos = find_non_colliding_position(graph, preferred_pos, node, base_station_spacing);
        graph.set_station_position(node, current_pos);
        visited.insert(node);
    }

    // Place nodes AFTER the hub - go in spine_direction (away from hub)
    current_pos = center_pos;
    for &node in spine.iter().skip(hub_index + 1) {
        let is_passing_loop = graph.graph.node_weight(node)
            .and_then(|n| n.as_station())
            .is_some_and(|s| s.passing_loop);

        if is_passing_loop {
            visited.insert(node);
            continue;
        }

        if pinned_nodes.contains(&node) {
            if let Some(pos) = graph.graph.node_weight(node)
                .and_then(|n| n.as_station())
                .and_then(|s| s.position)
            {
                current_pos = pos;
            }
            visited.insert(node);
            continue;
        }

        let preferred_pos = snap_to_grid(
            current_pos.0 + spine_direction.cos() * base_station_spacing,
            current_pos.1 + spine_direction.sin() * base_station_spacing,
        );
        current_pos = find_non_colliding_position(graph, preferred_pos, node, base_station_spacing);
        graph.set_station_position(node, current_pos);
        visited.insert(node);
    }

    // Place cluster siblings for spine nodes
    for &node in &spine {
        if let Some(&cluster_idx) = cluster_lookup.get(&node) {
            place_cluster_secondaries(
                graph,
                &station_clusters[cluster_idx],
                node,
                &mut visited,
                base_station_spacing,
                pinned_nodes,
            );
        }
    }

    // Phase 2.5: Place parallel route intermediates (local line stations offset from express)
    place_parallel_routes(graph, &parallel_routes, &mut visited, base_station_spacing, pinned_nodes);

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
                // Use spine direction: before hub came from spine_direction, after hub came from spine_reverse
                let dir = if i <= hub_index { spine_direction } else { spine_reverse };
                (edge, dir)
            } else {
                (None, spine_direction)
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
            let debug_stations = ["Roa", "Heggedal", "Ski", "Lillestrøm", "Drammen", "Gjøvik"];
            let neighbor_name = graph.graph[neighbor].display_name();
            if debug_stations.contains(&neighbor_name.as_str()) {
                let geo_dir = geo_hints.and_then(|h| h.preferred_direction(current_node, neighbor));
                leptos::logging::log!("PLACING {} from {} at ({:.1}, {:.1})",
                    neighbor_name,
                    graph.graph[current_node].display_name(),
                    current_pos.0, current_pos.1);
                leptos::logging::log!("  Geographic direction: {:?}°", geo_dir.map(f64::to_degrees));
                leptos::logging::log!("  Best direction: {:.0}°, spacing: {:.1}, score: {}",
                    best_direction.to_degrees(), best_spacing, best_score);
                leptos::logging::log!("  Incoming direction: {:.0}°", incoming_direction.to_degrees());
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

                // If this node is in a cluster, place its siblings nearby
                if let Some(&cluster_idx) = cluster_lookup.get(&neighbor) {
                    place_cluster_secondaries(
                        graph,
                        &station_clusters[cluster_idx],
                        neighbor,
                        &mut visited,
                        base_station_spacing,
                        pinned_nodes,
                    );
                }
            } else {
                // Absolutely no valid position found - this should be extremely rare
                // Use a varied emergency direction with collision checking
                let emergency_dir = DIRECTIONS[fallback_direction_index % DIRECTIONS.len()];
                fallback_direction_index += 1;
                let preferred_emergency = (
                    current_pos.0 + emergency_dir.cos() * base_station_spacing * 5.0,
                    current_pos.1 + emergency_dir.sin() * base_station_spacing * 5.0,
                );
                let emergency_pos = find_non_colliding_position(
                    graph,
                    preferred_emergency,
                    neighbor,
                    base_station_spacing,
                );
                graph.set_station_position(neighbor, emergency_pos);
                visited.insert(neighbor);
                local_branches.push((emergency_dir, reachable.clone()));
                global_branches.push((emergency_dir, reachable.clone()));
                queue.push_back((neighbor, emergency_pos, emergency_dir, edge_to_neighbor));

                // If this node is in a cluster, place its siblings nearby
                if let Some(&cluster_idx) = cluster_lookup.get(&neighbor) {
                    place_cluster_secondaries(
                        graph,
                        &station_clusters[cluster_idx],
                        neighbor,
                        &mut visited,
                        base_station_spacing,
                        pinned_nodes,
                    );
                }
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
                    // Place disconnected components going downward (positive Y)
                    let preferred_pos = snap_to_grid(
                        offset_x,
                        start_y + offset,
                    );

                    // Find non-colliding position for disconnected component nodes
                    let pos = find_non_colliding_position(graph, preferred_pos, comp_node, base_station_spacing);
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
