use leptos::{WriteSignal, SignalSet};
use crate::conflict::Conflict;
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
        let (conflicts, _) = crate::conflict::detect_line_conflicts(&journeys, &graph);
        self.set_conflicts.set(conflicts);
    }
}
