#![allow(clippy::implicit_hasher)]

pub mod models;
pub mod components;
pub mod storage;
pub mod data;
pub mod constants;
pub mod time;
pub mod conflict;
pub mod train_journey;

#[cfg(target_arch = "wasm32")]
pub mod conflict_worker;
#[cfg(target_arch = "wasm32")]
pub mod worker_bridge;

pub use components::app::App;