use crate::conflict::{Conflict, SerializableConflictContext};
use crate::conflict_worker::{ConflictWorker, ConflictRequest, ConflictResponse, BincodeCodec};
#[allow(unused_imports)]
use crate::logging::log;
use crate::models::{RailwayGraph, ProjectSettings};
use crate::train_journey::TrainJourney;
use gloo_worker::Spawnable;
use leptos::{create_signal, ReadSignal, WriteSignal, SignalSet};

pub struct ConflictDetector {
    worker: Option<gloo_worker::WorkerBridge<ConflictWorker>>,
    set_conflicts: WriteSignal<Vec<Conflict>>,
    set_is_calculating: WriteSignal<bool>,
}

impl ConflictDetector {
    pub fn new(set_conflicts: WriteSignal<Vec<Conflict>>, set_is_calculating: WriteSignal<bool>) -> Self {
        Self {
            worker: None,
            set_conflicts,
            set_is_calculating,
        }
    }

    /// Spawns a fresh worker, terminating any existing one first.
    /// This cancels any in-flight calculation.
    fn spawn_worker(&mut self) -> &mut gloo_worker::WorkerBridge<ConflictWorker> {
        let set_conflicts = self.set_conflicts;
        let set_is_calculating = self.set_is_calculating;
        self.worker = Some(
            ConflictWorker::spawner()
                .encoding::<BincodeCodec>()
                .callback(move |response: ConflictResponse| {
                    let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());
                    set_conflicts.set(response.conflicts.clone());
                    set_is_calculating.set(false);
                    if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
                        log!("Set conflicts signal took {:.2}ms ({} conflicts)",
                            elapsed, response.conflicts.len());
                    }
                })
                .spawn("conflict_worker.js")
        );
        self.worker.as_mut().expect("worker should be Some after spawn")
    }

    pub fn detect(&mut self, journeys: Vec<TrainJourney>, graph: RailwayGraph, settings: ProjectSettings) {
        self.set_is_calculating.set(true);
        log!("Sending to worker: {} journeys, {} nodes",
            journeys.len(), graph.graph.node_count());
        let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

        // Terminate any existing worker (cancels in-flight calculation)
        self.worker = None;

        // Spawn fresh worker for this request
        let worker = self.spawn_worker();

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

        worker.send(ConflictRequest { journeys, context });
        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            log!("Worker.send() took {:.2}ms", elapsed);
        }
    }
}

/// Creates signals and worker for async conflict detection
pub fn create_conflict_detector() -> (ConflictDetector, ReadSignal<Vec<Conflict>>, ReadSignal<bool>) {
    let (conflicts, set_conflicts) = create_signal(Vec::new());
    let (is_calculating, set_is_calculating) = create_signal(false);
    let detector = ConflictDetector::new(set_conflicts, set_is_calculating);
    (detector, conflicts, is_calculating)
}
