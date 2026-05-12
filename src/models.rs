// Re-export everything from core
pub use railgraph_core::models::*;

// Re-export frontend-specific keyboard shortcut functions
pub use crate::keyboard_shortcuts_ext::{
    setup_shortcut_handler, setup_single_shortcut_handler,
    is_mac_platform, is_windows_platform, is_input_field_target,
    keyboard_shortcut_from_event,
};
