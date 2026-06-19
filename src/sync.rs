use crate::tauri_bridge;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Envelope wrapping sync payloads with source window identification.
#[derive(Serialize, Deserialize)]
pub struct SyncEnvelope {
    pub source_window: String,
    pub kind: SyncKind,
}

/// The kind of sync message being broadcast.
#[derive(Serialize, Deserialize)]
pub enum SyncKind {
    /// Tab drag started in a window.
    TabDragStart {
        tab_id: String,
    },
    /// Tab drag cancelled.
    TabDragCancel,
    /// Tab drag ended at screen coordinates (for cross-window hit-testing).
    TabDragEnd {
        tab_id: String,
        screen_x: i32,
        screen_y: i32,
    },
    /// Tab was dropped on a window's tab bar.
    TabDrop {
        tab_id: String,
        target_window: String,
        insert_index: usize,
    },
    /// A window is closing.
    WindowClosing,
    /// Window layout update (tab assignments for this window).
    LayoutUpdate {
        window_id: String,
        tab_ids: Vec<String>,
        active_tab_id: Option<String>,
        bounds: Option<crate::models::WindowBounds>,
    },
}

const SYNC_EVENT: &str = "railgraph-sync";

/// Broadcast a sync message to all windows.
pub fn broadcast(envelope: &SyncEnvelope) {
    let json = match serde_json::to_string(envelope) {
        Ok(j) => j,
        Err(e) => {
            leptos::logging::error!("Failed to serialize sync envelope: {e}");
            return;
        }
    };
    leptos::spawn_local(async move {
        if let Err(e) = tauri_bridge::emit_event(SYNC_EVENT, &json).await {
            leptos::logging::error!("Failed to broadcast sync: {e}");
        }
    });
}

/// Listen for sync messages from other windows.
/// The callback receives the deserialized envelope.
/// Returns the unlisten handle.
///
/// # Errors
/// Returns an error if the listener cannot be registered.
pub async fn listen(
    callback: impl Fn(SyncEnvelope) + 'static,
) -> Result<JsValue, String> {
    tauri_bridge::listen_event(SYNC_EVENT, move |payload| {
        match serde_json::from_str::<SyncEnvelope>(&payload) {
            Ok(envelope) => callback(envelope),
            Err(e) => leptos::logging::error!("Failed to deserialize sync envelope: {e}"),
        }
    })
    .await
}

/// Broadcast a tab drag start event.
pub fn broadcast_tab_drag_start(source_window: &str, tab_id: &str) {
    broadcast(&SyncEnvelope {
        source_window: source_window.to_string(),
        kind: SyncKind::TabDragStart {
            tab_id: tab_id.to_string(),
        },
    });
}

/// Broadcast a tab drag cancel event.
pub fn broadcast_tab_drag_cancel(source_window: &str) {
    broadcast(&SyncEnvelope {
        source_window: source_window.to_string(),
        kind: SyncKind::TabDragCancel,
    });
}

/// Broadcast tab drag end with screen coordinates for cross-window hit-testing.
pub fn broadcast_tab_drag_end(source_window: &str, tab_id: &str, screen_x: i32, screen_y: i32) {
    broadcast(&SyncEnvelope {
        source_window: source_window.to_string(),
        kind: SyncKind::TabDragEnd {
            tab_id: tab_id.to_string(),
            screen_x,
            screen_y,
        },
    });
}

/// Broadcast a tab drop event.
pub fn broadcast_tab_drop(
    source_window: &str,
    tab_id: &str,
    target_window: &str,
    insert_index: usize,
) {
    broadcast(&SyncEnvelope {
        source_window: source_window.to_string(),
        kind: SyncKind::TabDrop {
            tab_id: tab_id.to_string(),
            target_window: target_window.to_string(),
            insert_index,
        },
    });
}

/// Broadcast a window closing event.
pub fn broadcast_window_closing(source_window: &str) {
    broadcast(&SyncEnvelope {
        source_window: source_window.to_string(),
        kind: SyncKind::WindowClosing,
    });
}

/// Broadcast a layout update for this window (async to fetch bounds).
pub fn broadcast_layout_update(
    source_window: &str,
    window_id: &str,
    tab_ids: &[String],
    active_tab_id: Option<&str>,
) {
    let envelope_base = SyncEnvelope {
        source_window: source_window.to_string(),
        kind: SyncKind::LayoutUpdate {
            window_id: window_id.to_string(),
            tab_ids: tab_ids.to_vec(),
            active_tab_id: active_tab_id.map(String::from),
            bounds: None,
        },
    };
    // Fetch bounds asynchronously and broadcast with them
    leptos::spawn_local(async move {
        let bounds = crate::tauri_bridge::get_window_bounds().await;
        let mut envelope = envelope_base;
        if let SyncKind::LayoutUpdate { bounds: ref mut saved_bounds, .. } = envelope.kind {
            *saved_bounds = bounds;
        }
        let json = match serde_json::to_string(&envelope) {
            Ok(j) => j,
            Err(e) => {
                leptos::logging::error!("Failed to serialize sync envelope: {e}");
                return;
            }
        };
        if let Err(e) = crate::tauri_bridge::emit_event(SYNC_EVENT, &json).await {
            leptos::logging::error!("Failed to broadcast sync: {e}");
        }
    });
}
