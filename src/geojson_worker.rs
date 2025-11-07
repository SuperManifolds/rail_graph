use gloo_worker::{HandlerId, Worker, WorkerScope};

use crate::import::geojson::{GeoJsonImportRequest, GeoJsonImportResponse};

// Re-export BincodeCodec from conflict_worker for consistency
pub use crate::conflict_worker::BincodeCodec;

pub struct GeoJsonImportWorker;

impl Worker for GeoJsonImportWorker {
    type Input = GeoJsonImportRequest;
    type Output = GeoJsonImportResponse;
    type Message = ();

    fn create(_scope: &WorkerScope<Self>) -> Self {
        web_sys::console::log_1(&"GeoJSON import worker created".into());
        Self
    }

    fn update(&mut self, _scope: &WorkerScope<Self>, _msg: Self::Message) {
        // No internal messages needed
    }

    fn received(&mut self, scope: &WorkerScope<Self>, msg: Self::Input, id: HandlerId) {
        web_sys::console::log_1(&"Worker received import request".into());
        let start = web_sys::window()
            .and_then(|w| w.performance())
            .map(|p| p.now());

        // Call core import logic (independent of worker vs sync)
        let response = crate::import::geojson::import_geojson_to_updates(&msg);

        if let Some(elapsed) = start.and_then(|s| web_sys::window()?.performance().map(|p| p.now() - s)) {
            web_sys::console::log_1(
                &format!(
                    "Worker import took {:.2}ms ({} stations, {} edges)",
                    elapsed, response.stations_added, response.edges_added
                )
                .into(),
            );
        }

        scope.respond(id, response);
    }
}
