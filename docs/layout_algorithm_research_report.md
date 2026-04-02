# Layout Algorithm Research Report: Nimby Rails Import

## Executive Summary

This report analyzes the layout challenges for station nodes and tracks imported from Nimby Rails and provides recommendations for improvements. The key requirements are:

1. **Pseudo-geographical positioning** - Stations to the north should appear north in the layout
2. **Grid-based layout** - Follow the infrastructure view's 30px grid system
3. **No overlapping** - Prevent overlapping nodes and edges
4. **Parallel line handling** - Lines through different intermediate stations
5. **Loop handling** - Lines that loop back on themselves

---

## Part 1: Current Implementation Analysis

### 1.1 Existing Architecture

The current layout system (`auto_layout.rs`, ~1,500 lines) uses a **hierarchical branch-based approach**:

1. **Spine Detection**: Find the main "backbone" using either:
   - `find_longest_path()` - for basic graphs
   - `find_heaviest_path()` - uses edge weights from line usage during NIMBY import

2. **Hub Identification**: The node with highest degree on the spine becomes the central hub

3. **Spine Placement**: Place spine nodes outward from hub in both directions

4. **Branch Placement**: BFS traversal places branches perpendicular to spine using a scoring system

### 1.2 Geographic Constraints

The `GeographicHints` struct provides real-world coordinates:
- Stores `(longitude, latitude)` for each node
- `preferred_direction()` calculates compass bearing between stations
- `filter_valid_directions()` restricts placement to within 90° of true geographic direction

**Constraint Type**: Hard constraint - directions more than 90° from reality are rejected.

### 1.3 Current Strengths

| Feature | Implementation | Status |
|---------|---------------|--------|
| Geographic hints | ✅ Uses lon/lat from NIMBY data | Working |
| Grid snapping | ✅ 30px grid via `snap_to_grid()` | Working |
| Collision detection | ✅ `has_node_collision_at()` | Working |
| Parallel routes | ✅ `detect_parallel_routes()` offsets local lines | Partial |
| Station clusters | ✅ Same-name stations placed together | Working |
| Passing loops | ✅ Skipped during layout, positioned later | Working |

### 1.4 Current Weaknesses

1. **Spine Selection Issues**
   - Longest/heaviest path doesn't always match visual importance
   - Multiple major corridors compete for "spine" status

2. **Branch Direction Conflicts**
   - Branches from the same node compete for limited directions
   - Crowding penalty helps but doesn't prevent all conflicts

3. **Loop/Cycle Handling**
   - When a branch reconnects to an already-placed node, the layout can become awkward
   - No explicit cycle detection or handling

4. **Parallel Line Detection**
   - Only detects parallel routes when there's BOTH a direct edge AND an alternative path
   - Misses cases where two separate lines share endpoints but take different routes

5. **Fallback Positions**
   - Emergency positioning can create long jumps in spacing
   - No backtracking if an early placement blocks better later options

---

## Part 2: Academic Research on Graph Layout Algorithms

### 2.1 Metro Map Layout (Nöllenburg & Wolff)

The seminal work on automatic metro map generation uses **Mixed Integer Programming (MIP)**:

**Hard Constraints:**
- Octilinear edges (0°, 45°, 90°, 135°, etc.)
- Minimum edge length
- No edge crossings (unless real crossings exist)
- Preserve topology (planarity)

**Soft Constraints (optimized):**
- Minimize total edge length
- Minimize number of bends
- Maximize bend angles
- Maintain geographic "mental map"

**Pros:** Produces high-quality results, guarantees constraint satisfaction
**Cons:** NP-hard, requires commercial solvers (Gurobi), slow for large networks

**Source:** [Nöllenburg & Wolff, IEEE TVCG 2011](https://www.researchgate.net/publication/44626536_Drawing_and_Labeling_High-Quality_Metro_Maps_by_Mixed-Integer_Programming)

### 2.2 Force-Directed Layouts

Classic force-directed (Fruchterman-Reingold, Kamada-Kawai):

**Mechanism:**
- Repulsive forces between all nodes
- Attractive forces along edges (Hooke's Law springs)
- Iterate until equilibrium

**Variants:**
- D3.js force simulation with fixed nodes (`fx`, `fy`)
- WebCola's constraint-based approach

**Pros:** Simple to implement, handles arbitrary graphs
**Cons:** Unstable/jittery, no octilinear constraint, can produce poor results

**Source:** [Force-Directed Graph Drawing - Wikipedia](https://en.wikipedia.org/wiki/Force-directed_graph_drawing)

### 2.3 Stress Majorization

Used by graphviz (neato) and WebCola:

**Mechanism:**
- Define "stress" as sum of squared differences between graph-theoretic distance and geometric distance
- Iteratively minimize stress using majorization

**Key Advantage:** Supports constraints via gradient projection:
- Alignment constraints (nodes on same horizontal/vertical line)
- Separation constraints (minimum spacing)
- Ordering constraints (A must be left of B)

**Pros:** Converges to local optimum (no jitter), supports constraints
**Cons:** May get stuck in poor local minima

**Source:** [Gansner, Koren & North, Graph Drawing 2004](https://www.graphviz.org/documentation/GKN04.pdf)

### 2.4 Sugiyama Hierarchical Layout

Designed for directed acyclic graphs (DAGs):

**Four Phases:**
1. **Cycle removal** - Temporarily reverse edges to make DAG
2. **Layer assignment** - Assign nodes to horizontal layers
3. **Crossing minimization** - Order nodes within layers
4. **Coordinate assignment** - Final x,y positions

**Pros:** Excellent for flow-based visualizations
**Cons:** Requires direction, creates many dummy nodes for long edges

**Source:** [Layered Graph Drawing - Wikipedia](https://en.wikipedia.org/wiki/Layered_graph_drawing)

### 2.5 WebCola Constraint-Based Layout

**cola.js** extends force-directed with constraint satisfaction:

**Constraint Types:**
```json
{
  "type": "alignment",
  "axis": "x",
  "offsets": [{"node": 1, "offset": 0}, {"node": 2, "offset": 0}]
}
```

**Features:**
- Alignment (nodes on same axis)
- Separation (minimum distance)
- Groups (nodes in bounding boxes)
- Convergent (no jitter)

**Three-Phase Schedule:**
1. Unconstrained (untangle)
2. User constraints only
3. Full constraints including anti-overlap

**Source:** [WebCola](https://ialab.it.monash.edu/webcola/)

### 2.6 Comparison Table

| Algorithm | Quality | Speed | Constraints | Octilinear | Grid |
|-----------|---------|-------|-------------|------------|------|
| MIP (Nöllenburg) | ⭐⭐⭐⭐⭐ | ⭐⭐ | ⭐⭐⭐⭐⭐ | ✅ | ✅ |
| Force-Directed | ⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐ | ❌ | ❌ |
| Stress Majorization | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐ | ❌ | ❌ |
| Sugiyama | ⭐⭐⭐⭐ | ⭐⭐⭐⭐ | ⭐⭐⭐ | ❌ | ✅ |
| WebCola | ⭐⭐⭐⭐ | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ❌ | ✅ |
| **Current (rail_graph)** | ⭐⭐⭐ | ⭐⭐⭐⭐⭐ | ⭐⭐⭐ | ✅ | ✅ |

---

## Part 3: Specific Challenge Analysis

### 3.1 Challenge: Pseudo-Geographic Layout

**Problem:** Stations must be positioned roughly according to their real-world compass directions.

**Current Implementation:**
- `GeographicHints.preferred_direction()` calculates true bearing
- `filter_valid_directions()` limits to ±90° of true direction
- Scoring favors directions closer to geographic truth

**Issues:**
1. Hard 90° cutoff can conflict with topology requirements
2. Long-distance relationships may override local geography
3. No consideration of overall "shape" preservation

**Recommendation: Geographic Shape Constraint**

Instead of point-to-point bearings, consider **shape-preserving** constraints:

```rust
/// Compute a "geographic embedding" that preserves relative positions
fn compute_geographic_embedding(
    geo_hints: &GeographicHints,
    nodes: &[NodeIndex],
) -> HashMap<NodeIndex, (f64, f64)> {
    // 1. Compute bounding box of lon/lat coordinates
    // 2. Scale to fit canvas while preserving aspect ratio
    // 3. Return target positions (soft constraints)
}
```

This gives each node a "target position" based on scaled geography, which can be used as a soft constraint.

### 3.2 Challenge: Parallel Lines Through Different Intermediate Stations

**Problem:** Two lines A→B may take different routes:
- Line 1: A → X → Y → B (local)
- Line 2: A → P → Q → B (express via different corridor)

**Current Implementation:**
- `detect_parallel_routes()` only finds cases where there's a direct edge A↔B AND an alternative path
- Intermediate stations get offset perpendicular to the A→B line

**Issues:**
1. Doesn't handle two separate corridors without direct edge
2. Offset direction is arbitrary (always "below")
3. No consideration of which route is more important

**Recommendation: Path-Based Corridor Detection**

```rust
struct Corridor {
    endpoints: (NodeIndex, NodeIndex),
    paths: Vec<Vec<NodeIndex>>,  // Multiple paths between same endpoints
    primary_path: usize,         // Which path is the "main" one
}

fn detect_corridors(
    graph: &RailwayGraph,
    geo_hints: &GeographicHints,
) -> Vec<Corridor> {
    // 1. For each pair of high-degree stations (hubs)
    // 2. Find ALL paths between them up to some max length
    // 3. Group paths that are geographically parallel
    // 4. Mark primary path based on line weights
}
```

**Layout Strategy for Corridors:**
1. Place primary path stations along the direct line
2. Place secondary path stations offset perpendicular
3. Use geographic hints to determine offset direction (north/south or east/west of primary)

### 3.3 Challenge: Lines Looping Back (Cycles)

**Problem:** A line may visit stations in a cycle:
- A → B → C → D → B (returns to B)
- Or a full loop: A → B → C → D → A

**Current Implementation:**
- No explicit cycle detection
- BFS traversal marks nodes as visited, so second visit is ignored
- This can cause the returning edge to cross over placed nodes

**Issues:**
1. No visual indication that a line loops
2. Edge rendering may cross other elements
3. Station order on loop is arbitrary

**Recommendation: Cycle-Aware Layout**

```rust
enum GraphComponent {
    Tree { root: NodeIndex, children: Vec<NodeIndex> },
    Cycle { nodes: Vec<NodeIndex> },
    Mixed { spine: Vec<NodeIndex>, loops: Vec<Vec<NodeIndex>> },
}

fn decompose_graph(graph: &RailwayGraph) -> Vec<GraphComponent> {
    // 1. Find all cycles using Tarjan's or DFS
    // 2. Classify remaining structure as trees
    // 3. Identify mixed (tree with loops attached)
}
```

**Layout Strategies for Cycles:**

1. **Balloon Layout for Small Loops:**
   - Loops with ≤8 stations: arrange in circle/oval
   - Entry/exit points positioned for smooth connection

2. **Hairpin Layout for Turnarounds:**
   - When a line goes A→B→C→B (returns same way)
   - Place B at turning point, visualize as fold

3. **Figure-8 for Complex Loops:**
   - When multiple loops share a station
   - Use orthogonal grid positions with clear crossing

### 3.4 Challenge: Overlapping Stations and Lines

**Problem:** Layout must avoid:
- Stations placed at same position
- Stations too close together (label overlap)
- Edge lines crossing through stations

**Current Implementation:**
- `has_node_collision_at()` checks minimum distance
- `find_fallback_position()` spirals outward to find free position
- No edge-through-node collision check

**Issues:**
1. Spiral fallback can place nodes far from ideal position
2. No global optimization (greedy placement)
3. Edge routing is straight lines only

**Recommendation: Post-Processing Overlap Removal**

Implement a separate optimization pass after initial layout:

```rust
fn remove_overlaps(
    graph: &mut RailwayGraph,
    max_iterations: usize,
) {
    for _ in 0..max_iterations {
        let overlaps = find_all_overlaps(graph);
        if overlaps.is_empty() {
            break;
        }

        for (node_a, node_b) in overlaps {
            // Compute repulsion vector
            // Apply minimum displacement to resolve
            // Snap to grid
        }
    }
}
```

**For edge-through-node collisions:**
- Detect edges that pass within threshold of intermediate nodes
- Either: adjust node positions, or introduce edge bends

---

## Part 4: Recommended Algorithm Architecture

### 4.1 Multi-Phase Layout Pipeline

```
┌─────────────────────────────────────────────────────────────────┐
│                     PHASE 1: ANALYSIS                           │
├─────────────────────────────────────────────────────────────────┤
│  • Detect corridors and parallel routes                         │
│  • Find cycles and decompose graph                              │
│  • Identify station clusters (transfers)                        │
│  • Build edge weight map from line usage                        │
│  • Compute geographic shape embedding                           │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 2: INITIAL PLACEMENT                   │
├─────────────────────────────────────────────────────────────────┤
│  • Place primary corridor (heaviest/longest path)               │
│  • Place secondary corridors with perpendicular offset          │
│  • Place cycle nodes using appropriate pattern                  │
│  • Place remaining branches from hubs                           │
│  • Respect geographic soft constraints                          │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 3: CONSTRAINT SOLVING                  │
├─────────────────────────────────────────────────────────────────┤
│  • Apply stress majorization with constraints:                  │
│    - Geographic target positions (soft)                         │
│    - Minimum node separation (hard)                             │
│    - Octilinear edge directions (soft)                          │
│    - Cluster proximity (soft)                                   │
│  • Run for limited iterations                                   │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│                    PHASE 4: REFINEMENT                          │
├─────────────────────────────────────────────────────────────────┤
│  • Snap all positions to grid                                   │
│  • Remove any remaining overlaps via displacement               │
│  • Adjust edge routing if needed                                │
│  • Place cluster siblings                                       │
│  • Position passing loops between adjacent stations             │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Constraint-Based Stress Majorization

Adapt the current scoring system into a proper constraint solver:

```rust
struct LayoutConstraint {
    constraint_type: ConstraintType,
    weight: f64,  // Higher = harder constraint
}

enum ConstraintType {
    // Hard constraints
    MinimumSeparation { node_a: NodeIndex, node_b: NodeIndex, min_dist: f64 },
    GridAlignment { node: NodeIndex },

    // Soft constraints
    GeographicTarget { node: NodeIndex, target: (f64, f64) },
    OctilinearEdge { edge: EdgeIndex },
    ClusterProximity { nodes: Vec<NodeIndex>, max_spread: f64 },
    EdgeDirection { edge: EdgeIndex, preferred_angle: f64 },
}

fn solve_layout(
    graph: &mut RailwayGraph,
    constraints: &[LayoutConstraint],
    initial_positions: &HashMap<NodeIndex, (f64, f64)>,
    max_iterations: usize,
) {
    // Gradient descent with constraint projection
    for _ in 0..max_iterations {
        // 1. Compute gradients from soft constraints
        // 2. Take step in gradient direction
        // 3. Project onto hard constraint feasible region
        // 4. Snap to grid
    }
}
```

### 4.3 Recommended Implementation Priority

| Priority | Feature | Effort | Impact |
|----------|---------|--------|--------|
| 1 | Better cycle detection and handling | Medium | High |
| 2 | Improved corridor detection (multiple paths) | Medium | High |
| 3 | Geographic shape embedding | Low | Medium |
| 4 | Post-processing overlap removal | Low | Medium |
| 5 | Constraint-based refinement pass | High | High |
| 6 | Edge bend routing | High | Medium |

---

## Part 5: Implementation Recommendations

### 5.1 Short-Term Improvements (Low Effort)

#### 5.1.1 Improve Geographic Constraint Handling

```rust
// Current: Hard 90° cutoff
// Better: Soft penalty with gradual falloff

fn geographic_score(actual_dir: f64, geo_dir: f64) -> f64 {
    let diff = angle_difference(actual_dir, geo_dir);
    // Gaussian falloff instead of hard cutoff
    let sigma = std::f64::consts::FRAC_PI_4; // 45° standard deviation
    (-diff.powi(2) / (2.0 * sigma.powi(2))).exp() * GEOGRAPHIC_BONUS_MAX
}
```

#### 5.1.2 Add Cycle Detection

```rust
fn find_cycles(graph: &RailwayGraph) -> Vec<Vec<NodeIndex>> {
    let mut cycles = Vec::new();
    let mut visited = HashSet::new();
    let mut rec_stack = Vec::new();

    fn dfs(
        graph: &RailwayGraph,
        node: NodeIndex,
        parent: Option<NodeIndex>,
        visited: &mut HashSet<NodeIndex>,
        rec_stack: &mut Vec<NodeIndex>,
        cycles: &mut Vec<Vec<NodeIndex>>,
    ) {
        visited.insert(node);
        rec_stack.push(node);

        for neighbor in graph.graph.neighbors_undirected(node) {
            if Some(neighbor) == parent {
                continue;
            }
            if let Some(pos) = rec_stack.iter().position(|&n| n == neighbor) {
                // Found cycle
                cycles.push(rec_stack[pos..].to_vec());
            } else if !visited.contains(&neighbor) {
                dfs(graph, neighbor, Some(node), visited, rec_stack, cycles);
            }
        }

        rec_stack.pop();
    }

    for node in graph.graph.node_indices() {
        if !visited.contains(&node) {
            dfs(graph, node, None, &mut visited, &mut rec_stack, &mut cycles);
        }
    }

    cycles
}
```

#### 5.1.3 Better Parallel Corridor Detection

Extend `detect_parallel_routes()` to find all paths between hub pairs:

```rust
fn detect_all_corridors(
    graph: &RailwayGraph,
    geo_hints: Option<&GeographicHints>,
) -> Vec<Corridor> {
    let hubs: Vec<NodeIndex> = graph.graph.node_indices()
        .filter(|&n| graph.graph.neighbors_undirected(n).count() >= 3)
        .collect();

    let mut corridors = Vec::new();

    for (i, &hub_a) in hubs.iter().enumerate() {
        for &hub_b in hubs.iter().skip(i + 1) {
            // Find all simple paths between hub_a and hub_b
            let paths = find_all_simple_paths(graph, hub_a, hub_b, 10); // max length 10

            if paths.len() >= 2 {
                // Multiple paths between same hubs = corridor
                corridors.push(Corridor {
                    endpoints: (hub_a, hub_b),
                    paths,
                    primary_path: 0, // TODO: determine by edge weights
                });
            }
        }
    }

    corridors
}
```

### 5.2 Medium-Term Improvements (Medium Effort)

#### 5.2.1 Two-Pass Layout with Refinement

```rust
pub fn apply_layout_with_refinement(
    graph: &mut RailwayGraph,
    height: f64,
    settings: &ProjectSettings,
    geo_hints: Option<&GeographicHints>,
) {
    // Pass 1: Current algorithm (fast, approximate)
    apply_layout_internal(graph, height, settings, geo_hints, &HashSet::new(), None);

    // Pass 2: Refinement
    refine_layout(graph, geo_hints, settings);
}

fn refine_layout(
    graph: &mut RailwayGraph,
    geo_hints: Option<&GeographicHints>,
    settings: &ProjectSettings,
) {
    let base_spacing = settings.default_node_distance_grid_squares * GRID_SIZE;

    for _ in 0..100 {
        let mut any_moved = false;

        for node in graph.graph.node_indices() {
            let Some(pos) = graph.get_station_position(node) else { continue };

            // Compute forces
            let mut force = (0.0, 0.0);

            // Repulsion from nearby nodes
            for other in graph.graph.node_indices() {
                if other == node { continue; }
                let Some(other_pos) = graph.get_station_position(other) else { continue };
                let dx = pos.0 - other_pos.0;
                let dy = pos.1 - other_pos.1;
                let dist = (dx*dx + dy*dy).sqrt().max(1.0);
                if dist < base_spacing {
                    let repulsion = (base_spacing - dist) / dist;
                    force.0 += dx * repulsion * 0.1;
                    force.1 += dy * repulsion * 0.1;
                }
            }

            // Attraction to geographic target
            if let Some(target) = compute_geo_target(node, geo_hints) {
                force.0 += (target.0 - pos.0) * 0.01;
                force.1 += (target.1 - pos.1) * 0.01;
            }

            // Apply force if significant
            if force.0.abs() > 1.0 || force.1.abs() > 1.0 {
                let new_pos = snap_to_grid(pos.0 + force.0, pos.1 + force.1);
                if !has_node_collision_at(graph, new_pos, node, base_spacing) {
                    graph.set_station_position(node, new_pos);
                    any_moved = true;
                }
            }
        }

        if !any_moved { break; }
    }
}
```

#### 5.2.2 Cycle Layout Patterns

```rust
fn layout_cycle(
    graph: &mut RailwayGraph,
    cycle: &[NodeIndex],
    center: (f64, f64),
    radius: f64,
) {
    let n = cycle.len();
    for (i, &node) in cycle.iter().enumerate() {
        let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
        // Offset to align with grid
        let pos = snap_to_grid(
            center.0 + radius * angle.cos(),
            center.1 + radius * angle.sin(),
        );
        graph.set_station_position(node, pos);
    }
}
```

### 5.3 Long-Term Improvements (High Effort)

#### 5.3.1 Full Constraint Solver

Implement proper constrained stress majorization:

- Use quadratic programming for continuous relaxation
- Gradient projection for constraint satisfaction
- Reference: [Dwyer et al., TVCG 2006](https://www.sciencedirect.com/science/article/pii/S0012365X08000083)

#### 5.3.2 Mixed Integer Programming

For highest quality results, implement MIP-based layout:

- Define binary variables for edge directions (8 choices)
- Linear constraints for non-crossing, minimum distance
- Objective function for total edge length + bends

Requires: Integration with a solver (coin-cbc, HiGHS, or commercial Gurobi)

---

## Part 6: Summary and Next Steps

### Key Findings

1. **The current algorithm is fundamentally sound** - hierarchical spine + branch approach works well for tree-like networks

2. **Main gaps are cycle handling and corridor detection** - these cause the most visual problems

3. **Geographic constraints work but could be softer** - hard 90° cutoff causes conflicts

4. **No post-processing refinement** - greedy placement without backtracking

### Recommended Approach

**Phase 1 (Immediate):**
- Add cycle detection using DFS
- Special layout for cycles (circular/oval arrangement)
- Soften geographic constraints with Gaussian falloff

**Phase 2 (Short-term):**
- Improve corridor detection to find all hub-to-hub paths
- Add refinement pass with force-based adjustment
- Better offset direction for parallel routes

**Phase 3 (Medium-term):**
- Implement proper constraint solver
- Add edge bend routing for unavoidable crossings
- Consider interactive adjustment tools

### Sources

- [Nöllenburg & Wolff - Metro Map MIP](https://www.researchgate.net/publication/44626536_Drawing_and_Labeling_High-Quality_Metro_Maps_by_Mixed-Integer_Programming)
- [Force-Directed Graph Drawing](https://en.wikipedia.org/wiki/Force-directed_graph_drawing)
- [Stress Majorization](https://www.graphviz.org/documentation/GKN04.pdf)
- [WebCola Constraint-Based Layout](https://ialab.it.monash.edu/webcola/)
- [Layered Graph Drawing (Sugiyama)](https://en.wikipedia.org/wiki/Layered_graph_drawing)
- [transit-map MIP Tool](https://github.com/juliuste/transit-map)
- [yFiles Layout Documentation](https://docs.yworks.com/yfiles-html/dguide/layout/layout-summary.html)
- [Constrained Stress Majorization](https://www.sciencedirect.com/science/article/pii/S0012365X08000083)
