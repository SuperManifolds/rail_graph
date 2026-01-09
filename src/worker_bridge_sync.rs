use leptos::{WriteSignal, SignalSet};
use crate::conflict::{Conflict, SerializableConflictContext};
use crate::train_journey::TrainJourney;
use crate::models::{RailwayGraph, ProjectSettings};

/// Synchronous version of `ConflictDetector` for non-wasm32 targets (tests, etc.)
pub struct ConflictDetector {
    set_conflicts: WriteSignal<Vec<Conflict>>,
    set_is_calculating: WriteSignal<bool>,
}

impl ConflictDetector {
    #[must_use]
    pub fn new(set_conflicts: WriteSignal<Vec<Conflict>>, set_is_calculating: WriteSignal<bool>) -> Self {
        Self { set_conflicts, set_is_calculating }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn detect(&mut self, journeys: Vec<TrainJourney>, graph: RailwayGraph, settings: ProjectSettings) {
        self.set_is_calculating.set(true);

        // Build serializable context from graph
        let station_indices = graph.graph.node_indices()
            .enumerate()
            .map(|(idx, node_idx)| (node_idx, idx))
            .collect();
        let context = SerializableConflictContext::from_graph(
            &graph,
            station_indices,
            settings.station_margin,
            settings.minimum_separation,
            settings.ignore_same_direction_platform_conflicts,
        );

        let (conflicts, _) = crate::conflict::detect_line_conflicts(&journeys, &context);
        self.set_conflicts.set(conflicts);
        self.set_is_calculating.set(false);
    }
}
