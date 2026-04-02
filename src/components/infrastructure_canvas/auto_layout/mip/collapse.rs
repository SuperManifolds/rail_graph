/// Degree-2 vertex collapsing for MIP model size reduction.
///
/// Before solving, chains of degree-2 vertices (non-interchange stations between
/// two interchanges or terminals) are collapsed into single weighted edges.
/// After solving, the collapsed vertices are reinserted evenly along the edge.
use petgraph::stable_graph::NodeIndex;
use std::collections::{HashMap, HashSet};

use crate::models::RailwayGraph;

/// A collapsed chain of degree-2 vertices between two endpoints
#[derive(Debug)]
pub struct CollapsedChain {
    /// The two endpoint nodes (interchanges or terminals) that remain in the reduced graph
    pub endpoints: (NodeIndex, NodeIndex),
    /// The intermediate degree-2 nodes in order from `endpoints.0` to `endpoints.1`
    pub intermediates: Vec<NodeIndex>,
}

/// Result of collapsing a graph
pub struct CollapsedGraph {
    /// Nodes that remain in the reduced graph (interchanges + terminals)
    pub kept_nodes: Vec<NodeIndex>,
    /// Edges in the reduced graph as (from, to) pairs
    pub kept_edges: Vec<(NodeIndex, NodeIndex)>,
    /// The collapsed chains for later reinsertion
    pub chains: Vec<CollapsedChain>,
    /// Geographic coordinates for kept nodes (from `geo_hints`)
    pub positions: HashMap<NodeIndex, (f64, f64)>,
}

/// Collapse all chains of degree-2 vertices in the graph.
///
/// A node is "collapsible" if:
/// - It has exactly degree 2 (two neighbors in undirected graph)
/// - It is not a passing loop (those are handled separately)
pub fn collapse_degree2(
    graph: &RailwayGraph,
    geo_positions: &HashMap<NodeIndex, (f64, f64)>,
) -> CollapsedGraph {
    // Identify station clusters (same name, close geography) before collapsing.
    // Stations that are individually degree-2 but collectively degree-3+
    // represent interchanges and should be kept as MIP nodes.
    let cluster_keep = find_cluster_interchanges(graph, geo_positions);

    // Identify which nodes to keep in the reduced graph.
    // Collapse: degree-2 stations AND passing loops (positioned automatically later)
    let mut keep: HashSet<NodeIndex> = graph
        .graph
        .node_indices()
        .filter(|&node| {
            let degree = graph.graph.neighbors_undirected(node).count();
            let is_passing_loop = graph
                .graph
                .node_weight(node)
                .and_then(|n| n.as_station())
                .is_some_and(|s| s.passing_loop);
            // Keep if: high degree, cluster interchange, or not a passing loop
            (degree != 2 && !is_passing_loop) || cluster_keep.contains(&node)
        })
        .collect();

    // Merge same-name station clusters: stations with the same name and close
    // geographic positions represent transfer complexes and should share a position.
    // We pick one representative per cluster; others map to it.
    let cluster_map = build_cluster_map(graph, &keep, geo_positions);
    leptos::logging::log!("  Collapse: {} cluster merges", cluster_map.len());

    // Trace chains between kept nodes
    let mut chains = Vec::new();
    let mut visited_intermediates: HashSet<NodeIndex> = HashSet::new();
    // Nodes promoted to kept status (turnaround points of palindromic chains)
    let mut promoted: HashSet<NodeIndex> = HashSet::new();

    for &start in &keep {
        for neighbor in graph.graph.neighbors_undirected(start) {
            if keep.contains(&neighbor)
                || promoted.contains(&neighbor)
                || visited_intermediates.contains(&neighbor)
            {
                continue;
            }

            let mut chain_nodes = vec![neighbor];
            visited_intermediates.insert(neighbor);
            let mut chain_set: HashSet<NodeIndex> = HashSet::new();
            chain_set.insert(neighbor);
            let mut current = neighbor;
            let mut prev = start;

            loop {
                let next = graph
                    .graph
                    .neighbors_undirected(current)
                    .find(|&n| n != prev);

                let Some(next) = next else { break };

                if keep.contains(&next) || promoted.contains(&next) {
                    // Reached a kept/promoted endpoint
                    // Map both endpoints through cluster representatives
                    let mapped_start = *cluster_map.get(&start).unwrap_or(&start);
                    let mapped_next = *cluster_map.get(&next).unwrap_or(&next);
                    chains.push(CollapsedChain {
                        endpoints: (mapped_start, mapped_next),
                        intermediates: chain_nodes,
                    });
                    break;
                }

                // Palindrome detection: if we've seen this node in the current chain,
                // we're tracing back on a return trip. Promote the previous node as
                // the turnaround (branch terminus) and end the chain there.
                if chain_set.contains(&next) {
                    // `current` is the turnaround point — promote it to a kept node
                    promoted.insert(current);
                    chain_nodes.pop(); // Remove turnaround from intermediates
                    push_chain_if_nonempty(
                        &mut chains, chain_nodes, start, current, &cluster_map,
                    );
                    break;
                }

                visited_intermediates.insert(next);
                chain_set.insert(next);
                chain_nodes.push(next);
                prev = current;
                current = next;
            }
        }
    }

    // Add promoted nodes to the kept set
    keep.extend(&promoted);

    // Build the reduced edge list
    let mut kept_edges = Vec::new();
    let mut edge_pairs_seen: HashSet<(NodeIndex, NodeIndex)> = HashSet::new();

    // Add collapsed chain edges (skip self-loops from collapsed passing loops)
    for chain in &chains {
        if chain.endpoints.0 == chain.endpoints.1 {
            continue; // Self-loop
        }
        let pair = normalize_pair(chain.endpoints.0, chain.endpoints.1);
        if edge_pairs_seen.insert(pair) {
            kept_edges.push(chain.endpoints);
        }
    }

    // Add direct edges between kept nodes (mapped through clusters)
    for &node in &keep {
        let mapped_node = *cluster_map.get(&node).unwrap_or(&node);
        for neighbor in graph.graph.neighbors_undirected(node) {
            if keep.contains(&neighbor) {
                let mapped_neighbor = *cluster_map.get(&neighbor).unwrap_or(&neighbor);
                if mapped_node == mapped_neighbor {
                    continue; // Same cluster
                }
                let pair = normalize_pair(mapped_node, mapped_neighbor);
                if edge_pairs_seen.insert(pair) {
                    kept_edges.push((mapped_node, mapped_neighbor));
                }
            }
        }
    }

    // Kept nodes = unique cluster representatives
    let kept_nodes: Vec<NodeIndex> = keep
        .iter()
        .map(|&n| *cluster_map.get(&n).unwrap_or(&n))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let positions: HashMap<NodeIndex, (f64, f64)> = kept_nodes
        .iter()
        .filter_map(|&node| geo_positions.get(&node).map(|&pos| (node, pos)))
        .collect();

    CollapsedGraph {
        kept_nodes,
        kept_edges,
        chains,
        positions,
    }
}

/// Reinsert collapsed degree-2 vertices evenly along their edges.
/// For normal chains: interpolate between the two endpoint positions.
/// For chains parallel to a direct edge: offset perpendicular.
/// For self-loop chains: extend outward from the interchange using geo hints.
pub fn reinsert_collapsed(
    chains: &[CollapsedChain],
    solved_positions: &HashMap<NodeIndex, (f64, f64)>,
    direct_edges: &HashSet<(NodeIndex, NodeIndex)>,
    geo_positions: &HashMap<NodeIndex, (f64, f64)>,
    grid_size: f64,
) -> HashMap<NodeIndex, (f64, f64)> {
    let mut positions = HashMap::new();

    for chain in chains {
        let Some(&pos_a) = solved_positions.get(&chain.endpoints.0) else {
            continue;
        };

        if chain.endpoints.0 == chain.endpoints.1 {
            reinsert_self_loop(chain, pos_a, geo_positions, grid_size, &mut positions);
            continue;
        }

        let Some(&pos_b) = solved_positions.get(&chain.endpoints.1) else {
            continue;
        };

        // Check if this chain runs parallel to a direct MIP edge
        let pair = normalize_pair(chain.endpoints.0, chain.endpoints.1);
        let is_parallel = direct_edges.contains(&pair);

        // Perpendicular offset for parallel chains (one grid square)
        let (offset_x, offset_y) = if is_parallel {
            let dx = pos_b.0 - pos_a.0;
            let dy = pos_b.1 - pos_a.1;
            let len = (dx * dx + dy * dy).sqrt().max(1.0);
            // Perpendicular direction (rotate 90° clockwise)
            (dy / len * grid_size, -dx / len * grid_size)
        } else {
            (0.0, 0.0)
        };

        let n = chain.intermediates.len();
        for (i, &node) in chain.intermediates.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let t = (i + 1) as f64 / (n + 1) as f64;
            let x = pos_a.0 + (pos_b.0 - pos_a.0) * t + offset_x;
            let y = pos_a.1 + (pos_b.1 - pos_a.1) * t + offset_y;
            positions.insert(node, (x, y));
        }
    }

    positions
}

/// Place self-loop chain intermediates extending outward from the interchange.
///
/// Self-loop chains trace out-and-back on bidirectional lines (e.g., A→B→C→C→B→A).
/// We only use the first half (outbound) and deduplicate by `NodeIndex`.
fn reinsert_self_loop(
    chain: &CollapsedChain,
    interchange_pos: (f64, f64),
    geo_positions: &HashMap<NodeIndex, (f64, f64)>,
    grid_size: f64,
    positions: &mut HashMap<NodeIndex, (f64, f64)>,
) {
    if chain.intermediates.is_empty() {
        return;
    }

    // Deduplicate: only keep first occurrence of each NodeIndex
    // (self-loop chains are palindromic: out-and-back)
    let mut seen = HashSet::new();
    let unique: Vec<NodeIndex> = chain
        .intermediates
        .iter()
        .copied()
        .filter(|node| seen.insert(*node))
        .collect();

    if unique.is_empty() {
        return;
    }

    // Determine branch direction from geographic data of the farthest unique station
    let farthest = unique.last().copied();
    let direction = farthest
        .and_then(|node| geo_positions.get(&node))
        .map_or((1.0, 0.0), |&(lon, lat)| {
            let interchange_geo = geo_positions
                .get(&chain.endpoints.0)
                .copied()
                .unwrap_or((lon, lat));
            let dx = lon - interchange_geo.0;
            let dy = interchange_geo.1 - lat; // Flip Y for screen
            let angle = dy.atan2(dx);
            let snapped = (angle / std::f64::consts::FRAC_PI_4).round()
                * std::f64::consts::FRAC_PI_4;
            (snapped.cos(), snapped.sin())
        });

    // Step size: 2 grid squares per station on each active axis
    // direction components are ±0.707 (diagonal) or ±1/0 (cardinal)
    let step = grid_size * 2.0; // 2 grid squares per station
    let step_x = direction.0.signum() * step;
    let step_y = direction.1.signum() * step;

    for (i, &node) in unique.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let mult = (i + 1) as f64;
        // Snap to grid
        let x = ((interchange_pos.0 + step_x * mult) / grid_size).round() * grid_size;
        let y = ((interchange_pos.1 + step_y * mult) / grid_size).round() * grid_size;
        positions.insert(node, (x, y));
    }
}

/// Push a collapsed chain if it has intermediate nodes, mapping endpoints through clusters.
fn push_chain_if_nonempty(
    chains: &mut Vec<CollapsedChain>,
    chain_nodes: Vec<NodeIndex>,
    start: NodeIndex,
    end: NodeIndex,
    cluster_map: &HashMap<NodeIndex, NodeIndex>,
) {
    if chain_nodes.is_empty() {
        return;
    }
    let mapped_start = *cluster_map.get(&start).unwrap_or(&start);
    let mapped_end = *cluster_map.get(&end).unwrap_or(&end);
    chains.push(CollapsedChain {
        endpoints: (mapped_start, mapped_end),
        intermediates: chain_nodes,
    });
}

fn normalize_pair(a: NodeIndex, b: NodeIndex) -> (NodeIndex, NodeIndex) {
    if a < b { (a, b) } else { (b, a) }
}

/// Maximum geographic distance (in degrees) for stations to be considered the same complex
const CLUSTER_MAX_DISTANCE_DEG: f64 = 0.015; // ~1.5km

/// Find degree-2 nodes that form interchange clusters with other same-name nodes.
/// These should be kept as MIP nodes because the cluster acts as a junction.
fn find_cluster_interchanges(
    graph: &RailwayGraph,
    geo_positions: &HashMap<NodeIndex, (f64, f64)>,
) -> HashSet<NodeIndex> {
    let mut result = HashSet::new();

    // Group by lowercase name
    let mut name_groups: HashMap<String, Vec<NodeIndex>> = HashMap::new();
    for node in graph.graph.node_indices() {
        let name = graph.graph[node].display_name().to_lowercase();
        name_groups.entry(name).or_default().push(node);
    }

    for nodes in name_groups.values() {
        if nodes.len() < 2 {
            continue;
        }

        // Check geographic proximity
        let all_close = nodes.iter().all(|&a| {
            nodes.iter().all(|&b| {
                if a == b { return true; }
                let Some(&pa) = geo_positions.get(&a) else { return false };
                let Some(&pb) = geo_positions.get(&b) else { return false };
                ((pa.0 - pb.0).powi(2) + (pa.1 - pb.1).powi(2)).sqrt() <= CLUSTER_MAX_DISTANCE_DEG
            })
        });

        if !all_close {
            continue;
        }

        // Check combined degree: count unique neighbors across all cluster nodes
        let cluster_set: HashSet<NodeIndex> = nodes.iter().copied().collect();
        let combined_neighbors: HashSet<NodeIndex> = nodes
            .iter()
            .flat_map(|&n| graph.graph.neighbors_undirected(n))
            .filter(|n| !cluster_set.contains(n))
            .collect();

        // If combined degree >= 3, this cluster is an interchange
        if combined_neighbors.len() >= 3 {
            result.extend(nodes);
        }
    }

    result
}

/// Build a mapping from station nodes to their cluster representative.
/// Stations with the same name and close geographic positions are merged.
fn build_cluster_map(
    graph: &RailwayGraph,
    keep: &HashSet<NodeIndex>,
    geo_positions: &HashMap<NodeIndex, (f64, f64)>,
) -> HashMap<NodeIndex, NodeIndex> {
    let mut cluster_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();

    // Group kept nodes by lowercase name
    let mut name_groups: HashMap<String, Vec<NodeIndex>> = HashMap::new();
    for &node in keep {
        let name = graph.graph[node].display_name().to_lowercase();
        name_groups.entry(name).or_default().push(node);
    }

    // For each group with 2+ nodes, check geographic proximity
    for nodes in name_groups.values() {
        if nodes.len() < 2 {
            continue;
        }

        // Find nodes that are geographically close
        let representative = nodes[0]; // First node is the representative

        for &node in nodes.iter().skip(1) {
            let Some(&pos_rep) = geo_positions.get(&representative) else {
                continue;
            };
            let Some(&pos_node) = geo_positions.get(&node) else {
                continue;
            };

            let dist = ((pos_rep.0 - pos_node.0).powi(2)
                + (pos_rep.1 - pos_node.1).powi(2))
            .sqrt();

            if dist <= CLUSTER_MAX_DISTANCE_DEG {
                cluster_map.insert(node, representative);
            }
        }
    }

    cluster_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reinsert_evenly() {
        // 3 intermediates between (0,0) and (400,0)
        let chain = CollapsedChain {
            endpoints: (NodeIndex::new(0), NodeIndex::new(4)),
            intermediates: vec![NodeIndex::new(1), NodeIndex::new(2), NodeIndex::new(3)],
        };

        let mut solved = HashMap::new();
        solved.insert(NodeIndex::new(0), (0.0, 0.0));
        solved.insert(NodeIndex::new(4), (400.0, 0.0));

        let direct_edges = HashSet::new();
        let geo_positions = HashMap::new();
        let grid_size = 30.0;

        let result = reinsert_collapsed(&[chain], &solved, &direct_edges, &geo_positions, grid_size);
        assert_eq!(result.len(), 3);

        let p1 = result[&NodeIndex::new(1)];
        assert!((p1.0 - 100.0).abs() < 0.01);

        let p2 = result[&NodeIndex::new(2)];
        assert!((p2.0 - 200.0).abs() < 0.01);

        let p3 = result[&NodeIndex::new(3)];
        assert!((p3.0 - 300.0).abs() < 0.01);
    }
}
