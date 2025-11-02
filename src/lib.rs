#![allow(clippy::implicit_hasher)]
#![allow(unknown_lints)]
#![allow(clippy::manual_is_multiple_of)]

pub mod models;
pub mod components;
pub mod storage;
pub mod import;
pub mod api;
pub mod constants;
pub mod time;
pub mod geometry;
pub mod conflict;
pub mod train_journey;

#[cfg(target_arch = "wasm32")]
pub mod conflict_worker;

#[cfg(target_arch = "wasm32")]
pub mod geojson_worker;

#[cfg(target_arch = "wasm32")]
#[path = "worker_bridge.rs"]
pub mod worker_bridge;

#[cfg(not(target_arch = "wasm32"))]
#[path = "worker_bridge_sync.rs"]
pub mod worker_bridge;

pub use components::app::App;