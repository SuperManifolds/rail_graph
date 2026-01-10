use crate::conflict::{detect_line_conflicts, Conflict, SerializableConflictContext};
#[allow(unused_imports)]
use crate::logging::log;
use crate::models::Project;
use crate::train_journey::TrainJourney;
use gloo_worker::{HandlerId, Worker, WorkerScope, Codec};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Request for conflict detection using raw project bytes from IndexedDB
/// This avoids re-serializing the project data - we pass the msgpack bytes directly
#[derive(Serialize, Deserialize)]
pub struct ConflictRequest {
    /// Raw msgpack bytes of the project (as stored in IndexedDB)
    pub project_bytes: Vec<u8>,
    /// IDs of visible lines to include in conflict detection
    pub visible_line_ids: Vec<uuid::Uuid>,
    /// Station margin in milliseconds
    pub station_margin_ms: i64,
    /// Minimum separation in milliseconds
    pub minimum_separation_ms: i64,
    /// Whether to ignore same-direction platform conflicts
    pub ignore_same_direction_platform_conflicts: bool,
    /// Optional day filter
    pub day_filter: Option<chrono::Weekday>,
}

#[derive(Serialize, Deserialize)]
pub struct ConflictResponse {
    pub conflicts: Vec<Conflict>,
}

pub struct BincodeCodec;

impl Codec for BincodeCodec {
    fn encode<I: Serialize>(input: I) -> wasm_bindgen::JsValue {
        let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());
        let bytes = bincode::serialize(&input).expect("Bincode encode failed");
        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            log!("Bincode encode took {:.2}ms ({} bytes)", elapsed, bytes.len());
        }
        js_sys::Uint8Array::from(&bytes[..]).into()
    }

    fn decode<O: for<'de> Deserialize<'de>>(input: wasm_bindgen::JsValue) -> O {
        let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());
        let array = js_sys::Uint8Array::new(&input);
        let bytes = array.to_vec();
        let result = bincode::deserialize(&bytes).expect("Bincode decode failed");
        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            log!("Bincode decode took {:.2}ms ({} bytes)", elapsed, bytes.len());
        }
        result
    }
}

fn process_conflict_request(request: ConflictRequest) -> ConflictResponse {
    // Deserialize project from msgpack bytes
    let project = match Project::from_bytes(&request.project_bytes) {
        Ok(p) => p,
        Err(e) => {
            log!("Worker: Failed to deserialize project: {}", e);
            return ConflictResponse { conflicts: vec![] };
        }
    };

    // Filter to only visible lines
    let visible_line_set: HashSet<_> = request.visible_line_ids.iter().collect();
    let visible_lines: Vec<_> = project.lines
        .into_iter()
        .filter(|line| visible_line_set.contains(&line.id))
        .collect();

    // Generate journeys
    let journeys = TrainJourney::generate_journeys(
        &visible_lines,
        &project.graph,
        request.day_filter
    );
    let journeys_vec: Vec<_> = journeys.values().cloned().collect();

    // Build context
    let station_indices = project.graph.graph.node_indices()
        .enumerate()
        .map(|(idx, node_idx)| (node_idx, idx))
        .collect();

    let station_margin = chrono::Duration::milliseconds(request.station_margin_ms);
    let minimum_separation = chrono::Duration::milliseconds(request.minimum_separation_ms);

    let context = SerializableConflictContext::from_graph(
        &project.graph,
        station_indices,
        station_margin,
        minimum_separation,
        request.ignore_same_direction_platform_conflicts,
    );

    // Detect conflicts
    let (conflicts, _) = detect_line_conflicts(&journeys_vec, &context);

    ConflictResponse { conflicts }
}

pub struct ConflictWorker;

impl Worker for ConflictWorker {
    type Input = ConflictRequest;
    type Output = ConflictResponse;
    type Message = ();

    fn create(_scope: &WorkerScope<Self>) -> Self {
        Self
    }

    fn update(&mut self, _scope: &WorkerScope<Self>, _msg: Self::Message) {
        // No internal messages needed
    }

    fn received(&mut self, scope: &WorkerScope<Self>, msg: Self::Input, id: HandlerId) {
        let start = web_sys::window().and_then(|w| w.performance()).map(|p| p.now());

        let response = process_conflict_request(msg);

        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            log!("Worker: Total processing took {:.2}ms ({} conflicts)",
                elapsed, response.conflicts.len());
        }

        scope.respond(id, response);
    }
}
