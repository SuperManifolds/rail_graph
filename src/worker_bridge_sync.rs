use leptos::{WriteSignal, SignalSet};
use crate::conflict::{Conflict, SerializableConflictContext};
use crate::train_journey::TrainJourney;
use crate::models::RailwayGraph;

/// Synchronous version of `ConflictDetector` for non-wasm32 targets (tests, etc.)
pub struct ConflictDetector {
    set_conflicts: WriteSignal<Vec<Conflict>>,
}

impl ConflictDetector {
    #[must_use]
    pub fn new(set_conflicts: WriteSignal<Vec<Conflict>>) -> Self {
        Self { set_conflicts }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn detect(&mut self, journeys: Vec<TrainJourney>, graph: RailwayGraph) {
        // Build serializable context from graph
        let station_indices = graph.graph.node_indices()
            .enumerate()
            .map(|(idx, node_idx)| (node_idx, idx))
            .collect();
        let context = SerializableConflictContext::from_graph(&graph, station_indices);

        let (conflicts, _) = crate::conflict::detect_line_conflicts(&journeys, &context);
        self.set_conflicts.set(conflicts);
    }
}
