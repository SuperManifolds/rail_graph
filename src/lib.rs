#![allow(clippy::implicit_hasher)]
#![allow(unknown_lints)]
#![allow(clippy::manual_is_multiple_of)]

pub mod models;
pub mod components;
pub mod storage;
pub mod api;
pub mod theme;
pub mod logging;
pub mod keyboard_shortcuts_ext;
pub mod user_settings_ext;
pub mod time_ext;

pub mod tauri_bridge;

// Re-export core modules
pub use railgraph_core::conflict;
pub use railgraph_core::train_journey;
pub use railgraph_core::geometry;
pub use railgraph_core::constants;
pub use railgraph_core::time;
pub use railgraph_core::import;

pub use components::app::App;
