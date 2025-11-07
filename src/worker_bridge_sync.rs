use leptos::{WriteSignal, SignalSet};
use crate::conflict::{Conflict, SerializableConflictContext};
use crate::train_journey::TrainJourney;
use crate::models::RailwayGraph;
use crate::import::geojson::{GeoJsonImportRequest, GeoJsonImportResponse};

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

/// Synchronous version of `GeoJsonImporter` - runs on main thread for debugging
pub struct GeoJsonImporter<F>
where
    F: Fn(GeoJsonImportResponse) + 'static,
{
    callback: F,
}

impl<F> GeoJsonImporter<F>
where
    F: Fn(GeoJsonImportResponse) + 'static,
{
    pub fn new(callback: F) -> Self {
        leptos::logging::log!("🟡 Using SYNC GeoJsonImporter (debugging mode)");
        Self { callback }
    }

    pub fn import(&mut self, request: &GeoJsonImportRequest) {
        leptos::logging::log!("🟡 Sync import: scheduling work");

        let request = request.clone();
        leptos::logging::log!("🟡 Sync import: work scheduled, will execute shortly");

        leptos::logging::log!("🟡 Sync import: calling import_geojson_to_updates");
        let response = crate::import::geojson::import_geojson_to_updates(&request);
        leptos::logging::log!("🟡 Sync import: import_geojson_to_updates returned");

        // Call callback
        leptos::logging::log!("🟡 Sync import: calling callback");
        (self.callback)(response);
        leptos::logging::log!("🟡 Sync import: callback completed");
    }
}
