use leptos::{WriteSignal, SignalSet};
use crate::conflict::{Conflict, StationCrossing};
use crate::train_journey::TrainJourney;
use crate::models::RailwayGraph;

/// Synchronous version of `ConflictDetector` for non-wasm32 targets (tests, etc.)
pub struct ConflictDetector {
    set_conflicts: WriteSignal<Vec<Conflict>>,
    set_crossings: WriteSignal<Vec<StationCrossing>>,
}

impl ConflictDetector {
    #[must_use]
    pub fn new(
        set_conflicts: WriteSignal<Vec<Conflict>>,
        set_crossings: WriteSignal<Vec<StationCrossing>>,
    ) -> Self {
        Self {
            set_conflicts,
            set_crossings,
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn detect(&mut self, journeys: Vec<TrainJourney>, graph: RailwayGraph) {
        let (conflicts, crossings) = crate::conflict::detect_line_conflicts(&journeys, &graph);
        self.set_conflicts.set(conflicts);
        self.set_crossings.set(crossings);
    }
}
