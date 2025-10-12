#[cfg(target_arch = "wasm32")]
fn main() {
    use nimby_graph::conflict_worker::{ConflictWorker, MsgPackCodec};
    use gloo_worker::Registrable;

    console_error_panic_hook::set_once();
    ConflictWorker::registrar()
        .encoding::<MsgPackCodec>()
        .register();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    panic!("This binary is only for WASM targets");
}
