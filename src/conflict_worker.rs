use crate::conflict::{detect_line_conflicts, Conflict, SerializableConflictContext};
#[allow(unused_imports)]
use crate::logging::log;
use crate::models::Project;
use crate::train_journey::TrainJourney;
use gloo_worker::{HandlerId, Worker, WorkerScope, Codec};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Optional edge filter to restrict conflict detection to lines touching the view
pub type ViewEdgeFilter = Vec<usize>;

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
    /// Optional edge filter - exclude lines not touching these edges or their stations
    pub view_edge_filter: Option<ViewEdgeFilter>,
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

    // Filter to only visible lines, optionally filtering by view edges/stations
    let visible_line_set: HashSet<_> = request.visible_line_ids.iter().collect();
    let visible_lines: Vec<_> = if let Some(view_edges) = &request.view_edge_filter {
        // Build edge set and station set from view edges
        let view_edge_set: HashSet<usize> = view_edges.iter().copied().collect();
        let mut view_station_set: HashSet<petgraph::stable_graph::NodeIndex> = HashSet::new();
        for &edge_idx in view_edges {
            let edge_index = petgraph::stable_graph::EdgeIndex::new(edge_idx);
            if let Some((a, b)) = project.graph.graph.edge_endpoints(edge_index) {
                view_station_set.insert(a);
                view_station_set.insert(b);
            }
        }

        project.lines
            .into_iter()
            .filter(|line| visible_line_set.contains(&line.id))
            .filter(|line| {
                // Include line if route shares any edge with the view
                let shares_edge = line.forward_route.iter().any(|seg| view_edge_set.contains(&seg.edge_index))
                    || line.return_route.iter().any(|seg| view_edge_set.contains(&seg.edge_index));
                if shares_edge {
                    return true;
                }
                // Or if route visits any station in the view (for platform conflicts)
                for seg in line.forward_route.iter().chain(line.return_route.iter()) {
                    let edge_index = petgraph::stable_graph::EdgeIndex::new(seg.edge_index);
                    if let Some((a, b)) = project.graph.graph.edge_endpoints(edge_index) {
                        if view_station_set.contains(&a) || view_station_set.contains(&b) {
                            return true;
                        }
                    }
                }
                false
            })
            .collect()
    } else {
        // No view filter - include all visible lines
        project.lines
            .into_iter()
            .filter(|line| visible_line_set.contains(&line.id))
            .collect()
    };

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
