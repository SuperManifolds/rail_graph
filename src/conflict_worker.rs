use gloo_worker::{HandlerId, Worker, WorkerScope, Codec};
use serde::{Deserialize, Serialize};
use crate::conflict::{detect_line_conflicts, Conflict, StationCrossing};
use crate::train_journey::TrainJourney;
use crate::models::RailwayGraph;

#[derive(Serialize, Deserialize)]
pub struct ConflictRequest {
    pub journeys: Vec<TrainJourney>,
    pub graph: RailwayGraph,
}

#[derive(Serialize, Deserialize)]
pub struct ConflictResponse {
    pub conflicts: Vec<Conflict>,
    pub crossings: Vec<StationCrossing>,
}

pub struct MsgPackCodec;

impl Codec for MsgPackCodec {
    fn encode<I: Serialize>(input: I) -> wasm_bindgen::JsValue {
        let bytes = rmp_serde::to_vec(&input).expect("MessagePack encode failed");
        js_sys::Uint8Array::from(&bytes[..]).into()
    }

    fn decode<O: for<'de> Deserialize<'de>>(input: wasm_bindgen::JsValue) -> O {
        let array = js_sys::Uint8Array::new(&input);
        let bytes = array.to_vec();
        rmp_serde::from_slice(&bytes).expect("MessagePack decode failed")
    }
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
        #[cfg(target_arch = "wasm32")]
        {
            use web_sys::console;
            let start = web_sys::window()
                .and_then(|w| w.performance())
                .map(|p| p.now());

            let (conflicts, crossings) = detect_line_conflicts(&msg.journeys, &msg.graph);

            if let Some(start_time) = start {
                if let Some(performance) = web_sys::window().and_then(|w| w.performance()) {
                    let duration = performance.now() - start_time;
                    console::log_1(&format!(
                        "Worker conflict detection: {:.2}ms ({} conflicts, {} crossings)",
                        duration,
                        conflicts.len(),
                        crossings.len()
                    ).into());
                }
            }

            scope.respond(id, ConflictResponse { conflicts, crossings });
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let (conflicts, crossings) = detect_line_conflicts(&msg.journeys, &msg.graph);
            scope.respond(id, ConflictResponse { conflicts, crossings });
        }
    }
}
