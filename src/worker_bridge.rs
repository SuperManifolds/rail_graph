use gloo_worker::Spawnable;
use leptos::{create_signal, ReadSignal, WriteSignal, SignalSet};
use crate::conflict_worker::{ConflictWorker, ConflictRequest, ConflictResponse, BincodeCodec};
use crate::conflict::{Conflict, SerializableConflictContext};
use crate::train_journey::TrainJourney;
use crate::models::RailwayGraph;
use crate::geojson_worker::GeoJsonImportWorker;
use crate::import::geojson::{GeoJsonImportRequest, GeoJsonImportResponse};

pub struct ConflictDetector {
    worker: gloo_worker::WorkerBridge<ConflictWorker>,
}

impl ConflictDetector {
    pub fn new(set_conflicts: WriteSignal<Vec<Conflict>>) -> Self {
        let worker = ConflictWorker::spawner()
            .encoding::<BincodeCodec>()
            .callback(move |response: ConflictResponse| {
                let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());
                set_conflicts.set(response.conflicts.clone());
                if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
                    web_sys::console::log_1(&format!("Set conflicts signal took {:.2}ms ({} conflicts)",
                        elapsed, response.conflicts.len()).into());
                }
            })
            .spawn("conflict_worker.js");

        Self { worker }
    }

    pub fn detect(&mut self, journeys: Vec<TrainJourney>, graph: RailwayGraph) {
        web_sys::console::log_1(&format!("Sending to worker: {} journeys, {} nodes",
            journeys.len(), graph.graph.node_count()).into());
        let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

        // Build serializable context from graph
        let station_indices = graph.graph.node_indices()
            .enumerate()
            .map(|(idx, node_idx)| (node_idx, idx))
            .collect();
        let context = SerializableConflictContext::from_graph(&graph, station_indices);

        self.worker.send(ConflictRequest { journeys, context });
        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            web_sys::console::log_1(&format!("Worker.send() took {:.2}ms", elapsed).into());
        }
    }
}

/// Creates signals and worker for async conflict detection
pub fn create_conflict_detector() -> (ConflictDetector, ReadSignal<Vec<Conflict>>) {
    let (conflicts, set_conflicts) = create_signal(Vec::new());
    let detector = ConflictDetector::new(set_conflicts);
    (detector, conflicts)
}

pub struct GeoJsonImporter {
    worker: gloo_worker::WorkerBridge<GeoJsonImportWorker>,
}

impl GeoJsonImporter {
    pub fn new<F>(on_complete: F) -> Self
    where
        F: Fn(GeoJsonImportResponse) + 'static,
    {
        let worker = GeoJsonImportWorker::spawner()
            .encoding::<BincodeCodec>()
            .callback(move |response: GeoJsonImportResponse| {
                web_sys::console::log_1(&"GeoJSON import worker completed".into());
                on_complete(response);
            })
            .spawn("geojson_worker.js");

        Self { worker }
    }

    pub fn import(&mut self, request: GeoJsonImportRequest) {
        web_sys::console::log_1(&"Sending GeoJSON import request to worker".into());
        self.worker.send(request);
    }
}
