use crate::conflict::Conflict;
#[allow(unused_imports)]
use crate::logging::log;
use leptos::{SignalSet, WriteSignal};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

/// Invoke a Tauri command with a raw binary body and optional headers.
/// Returns the response as raw bytes.
async fn invoke_binary(cmd: &str, body: &[u8], headers: &[(&str, &str)]) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("No window")?;
    let tauri = js_sys::Reflect::get(&window, &"__TAURI__".into())
        .map_err(|_| "Tauri not available")?;
    let core = js_sys::Reflect::get(&tauri, &"core".into())
        .map_err(|_| "Tauri core not available")?;
    let invoke_fn = js_sys::Reflect::get(&core, &"invoke".into())
        .map_err(|_| "invoke not available")?;
    let invoke_fn: js_sys::Function = invoke_fn.dyn_into().map_err(|_| "invoke is not a function")?;

    let uint8 = js_sys::Uint8Array::from(body);

    // Build headers object
    let headers_obj = js_sys::Object::new();
    for (key, value) in headers {
        js_sys::Reflect::set(&headers_obj, &(*key).into(), &(*value).into())
            .map_err(|_| "Failed to set header")?;
    }

    // Build options: { headers }
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"headers".into(), &headers_obj)
        .map_err(|_| "Failed to set options")?;

    let promise = invoke_fn
        .call3(&core, &cmd.into(), &uint8.into(), &options)
        .map_err(|e| format!("invoke failed: {e:?}"))?;

    let result = JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("invoke promise rejected: {e:?}"))?;

    // Result should be an ArrayBuffer
    let array = js_sys::Uint8Array::new(&result);
    Ok(array.to_vec())
}

/// Invoke a Tauri command with JSON args (for small payloads).
async fn invoke_json(cmd: &str, args: &JsValue) -> Result<JsValue, String> {
    let window = web_sys::window().ok_or("No window")?;
    let tauri = js_sys::Reflect::get(&window, &"__TAURI__".into())
        .map_err(|_| "Tauri not available")?;
    let core = js_sys::Reflect::get(&tauri, &"core".into())
        .map_err(|_| "Tauri core not available")?;
    let invoke_fn = js_sys::Reflect::get(&core, &"invoke".into())
        .map_err(|_| "invoke not available")?;
    let invoke_fn: js_sys::Function = invoke_fn.dyn_into().map_err(|_| "invoke is not a function")?;

    let promise = invoke_fn
        .call2(&core, &cmd.into(), args)
        .map_err(|e| format!("invoke failed: {e:?}"))?;

    JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("invoke promise rejected: {e:?}"))
}

// --- Storage wrappers ---

/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn save_project(bytes: &[u8], id: &str) -> Result<(), String> {
    invoke_binary("save_project", bytes, &[("id", id)]).await?;
    Ok(())
}

/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn load_project(id: &str) -> Result<Vec<u8>, String> {
    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"id".into(), &id.into())
        .map_err(|_| "Failed to set id")?;
    let result = invoke_json("load_project", &args.into()).await?;
    let array = js_sys::Uint8Array::new(&result);
    Ok(array.to_vec())
}

/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn delete_project(id: &str) -> Result<(), String> {
    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"id".into(), &id.into())
        .map_err(|_| "Failed to set id")?;
    invoke_json("delete_project", &args.into()).await?;
    Ok(())
}

/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn list_projects() -> Result<Vec<crate::models::ProjectMetadata>, String> {
    let result = invoke_json("list_projects", &JsValue::UNDEFINED).await?;
    serde_wasm_bindgen::from_value(result).map_err(|e| format!("Failed to deserialize: {e}"))
}

/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn get_current_project_id() -> Result<Option<String>, String> {
    let result = invoke_json("get_current_project_id", &JsValue::UNDEFINED).await?;
    if result.is_null() || result.is_undefined() {
        return Ok(None);
    }
    Ok(result.as_string())
}

/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn set_current_project_id(id: &str) -> Result<(), String> {
    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &"id".into(), &id.into())
        .map_err(|_| "Failed to set id")?;
    invoke_json("set_current_project_id", &args.into()).await?;
    Ok(())
}

// --- Compute wrappers ---

/// # Errors
/// Returns an error if the Tauri command fails or deserialization fails.
pub async fn detect_conflicts_remote(
    project_bytes: &[u8],
    visible_line_ids: &[uuid::Uuid],
    settings: &crate::models::ProjectSettings,
    day_filter: Option<chrono::Weekday>,
    view_edge_filter: Option<&[usize]>,
) -> Result<Vec<Conflict>, String> {
    let line_ids_json =
        serde_json::to_string(visible_line_ids).map_err(|e| format!("JSON error: {e}"))?;

    let mut headers = vec![
        ("visible-line-ids", line_ids_json.as_str()),
    ];

    let margin_str = settings.station_margin.num_milliseconds().to_string();
    headers.push(("station-margin-ms", &margin_str));

    let sep_str = settings.minimum_separation.num_milliseconds().to_string();
    headers.push(("minimum-separation-ms", &sep_str));

    if settings.ignore_same_direction_platform_conflicts {
        headers.push(("ignore-same-dir", "true"));
    }

    let day_str = day_filter.map(|d| format!("{d}"));
    if let Some(ref d) = day_str {
        headers.push(("day-filter", d));
    }

    let edge_str = view_edge_filter.map(|e| serde_json::to_string(e).unwrap_or_default());
    if let Some(ref e) = edge_str {
        headers.push(("view-edge-filter", e));
    }

    let result_bytes = invoke_binary("detect_conflicts", project_bytes, &headers).await?;
    rmp_serde::from_slice(&result_bytes).map_err(|e| format!("Failed to deserialize conflicts: {e}"))
}

// --- Conflict Detector (drop-in replacement for worker_bridge) ---

pub struct ConflictDetector {
    set_conflicts: WriteSignal<Vec<Conflict>>,
    set_is_calculating: WriteSignal<bool>,
}

impl ConflictDetector {
    #[must_use]
    pub fn new(
        set_conflicts: WriteSignal<Vec<Conflict>>,
        set_is_calculating: WriteSignal<bool>,
    ) -> Self {
        Self {
            set_conflicts,
            set_is_calculating,
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn detect(
        &mut self,
        project_bytes: Vec<u8>,
        lines: Vec<crate::models::Line>,
        settings: crate::models::ProjectSettings,
        day_filter: Option<chrono::Weekday>,
        view_edge_filter: Option<Vec<usize>>,
    ) {
        let visible_line_ids: Vec<uuid::Uuid> = lines.iter().map(|l| l.id).collect();

        if visible_line_ids.is_empty() {
            self.set_conflicts.set(vec![]);
            self.set_is_calculating.set(false);
            return;
        }

        self.set_is_calculating.set(true);

        let set_conflicts = self.set_conflicts;
        let set_is_calculating = self.set_is_calculating;

        leptos::spawn_local(async move {
            let result = detect_conflicts_remote(
                &project_bytes,
                &visible_line_ids,
                &settings,
                day_filter,
                view_edge_filter.as_deref(),
            )
            .await;

            match result {
                Ok(conflicts) => {
                    set_conflicts.set(conflicts);
                }
                Err(e) => {
                    log!("Conflict detection error: {}", e);
                    set_conflicts.set(vec![]);
                }
            }
            set_is_calculating.set(false);
        });
    }
}

/// Creates signals and detector for async conflict detection
#[must_use]
pub fn create_conflict_detector() -> (
    ConflictDetector,
    leptos::ReadSignal<Vec<Conflict>>,
    leptos::ReadSignal<bool>,
) {
    let (conflicts, set_conflicts) = leptos::create_signal(Vec::new());
    let (is_calculating, set_is_calculating) = leptos::create_signal(false);
    let detector = ConflictDetector::new(set_conflicts, set_is_calculating);
    (detector, conflicts, is_calculating)
}

/// Edge filter type for view-based line filtering
pub type ViewEdgeFilter = Vec<usize>;

/// Compute auto-layout on the Tauri backend (runs MIP solver natively).
/// Returns a map of node index → (x, y) positions.
///
/// # Errors
/// Returns an error if the Tauri command fails or deserialization fails.
pub async fn compute_auto_layout(
    graph: &crate::models::RailwayGraph,
    geo_hints: Option<&railgraph_core::auto_layout::GeographicHints>,
    settings: &crate::models::ProjectSettings,
    height: f64,
) -> Result<std::collections::HashMap<usize, (f64, f64)>, String> {
    #[derive(serde::Serialize)]
    struct LayoutRequest<'a> {
        graph: &'a crate::models::RailwayGraph,
        geo_hints: Option<&'a railgraph_core::auto_layout::GeographicHints>,
        settings: &'a crate::models::ProjectSettings,
        height: f64,
    }

    let req = LayoutRequest {
        graph,
        geo_hints,
        settings,
        height,
    };

    let req_bytes = rmp_serde::to_vec(&req)
        .map_err(|e| format!("Failed to serialize layout request: {e}"))?;

    let result_bytes = invoke_binary("compute_auto_layout", &req_bytes, &[]).await?;

    rmp_serde::from_slice(&result_bytes)
        .map_err(|e| format!("Failed to deserialize layout positions: {e}"))
}
