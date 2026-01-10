use leptos::{WriteSignal, SignalSet};
use crate::conflict::{Conflict, SerializableConflictContext};
use crate::train_journey::TrainJourney;
use crate::models::{Line, Project, ProjectSettings};
use std::collections::HashSet;

/// Edge filter for view-based line filtering (same as wasm32 version)
pub type ViewEdgeFilter = Vec<usize>;

/// Check if a line touches the view (shares an edge or visits a station in the view)
fn line_touches_view(
    line: &Line,
    view_edge_set: &HashSet<usize>,
    view_station_set: &HashSet<petgraph::stable_graph::NodeIndex>,
    graph: &crate::models::RailwayGraph,
) -> bool {
    // Check if route shares any edge with the view
    let shares_edge = line.forward_route.iter().any(|seg| view_edge_set.contains(&seg.edge_index))
        || line.return_route.iter().any(|seg| view_edge_set.contains(&seg.edge_index));
    if shares_edge {
        return true;
    }
    // Check if route visits any station in the view (for platform conflicts)
    for seg in line.forward_route.iter().chain(line.return_route.iter()) {
        let edge_index = petgraph::stable_graph::EdgeIndex::new(seg.edge_index);
        if let Some((a, b)) = graph.graph.edge_endpoints(edge_index) {
            if view_station_set.contains(&a) || view_station_set.contains(&b) {
                return true;
            }
        }
    }
    false
}

/// Synchronous version of `ConflictDetector` for non-wasm32 targets (tests, etc.)
/// This version deserializes project bytes and generates journeys locally.
pub struct ConflictDetector {
    set_conflicts: WriteSignal<Vec<Conflict>>,
    set_is_calculating: WriteSignal<bool>,
}

impl ConflictDetector {
    #[must_use]
    pub fn new(set_conflicts: WriteSignal<Vec<Conflict>>, set_is_calculating: WriteSignal<bool>) -> Self {
        Self { set_conflicts, set_is_calculating }
    }

    /// Detect conflicts synchronously.
    /// Deserializes project from bytes and generates journeys locally.
    #[allow(clippy::needless_pass_by_value)]
    pub fn detect(
        &mut self,
        project_bytes: Vec<u8>,
        lines: Vec<Line>,
        settings: ProjectSettings,
        day_filter: Option<chrono::Weekday>,
        view_edge_filter: Option<ViewEdgeFilter>,
    ) {
        // Skip if no lines to check
        if lines.is_empty() {
            self.set_conflicts.set(vec![]);
            self.set_is_calculating.set(false);
            return;
        }

        self.set_is_calculating.set(true);

        // Deserialize project from bytes
        let Ok(project) = Project::from_bytes(&project_bytes) else {
            self.set_conflicts.set(vec![]);
            self.set_is_calculating.set(false);
            return;
        };

        // Filter to visible lines, optionally filtering by view edges/stations
        let visible_line_ids: HashSet<_> = lines.iter().map(|l| l.id).collect();
        let visible_lines: Vec<_> = if let Some(view_edges) = &view_edge_filter {
            // Build edge set and station set from view edges
            let view_edge_set: HashSet<usize> = view_edges.iter().copied().collect();
            let mut view_station_set: HashSet<petgraph::stable_graph::NodeIndex> = HashSet::new();
            for &edge_idx in view_edges {
                let edge_index = petgraph::stable_graph::EdgeIndex::new(edge_idx);
                if let Some((a, b)) = project.graph.graph.edge_endpoints(edge_index) {
                    view_station_set.insert(a);
                    view_station_set.insert(b);
                }
            }

            project.lines
                .into_iter()
                .filter(|line| visible_line_ids.contains(&line.id))
                .filter(|line| {
                    line_touches_view(line, &view_edge_set, &view_station_set, &project.graph)
                })
                .collect()
        } else {
            project.lines
                .into_iter()
                .filter(|line| visible_line_ids.contains(&line.id))
                .collect()
        };

        // Generate journeys
        let journeys = TrainJourney::generate_journeys(&visible_lines, &project.graph, day_filter);
        let journeys_vec: Vec<_> = journeys.values().cloned().collect();

        // Build serializable context from graph
        let station_indices = project.graph.graph.node_indices()
            .enumerate()
            .map(|(idx, node_idx)| (node_idx, idx))
            .collect();
        let context = SerializableConflictContext::from_graph(
            &project.graph,
            station_indices,
            settings.station_margin,
            settings.minimum_separation,
            settings.ignore_same_direction_platform_conflicts,
        );

        let (conflicts, _) = crate::conflict::detect_line_conflicts(&journeys_vec, &context);
        self.set_conflicts.set(conflicts);
        self.set_is_calculating.set(false);
    }
}
