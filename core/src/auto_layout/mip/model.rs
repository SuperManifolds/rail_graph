/// MIP model construction following Nollenburg & Wolff (2011).
///
/// Simplified formulation focusing on:
/// - Direction selection (each edge picks one of 3 octilinear directions)
/// - Coordinate constraints (enforce positions match selected direction)
/// - Soft constraints: minimize bends (S1), preserve geography (S2), minimize length (S3)
use good_lp::{
    variable, Constraint, Expression, ProblemVariables, Solution, SolverModel, Variable, highs,
};
use petgraph::stable_graph::NodeIndex;
use std::collections::HashMap;

use super::collapse::CollapsedGraph;
use super::geometry::{admissible_directions, direction_deltas, sector};

/// Big-M constant for constraint relaxation
const BIG_M: f64 = 10000.0;

/// Solved positions from the MIP
pub struct MipSolution {
    pub positions: HashMap<NodeIndex, (f64, f64)>,
    pub success: bool,
}

/// MIP objective weights
pub struct MipWeights {
    pub relative_position: f64,
    pub edge_length: f64,
}

impl Default for MipWeights {
    fn default() -> Self {
        Self {
            relative_position: 50.0,
            edge_length: 1.0,
        }
    }
}

/// A pair of edge indices that need spacing constraints (H4)
pub type EdgeSpacingPair = (usize, usize);

/// Build and solve the MIP model
pub fn solve_mip(
    collapsed: &CollapsedGraph,
    edge_min_lengths: &HashMap<(NodeIndex, NodeIndex), f64>,
    default_min_length: f64,
    weights: &MipWeights,
    spacing_pairs: &[EdgeSpacingPair],
    d_min: f64,
) -> MipSolution {
    // Build edges with geographic sectors
    // Deduplicate edges (parallel edges between same pair would create conflicting constraints)
    let mut seen_pairs = std::collections::HashSet::new();
    let edges: Vec<_> = collapsed
        .kept_edges
        .iter()
        .filter_map(|&(from, to)| {
            if from == to {
                return None; // Skip self-loops
            }
            let pair = if from < to { (from, to) } else { (to, from) };
            if !seen_pairs.insert(pair) {
                return None; // Skip duplicate
            }
            let from_pos = collapsed.positions.get(&from)?;
            let to_pos = collapsed.positions.get(&to)?;
            Some((from, to, sector(*from_pos, *to_pos)))
        })
        .collect();

    log::info!(
        "MIP model: {} nodes, {} edges (of {} total)",
        collapsed.kept_nodes.len(),
        edges.len(),
        collapsed.kept_edges.len(),
    );

    let mut vars = ProblemVariables::new();

    // Position variables: x(v), y(v) for each node
    let x: HashMap<NodeIndex, Variable> = collapsed
        .kept_nodes
        .iter()
        .map(|&n| (n, vars.add(variable().min(0.0).max(BIG_M))))
        .collect();
    let y: HashMap<NodeIndex, Variable> = collapsed
        .kept_nodes
        .iter()
        .map(|&n| (n, vars.add(variable().min(0.0).max(BIG_M))))
        .collect();

    let mut constraints: Vec<Constraint> = Vec::new();
    let mut objective = Expression::from(0.0);

    // For each edge: direction selection (3 admissible directions) + coordinate constraints
    for &(from, to, geo_sector) in &edges {
        let (prec, orig, succ) = admissible_directions(geo_sector);

        let b_prec = vars.add(variable().binary());
        let b_orig = vars.add(variable().binary());
        let b_succ = vars.add(variable().binary());

        // Exactly one selected
        constraints.push((b_prec + b_orig + b_succ - 1.0).leq(Expression::from(0.0)));
        constraints.push((b_prec + b_orig + b_succ - 1.0).geq(Expression::from(0.0)));

        // Edge length variable
        let pair = if from < to { (from, to) } else { (to, from) };
        let min_len = edge_min_lengths
            .get(&pair)
            .copied()
            .unwrap_or(default_min_length);
        let edge_len = vars.add(variable().min(min_len).max(BIG_M));

        let xu = x[&from];
        let yu = y[&from];
        let xv = x[&to];
        let yv = y[&to];

        for &(dir, binary) in &[(prec, b_prec), (orig, b_orig), (succ, b_succ)] {
            let (x_sign, y_sign, _diagonal) = direction_deltas(dir);
            add_axis_constraint(xu, xv, edge_len, x_sign, binary, &mut constraints);
            add_axis_constraint(yu, yv, edge_len, -y_sign, binary, &mut constraints);
        }

        // Soft constraint (S3): minimize total edge length
        objective += weights.edge_length * edge_len;

        // Soft constraint (S2): cost 0 for orig, 1 for prec/succ
        objective += weights.relative_position * (b_prec + b_succ);
    }

    // (H2) skipped -- causes more harm than good with 3-direction model.
    // Geographic ordering is handled by (S2) direction preference + (H4) hub spacing.

    // (S4) Geographic ordering for sibling branch endpoints:
    // For pairs of edges from the same hub going in similar directions,
    // penalize if the far endpoints are in the wrong geographic order.
    let geo_order_weight = weights.relative_position * 2.0;
    for &(ei, ej) in spacing_pairs {
        if ei >= edges.len() || ej >= edges.len() {
            continue;
        }
        let (_, far_i, _) = edges[ei];
        let (_, far_j, _) = edges[ej];

        let Some(&(lon_i, lat_i)) = collapsed.positions.get(&far_i) else { continue };
        let Some(&(lon_j, lat_j)) = collapsed.positions.get(&far_j) else { continue };

        // Y ordering: higher lat = north = lower screen y
        if (lat_i - lat_j).abs() > 0.02 {
            let slack = vars.add(variable().min(0.0));
            if lat_i > lat_j {
                constraints.push((y[&far_j] - y[&far_i] + slack).geq(Expression::from(0.0)));
            } else {
                constraints.push((y[&far_i] - y[&far_j] + slack).geq(Expression::from(0.0)));
            }
            objective += geo_order_weight * slack;
        }

        // X ordering
        if (lon_i - lon_j).abs() > 0.02 {
            let slack = vars.add(variable().min(0.0));
            if lon_i > lon_j {
                constraints.push((x[&far_i] - x[&far_j] + slack).geq(Expression::from(0.0)));
            } else {
                constraints.push((x[&far_j] - x[&far_i] + slack).geq(Expression::from(0.0)));
            }
            objective += geo_order_weight * slack;
        }
    }

    // (H4) Edge spacing constraints for crossing pairs
    for &(ei, ej) in spacing_pairs {
        if ei >= edges.len() || ej >= edges.len() {
            continue;
        }
        let (u1, v1, _) = edges[ei];
        let (u2, v2, _) = edges[ej];

        add_edge_spacing(
            x[&u1], y[&u1], x[&v1], y[&v1],
            x[&u2], y[&u2], x[&v2], y[&v2],
            d_min, &mut vars, &mut constraints,
        );
    }

    log::info!(
        "MIP model: {} spacing pairs added",
        spacing_pairs.len(),
    );

    // Solve
    let mut solver = vars.minimise(objective).using(highs)
        .set_option("primal_feasibility_tolerance", "1e-4")
        .set_option("mip_feasibility_tolerance", "1e-4");
    for c in constraints {
        solver = solver.with(c);
    }

    match solver.solve() {
        Ok(solution) => {
            let positions: HashMap<NodeIndex, (f64, f64)> = collapsed
                .kept_nodes
                .iter()
                .map(|&n| (n, (solution.value(x[&n]), solution.value(y[&n]))))
                .collect();

            log::info!("MIP solver: success, {} nodes positioned", positions.len());
            MipSolution {
                positions,
                success: true,
            }
        }
        Err(e) => {
            log::info!("MIP solver error: {e:?}");
            MipSolution {
                positions: HashMap::new(),
                success: false,
            }
        }
    }
}

/// Add big-M coordinate constraint for one axis.
///
/// When `binary` = 1 (direction active):
/// - `sign` = 1:  `coord_to` - `coord_from` = `edge_len`
/// - `sign` = -1: `coord_from` - `coord_to` = `edge_len`
/// - `sign` = 0:  `coord_from` = `coord_to`
///
/// When `binary` = 0: constraints are relaxed.
fn add_axis_constraint(
    coord_from: Variable,
    coord_to: Variable,
    edge_len: Variable,
    sign: i8,
    binary: Variable,
    constraints: &mut Vec<Constraint>,
) {
    // Big-M relaxation:
    //   geq: expr >= -M(1-b) = -M + Mb  ->  (expr - Mb).geq(-M)
    //   leq: expr <= M(1-b) = M - Mb   ->  (expr + Mb).leq(M)
    match sign {
        1 => {
            // When active (b=1): coord_to - coord_from = edge_len
            let diff = coord_to - coord_from - edge_len;
            constraints.push((diff.clone() - BIG_M * binary).geq(-BIG_M));
            constraints.push((diff + BIG_M * binary).leq(BIG_M));
        }
        -1 => {
            // When active (b=1): coord_from - coord_to = edge_len
            let diff = coord_from - coord_to - edge_len;
            constraints.push((diff.clone() - BIG_M * binary).geq(-BIG_M));
            constraints.push((diff + BIG_M * binary).leq(BIG_M));
        }
        _ => {
            // When active (b=1): coord_from = coord_to
            let diff = coord_from - coord_to;
            constraints.push((diff.clone() - BIG_M * binary).geq(-BIG_M));
            constraints.push((diff + BIG_M * binary).leq(BIG_M));
        }
    }
}

/// (H4) Edge spacing: force two non-incident edges apart by `d_min` in at least one
/// of the 8 octilinear directions, using the L-infinity metric.
///
/// For each direction, all 4 vertex combinations of the two edges must be separated.
/// The diagonal directions use z1=(x+y)/2 and z2=(x-y)/2 coordinates.
#[allow(clippy::too_many_arguments)]
fn add_edge_spacing(
    xu1: Variable, yu1: Variable, xv1: Variable, yv1: Variable,
    xu2: Variable, yu2: Variable, xv2: Variable, yv2: Variable,
    d_min: f64,
    vars: &mut ProblemVariables,
    constraints: &mut Vec<Constraint>,
) {
    // 8 binary variables, one per octilinear direction
    let binaries: Vec<Variable> = (0..8).map(|_| vars.add(variable().binary())).collect();

    // At least one must be active
    let sum: Expression = binaries.iter().copied().sum();
    constraints.push((sum - 1.0).geq(Expression::from(0.0)));

    // E (dir 0): e1 east of e2
    add_separation_constraints(
        &[(xu1, xu2), (xu1, xv2), (xv1, xu2), (xv1, xv2)],
        binaries[0], d_min, constraints,
    );

    // W (dir 4): e1 west of e2
    add_separation_constraints(
        &[(xu2, xu1), (xu2, xv1), (xv2, xu1), (xv2, xv1)],
        binaries[4], d_min, constraints,
    );

    // N (dir 2): e1 north of e2 (screen: e2 has larger y)
    add_separation_constraints(
        &[(yu2, yu1), (yu2, yv1), (yv2, yu1), (yv2, yv1)],
        binaries[2], d_min, constraints,
    );

    // S (dir 6): e1 south of e2
    add_separation_constraints(
        &[(yu1, yu2), (yu1, yv2), (yv1, yu2), (yv1, yv2)],
        binaries[6], d_min, constraints,
    );

    // For diagonals, use z1 = x + y and z2 = x - y (scaled by the L-infinity metric)
    // NE (dir 1): z1(e1) - z1(e2) >= d_min, i.e., (x+y)(e1) - (x+y)(e2) >= d_min
    add_diagonal_separation(
        xu1, yu1, xv1, yv1, xu2, yu2, xv2, yv2,
        true, true, // z1 = x+y, e1 > e2
        binaries[1], d_min, constraints,
    );

    // SW (dir 5): z1(e2) - z1(e1) >= d_min
    add_diagonal_separation(
        xu1, yu1, xv1, yv1, xu2, yu2, xv2, yv2,
        true, false, // z1 = x+y, e2 > e1
        binaries[5], d_min, constraints,
    );

    // NW (dir 3): z2(e2) - z2(e1) >= d_min, where z2 = x-y
    // e1 is NW of e2 means e2 has larger x-y
    add_diagonal_separation(
        xu1, yu1, xv1, yv1, xu2, yu2, xv2, yv2,
        false, false, // z2 = x-y, e2 > e1
        binaries[3], d_min, constraints,
    );

    // SE (dir 7): z2(e1) - z2(e2) >= d_min
    add_diagonal_separation(
        xu1, yu1, xv1, yv1, xu2, yu2, xv2, yv2,
        false, true, // z2 = x-y, e1 > e2
        binaries[7], d_min, constraints,
    );
}

/// Add separation constraints for 4 vertex pairs in one axis direction
fn add_separation_constraints(
    pairs: &[(Variable, Variable); 4],
    binary: Variable,
    d_min: f64,
    constraints: &mut Vec<Constraint>,
) {
    for &(a, b) in pairs {
        // a - b >= d_min when binary=1, relaxed when binary=0
        constraints.push((a - b - BIG_M * binary).geq(d_min - BIG_M));
    }
}

/// Add diagonal separation constraints using z1=(x+y) or z2=(x-y)
#[allow(clippy::too_many_arguments)]
fn add_diagonal_separation(
    xu1: Variable, yu1: Variable, xv1: Variable, yv1: Variable,
    xu2: Variable, yu2: Variable, xv2: Variable, yv2: Variable,
    use_z1: bool, e1_greater: bool,
    binary: Variable, d_min: f64,
    constraints: &mut Vec<Constraint>,
) {
    // z1 = x + y, z2 = x - y
    // For each of 4 vertex combinations: z(e1_vertex) - z(e2_vertex) >= d_min
    let e1_verts = [(xu1, yu1), (xv1, yv1)];
    let e2_verts = [(xu2, yu2), (xv2, yv2)];

    for &(x1, y1) in &e1_verts {
        for &(x2, y2) in &e2_verts {
            // z = x +/- y depending on use_z1
            // diff = z(a) - z(b) where a is the "greater" side
            let diff = if e1_greater {
                if use_z1 {
                    (x1 + y1) - (x2 + y2) // z1(e1) - z1(e2)
                } else {
                    (x1 - y1) - (x2 - y2) // z2(e1) - z2(e2)
                }
            } else if use_z1 {
                (x2 + y2) - (x1 + y1) // z1(e2) - z1(e1)
            } else {
                (x2 - y2) - (x1 - y1) // z2(e2) - z2(e1)
            };
            constraints.push((diff - BIG_M * binary).geq(d_min - BIG_M));
        }
    }
}

/// Get the edge list (for use by the iterative solver in mod.rs)
pub fn build_edge_list(collapsed: &CollapsedGraph) -> Vec<(NodeIndex, NodeIndex, u8)> {
    let mut seen_pairs = std::collections::HashSet::new();
    collapsed
        .kept_edges
        .iter()
        .filter_map(|&(from, to)| {
            if from == to {
                return None;
            }
            let pair = if from < to { (from, to) } else { (to, from) };
            if !seen_pairs.insert(pair) {
                return None;
            }
            let from_pos = collapsed.positions.get(&from)?;
            let to_pos = collapsed.positions.get(&to)?;
            Some((from, to, sector(*from_pos, *to_pos)))
        })
        .collect()
}
