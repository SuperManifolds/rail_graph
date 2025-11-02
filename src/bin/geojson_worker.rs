#[cfg(target_arch = "wasm32")]
fn main() {
    use nimby_graph::geojson_worker::{GeoJsonImportWorker, BincodeCodec};
    use gloo_worker::Registrable;

    console_error_panic_hook::set_once();
    GeoJsonImportWorker::registrar()
        .encoding::<BincodeCodec>()
        .register();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    panic!("This binary is only for WASM targets");
}
