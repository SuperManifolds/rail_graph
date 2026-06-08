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

// --- Project State Cache (multi-window sync) ---

/// Cache the project state in the Tauri backend for secondary windows to read.
///
/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn cache_project_state(bytes: &[u8]) -> Result<(), String> {
    invoke_binary("cache_project_state", bytes, &[]).await?;
    Ok(())
}

/// Get the cached project state from the Tauri backend.
/// Returns None if no state has been cached yet.
///
/// # Errors
/// Returns an error if the Tauri command fails.
pub async fn get_cached_project_state() -> Result<Vec<u8>, String> {
    let result = invoke_json("get_cached_project_state", &JsValue::UNDEFINED).await?;
    let array = js_sys::Uint8Array::new(&result);
    Ok(array.to_vec())
}

/// Get the current Tauri window's label.
#[must_use]
pub fn get_current_window_label() -> Option<String> {
    let window = web_sys::window()?;
    let tauri = js_sys::Reflect::get(&window, &"__TAURI__".into()).ok()?;
    let ww_module = js_sys::Reflect::get(&tauri, &"webviewWindow".into()).ok()?;
    let get_current = js_sys::Reflect::get(&ww_module, &"getCurrentWebviewWindow".into()).ok()?;
    let get_current: js_sys::Function = get_current.dyn_into().ok()?;
    let current = get_current.call0(&JsValue::NULL).ok()?;
    let label = js_sys::Reflect::get(&current, &"label".into()).ok()?;
    label.as_string()
}

// --- Native Window Management ---

fn get_tauri_module(module: &str) -> Result<JsValue, String> {
    let window = web_sys::window().ok_or("No window")?;
    let tauri = js_sys::Reflect::get(&window, &"__TAURI__".into())
        .map_err(|_| "Tauri not available")?;
    js_sys::Reflect::get(&tauri, &module.into())
        .map_err(|e| format!("Tauri module '{module}' not available: {e:?}"))
}

/// Create a new native Tauri window.
///
/// # Errors
/// Returns an error if window creation fails.
pub async fn create_native_window(
    label: &str,
    url: &str,
    title: &str,
    size: (u32, u32),
) -> Result<(), String> {
    let ww_module = get_tauri_module("webviewWindow")?;
    let ww_class = js_sys::Reflect::get(&ww_module, &"WebviewWindow".into())
        .map_err(|_| "WebviewWindow class not found")?;

    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &"url".into(), &url.into())
        .map_err(|_| "Failed to set url")?;
    js_sys::Reflect::set(&options, &"title".into(), &title.into())
        .map_err(|_| "Failed to set title")?;
    js_sys::Reflect::set(&options, &"width".into(), &JsValue::from_f64(f64::from(size.0)))
        .map_err(|_| "Failed to set width")?;
    js_sys::Reflect::set(&options, &"height".into(), &JsValue::from_f64(f64::from(size.1)))
        .map_err(|_| "Failed to set height")?;
    js_sys::Reflect::set(&options, &"resizable".into(), &JsValue::TRUE)
        .map_err(|_| "Failed to set resizable")?;
    js_sys::Reflect::set(&options, &"center".into(), &JsValue::TRUE)
        .map_err(|_| "Failed to set center")?;

    // new WebviewWindow(label, options)
    let constructor: js_sys::Function = ww_class.dyn_into()
        .map_err(|_| "WebviewWindow is not a constructor")?;
    let args = js_sys::Array::new();
    args.push(&label.into());
    args.push(&options);
    let instance = js_sys::Reflect::construct(&constructor, &args)
        .map_err(|e| format!("Failed to construct WebviewWindow: {e:?}"))?;

    // Wait for tauri://created event
    let once_fn = js_sys::Reflect::get(&instance, &"once".into())
        .map_err(|_| "once method not found")?;
    let once_fn: js_sys::Function = once_fn.dyn_into()
        .map_err(|_| "once is not a function")?;

    let (tx, rx) = futures_channel::oneshot::channel::<Result<(), String>>();
    let tx = std::cell::RefCell::new(Some(tx));

    let on_created = Closure::once(move |_: JsValue| {
        if let Some(tx) = tx.borrow_mut().take() {
            let _ = tx.send(Ok(()));
        }
    });

    once_fn.call2(&instance, &"tauri://created".into(), on_created.as_ref().unchecked_ref())
        .map_err(|e| format!("Failed to register created listener: {e:?}"))?;
    on_created.forget();

    // Also handle error
    let (err_tx, err_rx) = futures_channel::oneshot::channel::<String>();
    let err_tx = std::cell::RefCell::new(Some(err_tx));

    let on_error = Closure::once(move |e: JsValue| {
        if let Some(tx) = err_tx.borrow_mut().take() {
            let _ = tx.send(format!("Window creation error: {e:?}"));
        }
    });

    once_fn.call2(&instance, &"tauri://error".into(), on_error.as_ref().unchecked_ref())
        .map_err(|e| format!("Failed to register error listener: {e:?}"))?;
    on_error.forget();

    // Wait for either created or error
    futures_lite::future::or(
        async { rx.await.unwrap_or(Err("Channel dropped".into())) },
        async { Err(err_rx.await.unwrap_or_else(|_| "Channel dropped".into())) },
    ).await
}

/// Close a native Tauri window by label.
///
/// # Errors
/// Returns an error if the window cannot be found or closed.
pub async fn close_native_window(label: &str) -> Result<(), String> {
    let ww_module = get_tauri_module("webviewWindow")?;
    let ww_class = js_sys::Reflect::get(&ww_module, &"WebviewWindow".into())
        .map_err(|_| "WebviewWindow class not found")?;

    // WebviewWindow.getByLabel(label)
    let get_by_label = js_sys::Reflect::get(&ww_class, &"getByLabel".into())
        .map_err(|_| "getByLabel not found")?;
    let get_by_label: js_sys::Function = get_by_label.dyn_into()
        .map_err(|_| "getByLabel is not a function")?;

    let instance = get_by_label.call1(&ww_class, &label.into())
        .map_err(|e| format!("getByLabel failed: {e:?}"))?;

    if instance.is_null() || instance.is_undefined() {
        return Ok(());
    }

    let close_fn = js_sys::Reflect::get(&instance, &"close".into())
        .map_err(|_| "close method not found")?;
    let close_fn: js_sys::Function = close_fn.dyn_into()
        .map_err(|_| "close is not a function")?;

    let promise = close_fn.call0(&instance)
        .map_err(|e| format!("close() failed: {e:?}"))?;
    JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("close() promise rejected: {e:?}"))?;

    Ok(())
}

/// Emit a Tauri event with a string payload.
///
/// # Errors
/// Returns an error if the event cannot be emitted.
pub async fn emit_event(event_name: &str, payload: &str) -> Result<(), String> {
    let event_module = get_tauri_module("event")?;
    let emit_fn = js_sys::Reflect::get(&event_module, &"emit".into())
        .map_err(|_| "emit not found")?;
    let emit_fn: js_sys::Function = emit_fn.dyn_into()
        .map_err(|_| "emit is not a function")?;

    let promise = emit_fn.call2(&JsValue::NULL, &event_name.into(), &payload.into())
        .map_err(|e| format!("emit failed: {e:?}"))?;
    JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("emit promise rejected: {e:?}"))?;

    Ok(())
}

/// Listen for a Tauri event once. Calls the callback with the payload string.
///
/// # Errors
/// Returns an error if the listener cannot be registered.
pub async fn listen_event_once(
    event_name: &str,
    callback: impl Fn(String) + 'static,
) -> Result<(), String> {
    let event_module = get_tauri_module("event")?;
    let once_fn = js_sys::Reflect::get(&event_module, &"once".into())
        .map_err(|_| "once not found")?;
    let once_fn: js_sys::Function = once_fn.dyn_into()
        .map_err(|_| "once is not a function")?;

    let closure = Closure::wrap(Box::new(move |event: JsValue| {
        let payload = js_sys::Reflect::get(&event, &"payload".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        callback(payload);
    }) as Box<dyn FnMut(JsValue)>);

    let promise = once_fn.call2(&JsValue::NULL, &event_name.into(), closure.as_ref().unchecked_ref())
        .map_err(|e| format!("once failed: {e:?}"))?;
    closure.forget();

    JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("once promise rejected: {e:?}"))?;

    Ok(())
}

/// Listen for a Tauri event continuously. Calls the callback each time.
/// Returns an unlisten function ID (the JS promise resolves to an unlisten callback).
///
/// # Errors
/// Returns an error if the listener cannot be registered.
pub async fn listen_event(
    event_name: &str,
    callback: impl Fn(String) + 'static,
) -> Result<JsValue, String> {
    let event_module = get_tauri_module("event")?;
    let listen_fn = js_sys::Reflect::get(&event_module, &"listen".into())
        .map_err(|_| "listen not found")?;
    let listen_fn: js_sys::Function = listen_fn.dyn_into()
        .map_err(|_| "listen is not a function")?;

    let closure = Closure::wrap(Box::new(move |event: JsValue| {
        let payload = js_sys::Reflect::get(&event, &"payload".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        callback(payload);
    }) as Box<dyn FnMut(JsValue)>);

    let promise = listen_fn.call2(&JsValue::NULL, &event_name.into(), closure.as_ref().unchecked_ref())
        .map_err(|e| format!("listen failed: {e:?}"))?;
    closure.forget();

    let unlisten = JsFuture::from(js_sys::Promise::from(promise))
        .await
        .map_err(|e| format!("listen promise rejected: {e:?}"))?;

    Ok(unlisten)
}

/// Listen for a native window close event.
///
/// # Errors
/// Returns an error if the listener cannot be registered.
pub fn listen_window_close(
    label: &str,
    callback: impl Fn() + 'static,
) -> Result<(), String> {
    let ww_module = get_tauri_module("webviewWindow")?;
    let ww_class = js_sys::Reflect::get(&ww_module, &"WebviewWindow".into())
        .map_err(|_| "WebviewWindow class not found")?;

    let get_by_label = js_sys::Reflect::get(&ww_class, &"getByLabel".into())
        .map_err(|_| "getByLabel not found")?;
    let get_by_label: js_sys::Function = get_by_label.dyn_into()
        .map_err(|_| "getByLabel is not a function")?;

    let instance = get_by_label.call1(&ww_class, &label.into())
        .map_err(|e| format!("getByLabel failed: {e:?}"))?;

    if instance.is_null() || instance.is_undefined() {
        return Err("Window not found".into());
    }

    let once_fn = js_sys::Reflect::get(&instance, &"once".into())
        .map_err(|_| "once not found")?;
    let once_fn: js_sys::Function = once_fn.dyn_into()
        .map_err(|_| "once is not a function")?;

    let closure = Closure::wrap(Box::new(move |_: JsValue| {
        callback();
    }) as Box<dyn FnMut(JsValue)>);

    once_fn.call2(&instance, &"tauri://close-requested".into(), closure.as_ref().unchecked_ref())
        .map_err(|e| format!("once failed: {e:?}"))?;
    closure.forget();

    Ok(())
}
