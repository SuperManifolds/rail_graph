#![allow(clippy::implicit_hasher)]

pub mod models;
pub mod components;
pub mod storage;
pub mod data;
pub mod constants;
pub mod time;
pub mod conflict;
pub mod train_journey;
pub mod jtraingraph;

#[cfg(target_arch = "wasm32")]
pub mod conflict_worker;

#[cfg(target_arch = "wasm32")]
#[path = "worker_bridge.rs"]
pub mod worker_bridge;

#[cfg(not(target_arch = "wasm32"))]
#[path = "worker_bridge_sync.rs"]
pub mod worker_bridge;

pub use components::app::App;