use crate::conflict::{detect_line_conflicts, Conflict, SerializableConflictContext};
#[allow(unused_imports)]
use crate::logging::log;
use crate::train_journey::TrainJourney;
use gloo_worker::{HandlerId, Worker, WorkerScope, Codec};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ConflictRequest {
    pub journeys: Vec<TrainJourney>,
    pub context: SerializableConflictContext,
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
        let (conflicts, _) = detect_line_conflicts(&msg.journeys, &msg.context);
        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            log!("Worker conflict detection took {:.2}ms ({} conflicts from {} journeys)",
                elapsed, conflicts.len(), msg.journeys.len());
        }
        scope.respond(id, ConflictResponse { conflicts });
    }
}
