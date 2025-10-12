use gloo_worker::Spawnable;
use leptos::{create_signal, ReadSignal, WriteSignal, SignalSet};
use crate::conflict_worker::{ConflictWorker, ConflictRequest, ConflictResponse, MsgPackCodec};
use crate::conflict::{Conflict, StationCrossing};
use crate::train_journey::TrainJourney;
use crate::models::RailwayGraph;

pub struct ConflictDetector {
    worker: gloo_worker::WorkerBridge<ConflictWorker>,
}

impl ConflictDetector {
    pub fn new(
        set_conflicts: WriteSignal<Vec<Conflict>>,
        set_crossings: WriteSignal<Vec<StationCrossing>>,
    ) -> Self {
        let worker = ConflictWorker::spawner()
            .encoding::<MsgPackCodec>()
            .callback(move |response: ConflictResponse| {
                set_conflicts.set(response.conflicts);
                set_crossings.set(response.crossings);
            })
            .spawn("conflict_worker.js");

        Self { worker }
    }

    pub fn detect(&mut self, journeys: Vec<TrainJourney>, graph: RailwayGraph) {
        self.worker.send(ConflictRequest { journeys, graph });
    }
}

/// Creates signals and worker for async conflict detection
pub fn create_conflict_detector() -> (
    ConflictDetector,
    ReadSignal<Vec<Conflict>>,
    ReadSignal<Vec<StationCrossing>>,
) {
    let (conflicts, set_conflicts) = create_signal(Vec::new());
    let (crossings, set_crossings) = create_signal(Vec::new());

    let detector = ConflictDetector::new(set_conflicts, set_crossings);

    (detector, conflicts, crossings)
}
