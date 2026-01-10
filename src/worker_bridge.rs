use crate::conflict::Conflict;
use crate::conflict_worker::{ConflictWorker, ConflictRequest, ConflictResponse, BincodeCodec};
#[allow(unused_imports)]
use crate::logging::log;
use crate::models::{Line, ProjectSettings};
use gloo_worker::Spawnable;
use leptos::{create_signal, ReadSignal, WriteSignal, SignalSet};

pub struct ConflictDetector {
    worker: Option<gloo_worker::WorkerBridge<ConflictWorker>>,
    set_conflicts: WriteSignal<Vec<Conflict>>,
    set_is_calculating: WriteSignal<bool>,
}

impl ConflictDetector {
    #[must_use]
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

    /// Detect conflicts by sending raw project bytes to the worker.
    /// The worker deserializes and generates journeys itself.
    #[allow(clippy::needless_pass_by_value)]
    pub fn detect(
        &mut self,
        project_bytes: Vec<u8>,
        lines: Vec<Line>,
        settings: ProjectSettings,
        day_filter: Option<chrono::Weekday>,
    ) {
        // Extract visible line IDs
        let visible_line_ids: Vec<uuid::Uuid> = lines.iter().map(|l| l.id).collect();

        // Skip if no lines to check
        if visible_line_ids.is_empty() {
            self.set_conflicts.set(vec![]);
            self.set_is_calculating.set(false);
            return;
        }

        self.set_is_calculating.set(true);
        log!("Sending to worker: {} bytes, {} visible lines",
            project_bytes.len(), visible_line_ids.len());
        let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

        // Terminate any existing worker (cancels in-flight calculation)
        self.worker = None;

        // Spawn fresh worker for this request
        let worker = self.spawn_worker();

        // Send request with raw project bytes
        let request = ConflictRequest {
            project_bytes,
            visible_line_ids,
            station_margin_ms: settings.station_margin.num_milliseconds(),
            minimum_separation_ms: settings.minimum_separation.num_milliseconds(),
            ignore_same_direction_platform_conflicts: settings.ignore_same_direction_platform_conflicts,
            day_filter,
        };

        worker.send(request);
        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            log!("Worker.send() took {:.2}ms", elapsed);
        }
    }
}

/// Creates signals and worker for async conflict detection
#[must_use]
pub fn create_conflict_detector() -> (ConflictDetector, ReadSignal<Vec<Conflict>>, ReadSignal<bool>) {
    let (conflicts, set_conflicts) = create_signal(Vec::new());
    let (is_calculating, set_is_calculating) = create_signal(false);
    let detector = ConflictDetector::new(set_conflicts, set_is_calculating);
    (detector, conflicts, is_calculating)
}
