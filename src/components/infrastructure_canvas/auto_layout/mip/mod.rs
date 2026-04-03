/// MIP-based metro map layout following Nöllenburg & Wolff (2011).
///
/// Formulates the metro map layout problem as a mixed-integer program:
/// - Hard constraints enforce octilinear edges, topology preservation, minimum edge length
/// - Soft constraints minimize bends, preserve geography, minimize total edge length
/// - Degree-2 vertices are collapsed before solving and reinserted after
pub mod collapse;
pub mod geometry;
pub mod model;

use petgraph::stable_graph::NodeIndex;
use std::collections::HashMap;

use super::constants::GRID_SIZE;
use super::geographic_hints::GeographicHints;
use crate::models::RailwayGraph;

use collapse::{collapse_degree2, reinsert_collapsed};
use model::{solve_mip, MipWeights};

/// Build per-edge minimum lengths: edges with many collapsed stations need more space
fn build_edge_min_lengths(
    collapsed: &collapse::CollapsedGraph,
    base_min: f64,
) -> HashMap<(NodeIndex, NodeIndex), f64> {
    let mut lengths = HashMap::new();

    // Spacing per intermediate station (in MIP grid units)
    let spacing_per_station = 2.0;

    for chain in &collapsed.chains {
        if chain.endpoints.0 == chain.endpoints.1 {
            continue;
        }
        let pair = normalize_pair(chain.endpoints.0, chain.endpoints.1);
        #[allow(clippy::cast_precision_loss)]
        let chain_min = (chain.intermediates.len() as f64 + 1.0) * spacing_per_station;
        let min_len = chain_min.max(base_min);

        // Keep the largest minimum if multiple chains share an edge
        let entry = lengths.entry(pair).or_insert(base_min);
        if min_len > *entry {
            *entry = min_len;
        }
    }

    lengths
}

/// Detect chains whose interpolation path overlaps a MIP edge geometrically.
/// A chain is parallel if its two endpoints lie on the same line (horizontal,
/// vertical, or diagonal) as another MIP edge that the chain is NOT part of.
fn detect_geometric_parallels(
    collapsed: &collapse::CollapsedGraph,
    positions: &HashMap<NodeIndex, (f64, f64)>,
) -> std::collections::HashSet<(NodeIndex, NodeIndex)> {
    type Segment = ((f64, f64), (f64, f64), NodeIndex, NodeIndex);

    let mut parallel = std::collections::HashSet::new();

    // Collect MIP edge segments as (pos_a, pos_b) with their node pairs
    let mip_segments: Vec<Segment> = collapsed
        .kept_edges
        .iter()
        .filter(|&&(a, b)| a != b)
        .filter_map(|&(a, b)| {
            let pa = positions.get(&a)?;
            let pb = positions.get(&b)?;
            Some((*pa, *pb, a, b))
        })
        .collect();

    for chain in &collapsed.chains {
        if chain.endpoints.0 == chain.endpoints.1 || chain.intermediates.is_empty() {
            continue;
        }
        let Some(&ca) = positions.get(&chain.endpoints.0) else { continue };
        let Some(&cb) = positions.get(&chain.endpoints.1) else { continue };

        // Check if any MIP edge overlaps this chain's interpolation line
        for &(sa, sb, na, nb) in &mip_segments {
            // Skip if this edge IS the chain's own edge
            if normalize_pair(na, nb) == normalize_pair(chain.endpoints.0, chain.endpoints.1) {
                continue;
            }

            // Check if the chain segment and MIP edge are collinear
            // (same horizontal line, vertical line, or diagonal)
            if segments_collinear((ca, cb), (sa, sb)) {
                parallel.insert(normalize_pair(chain.endpoints.0, chain.endpoints.1));
                break;
            }
        }
    }

    parallel
}

/// Check if two line segments are collinear (on the same horizontal, vertical, or 45° line)
/// and overlapping in extent.
fn segments_collinear(
    seg_a: ((f64, f64), (f64, f64)),
    seg_b: ((f64, f64), (f64, f64)),
) -> bool {
    let tolerance = 5.0;
    let (pa1, pa2) = seg_a;
    let (pb1, pb2) = seg_b;

    // Same horizontal line
    if (pa1.1 - pa2.1).abs() < tolerance
        && (pb1.1 - pb2.1).abs() < tolerance
        && (pa1.1 - pb1.1).abs() < tolerance
    {
        return ranges_overlap(pa1.0, pa2.0, pb1.0, pb2.0);
    }

    // Same vertical line
    if (pa1.0 - pa2.0).abs() < tolerance
        && (pb1.0 - pb2.0).abs() < tolerance
        && (pa1.0 - pb1.0).abs() < tolerance
    {
        return ranges_overlap(pa1.1, pa2.1, pb1.1, pb2.1);
    }

    // Same 45° diagonal
    let (dxa, dya) = (pa2.0 - pa1.0, pa2.1 - pa1.1);
    let (dxb, dyb) = (pb2.0 - pb1.0, pb2.1 - pb1.1);
    if (dxa.abs() - dya.abs()).abs() < tolerance && (dxb.abs() - dyb.abs()).abs() < tolerance {
        let sum_a = pa1.0 + pa1.1;
        let sum_b = pb1.0 + pb1.1;
        let diff_a = pa1.0 - pa1.1;
        let diff_b = pb1.0 - pb1.1;
        if (sum_a - sum_b).abs() < tolerance || (diff_a - diff_b).abs() < tolerance {
            return true;
        }
    }

    false
}

fn ranges_overlap(a1: f64, a2: f64, b1: f64, b2: f64) -> bool {
    let (lo_a, hi_a) = if a1 < a2 { (a1, a2) } else { (a2, a1) };
    let (lo_b, hi_b) = if b1 < b2 { (b1, b2) } else { (b2, b1) };
    lo_a < hi_b && lo_b < hi_a
}

fn normalize_pair(a: NodeIndex, b: NodeIndex) -> (NodeIndex, NodeIndex) {
    if a < b { (a, b) } else { (b, a) }
}

/// Find pairs of non-incident edges from the same hub that go in SIMILAR
/// geographic directions (within 90°). Only these can realistically cross.
fn compute_hub_spacing_pairs(
    edges: &[(NodeIndex, NodeIndex, u8)],
) -> Vec<model::EdgeSpacingPair> {
    use crate::geometry::angle_difference;
    use super::constants::COMPASS_DIRECTIONS;

    let mut pairs = Vec::new();

    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            let (u1, v1, sec1) = edges[i];
            let (u2, v2, sec2) = edges[j];

            // Skip incident edges
            if u1 == u2 || u1 == v2 || v1 == u2 || v1 == v2 {
                continue;
            }

            // Check if they share a hub
            let shares_hub = edges.iter().any(|&(a, b, _)| {
                ((a == u1 || a == v1) && (b == u2 || b == v2))
                    || ((b == u1 || b == v1) && (a == u2 || a == v2))
            });

            if !shares_hub {
                continue;
            }

            // Only add spacing if geographic directions are DIFFERENT but close.
            // Same direction = collinear (one is on the way to the other, not a branch).
            // Opposite directions = can't cross.
            // Only close-but-different directions can cross and need spacing.
            if sec1 == sec2 {
                continue; // Collinear — no spacing needed
            }
            let angle1 = COMPASS_DIRECTIONS[sec1 as usize % 8];
            let angle2 = COMPASS_DIRECTIONS[sec2 as usize % 8];
            let diff = angle_difference(angle1, angle2);
            if diff <= std::f64::consts::FRAC_PI_2 {
                pairs.push((i, j));
            }
        }
    }

    pairs
}

/// Merge positions of same-name stations that are geographically close.
/// These represent transfer complexes (e.g., "Bryn" metro and "Bryn" commuter rail).
fn merge_station_clusters(
    graph: &RailwayGraph,
    geo_positions: &HashMap<NodeIndex, (f64, f64)>,
    positions: &mut HashMap<NodeIndex, (f64, f64)>,
) {
    const MAX_GEO_DISTANCE: f64 = 0.015; // ~1.5km

    // Group nodes by lowercase name
    let mut name_groups: HashMap<String, Vec<NodeIndex>> = HashMap::new();
    for node in graph.graph.node_indices() {
        let name = graph.graph[node].display_name().to_lowercase();
        name_groups.entry(name).or_default().push(node);
    }

    let mut merged = 0;
    for nodes in name_groups.values() {
        if nodes.len() < 2 {
            continue;
        }

        // Check geographic proximity and merge to first positioned node's location
        let representative = nodes.iter().find(|&&n| positions.contains_key(&n)).copied();
        let Some(rep) = representative else { continue };
        let rep_geo = geo_positions.get(&rep).copied();

        for &node in nodes {
            if node == rep {
                continue;
            }

            // Check geographic proximity
            let close = match (rep_geo, geo_positions.get(&node)) {
                (Some((lon1, lat1)), Some(&(lon2, lat2))) => {
                    ((lon1 - lon2).powi(2) + (lat1 - lat2).powi(2)).sqrt() <= MAX_GEO_DISTANCE
                }
                _ => false,
            };

            if close {
                if let Some(&rep_pos) = positions.get(&rep) {
                    positions.insert(node, rep_pos);
                    merged += 1;
                }
            }
        }
    }

    if merged > 0 {
        leptos::logging::log!("MIP layout: merged {merged} cluster stations");
    }
}

/// Assign geographic positions to nodes that don't have them by averaging neighbors
fn interpolate_geo_positions(
    graph: &RailwayGraph,
    positions: &mut HashMap<NodeIndex, (f64, f64)>,
) {
    let nodes: Vec<NodeIndex> = graph.graph.node_indices().collect();
    let max_passes = nodes.len();

    for _ in 0..max_passes {
        let mut progress = false;

        for &node in &nodes {
            if positions.contains_key(&node) {
                continue;
            }

            // Average positions of neighbors that have coordinates
            let neighbors: Vec<(f64, f64)> = graph
                .graph
                .neighbors_undirected(node)
                .filter_map(|n| positions.get(&n).copied())
                .collect();

            if neighbors.is_empty() {
                continue;
            }

            #[allow(clippy::cast_precision_loss)]
            let avg = (
                neighbors.iter().map(|p| p.0).sum::<f64>() / neighbors.len() as f64,
                neighbors.iter().map(|p| p.1).sum::<f64>() / neighbors.len() as f64,
            );
            positions.insert(node, avg);
            progress = true;
        }

        if !progress {
            break;
        }
    }
}

/// Run MIP layout on a graph with geographic hints.
/// Returns positions for all nodes, or None if the solver fails.
pub fn run_mip_layout(
    graph: &RailwayGraph,
    geo_hints: &GeographicHints,
    min_edge_length: f64,
) -> Option<HashMap<NodeIndex, (f64, f64)>> {
    // Build geographic position map from hints
    let mut geo_positions: HashMap<NodeIndex, (f64, f64)> = graph
        .graph
        .node_indices()
        .filter_map(|node| geo_hints.get_coords(node).map(|coords| (node, coords)))
        .collect();

    // Interpolate geo positions for nodes that don't have them (junctions, etc.)
    interpolate_geo_positions(graph, &mut geo_positions);

    leptos::logging::log!(
        "MIP layout: {} of {} nodes have geographic positions (after interpolation)",
        geo_positions.len(),
        graph.graph.node_count()
    );

    if geo_positions.len() < 2 {
        return None;
    }

    // Collapse degree-2 vertices
    let collapsed = collapse_degree2(graph, &geo_positions);

    leptos::logging::log!(
        "MIP layout: {} nodes collapsed to {}, {} edges",
        graph.graph.node_count(),
        collapsed.kept_nodes.len(),
        collapsed.kept_edges.len()
    );


    if collapsed.kept_nodes.len() < 2 {
        return None;
    }

    // Build per-edge minimum lengths based on chain sizes
    // Each intermediate station needs at least 2 grid units of space
    let edge_min_lengths = build_edge_min_lengths(&collapsed, min_edge_length);

    // Build edge list and pre-compute (H4) spacing pairs
    let weights = MipWeights::default();
    let edge_list = model::build_edge_list(&collapsed);
    let d_min = 1.0;

    // (H4) spacing for hub branches going in similar geographic directions
    let spacing_pairs = compute_hub_spacing_pairs(&edge_list);
    leptos::logging::log!("MIP layout: {} hub spacing pairs", spacing_pairs.len());

    let solution = solve_mip(
        &collapsed, &edge_min_lengths, min_edge_length, &weights,
        &spacing_pairs, d_min,
    );

    if !solution.success {
        leptos::logging::log!("MIP layout: solver failed");
        return None;
    }

    // Scale positions from MIP units to grid units
    let mut positions: HashMap<NodeIndex, (f64, f64)> = solution
        .positions
        .into_iter()
        .map(|(node, (x, y))| (node, (x * GRID_SIZE, y * GRID_SIZE)))
        .collect();

    // Detect parallel chains geometrically: a chain should be offset if its
    // interpolation path overlaps with a MIP edge (same horizontal/vertical line).
    let parallel_pairs = detect_geometric_parallels(&collapsed, &positions);

    for chain in &collapsed.chains {
        if chain.endpoints.0 == chain.endpoints.1 { continue; }
        let pair = normalize_pair(chain.endpoints.0, chain.endpoints.1);
        if parallel_pairs.contains(&pair) {
            let a = graph.graph[pair.0].display_name();
            let b = graph.graph[pair.1].display_name();
            leptos::logging::log!("  Parallel: {a} <-> {b} ({} intermediates)", chain.intermediates.len());
        }
    }
    // Log which chains share endpoints with MIP edges
    for chain in &collapsed.chains {
        if chain.endpoints.0 == chain.endpoints.1 { continue; }
        let (a, b) = chain.endpoints;
        let a_name = graph.graph[a].display_name();
        let b_name = graph.graph[b].display_name();
        let has_direct = graph.graph.edges_connecting(a, b).next().is_some()
            || graph.graph.edges_connecting(b, a).next().is_some();
        if a_name.contains("Bryn") || b_name.contains("Bryn") || a_name.contains("Nyland") || b_name.contains("Nyland") {
            leptos::logging::log!(
                "  Chain {a_name}->{b_name}: {} intermediates, direct_edge={has_direct}",
                chain.intermediates.len()
            );
        }
    }
    leptos::logging::log!("MIP layout: {} parallel local/express pairs", parallel_pairs.len());

    // Reinsert collapsed degree-2 vertices (offset parallel chains)
    let reinserted = reinsert_collapsed(
        &collapsed.chains, &positions, &parallel_pairs, &geo_positions, GRID_SIZE,
    );
    for (node, pos) in reinserted {
        positions.insert(node, pos);
    }

    // Merge same-name station clusters: stations with the same name and close
    // geographic positions should share the same layout position
    merge_station_clusters(graph, &geo_positions, &mut positions);

    // Fallback: place any remaining unpositioned nodes at their nearest positioned neighbor
    for node in graph.graph.node_indices() {
        if positions.contains_key(&node) {
            continue;
        }
        let neighbor_pos = graph
            .graph
            .neighbors_undirected(node)
            .find_map(|n| positions.get(&n).copied());
        if let Some(pos) = neighbor_pos {
            positions.insert(node, pos);
        }
    }

    // Re-run cluster merge to catch nodes positioned by fallback
    merge_station_clusters(graph, &geo_positions, &mut positions);

    // Log position bounds
    if let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (
        positions.values().map(|p| p.0).reduce(f64::min),
        positions.values().map(|p| p.0).reduce(f64::max),
        positions.values().map(|p| p.1).reduce(f64::min),
        positions.values().map(|p| p.1).reduce(f64::max),
    ) {
        leptos::logging::log!(
            "MIP layout: positioned {} nodes, bounds x=[{:.1}, {:.1}] y=[{:.1}, {:.1}]",
            positions.len(), min_x, max_x, min_y, max_y
        );
    }

    Some(positions)
}
