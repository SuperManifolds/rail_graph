use crate::models::{Line, Project};
use leptos::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbRequest, IdbTransactionMode};

const LINES_STORAGE_KEY: &str = "nimby_graph_lines";
const DB_NAME: &str = "nimby_graph_db";
const DB_VERSION: u32 = 1;
const PROJECT_STORE: &str = "projects";
const PROJECT_KEY: &str = "current_project";

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = localStorage)]
    fn getItem(key: &str) -> Option<String>;

    #[wasm_bindgen(js_namespace = localStorage)]
    fn setItem(key: &str, value: &str);

    #[wasm_bindgen(js_namespace = localStorage)]
    fn removeItem(key: &str);
}

pub fn save_lines_to_storage(lines: &[Line]) -> Result<(), String> {
    match serde_json::to_string(lines) {
        Ok(json) => {
            setItem(LINES_STORAGE_KEY, &json);
            Ok(())
        }
        Err(e) => Err(format!("Failed to serialize lines: {}", e))
    }
}

pub fn load_lines_from_storage() -> Result<Vec<Line>, String> {
    match getItem(LINES_STORAGE_KEY) {
        Some(json) => {
            match serde_json::from_str(&json) {
                Ok(lines) => Ok(lines),
                Err(e) => Err(format!("Failed to parse stored lines: {}", e))
            }
        }
        None => Err("No saved configuration found".to_string())
    }
}

pub fn clear_lines_storage() {
    removeItem(LINES_STORAGE_KEY);
}

// IndexedDB helper functions
fn request_to_promise(request: &IdbRequest) -> js_sys::Promise {
    let request = request.clone();
    js_sys::Promise::new(&mut |resolve, reject| {
        let reject_clone = reject.clone();
        let onsuccess = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let Some(target) = event.target() else {
                let _ = reject_clone.call1(&JsValue::NULL, &JsValue::from_str("No event target"));
                return;
            };
            let Ok(request) = target.dyn_into::<IdbRequest>() else {
                let _ = reject_clone.call1(&JsValue::NULL, &JsValue::from_str("Invalid request type"));
                return;
            };
            let Ok(result) = request.result() else {
                let _ = reject_clone.call1(&JsValue::NULL, &JsValue::from_str("Failed to get result"));
                return;
            };
            let _ = resolve.call1(&JsValue::NULL, &result);
        }) as Box<dyn FnMut(_)>);

        let onerror = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let _ = reject.call1(&JsValue::NULL, &JsValue::from_str("Request failed"));
        }) as Box<dyn FnMut(_)>);

        request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
        request.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        onsuccess.forget();
        onerror.forget();
    })
}

async fn open_db() -> Result<IdbDatabase, String> {
    let window = web_sys::window().ok_or("No window")?;
    let idb = window
        .indexed_db()
        .map_err(|_| "IndexedDB not supported")?
        .ok_or("IndexedDB not available")?;

    let open_request = idb
        .open_with_u32(DB_NAME, DB_VERSION)
        .map_err(|_| "Failed to open database")?;

    // Setup onupgradeneeded to create object store
    let onupgradeneeded = Closure::wrap(Box::new(move |event: web_sys::IdbVersionChangeEvent| {
        let Some(target) = event.target() else {
            leptos::logging::error!("No event target in onupgradeneeded");
            return;
        };
        let Ok(request) = target.dyn_into::<IdbRequest>() else {
            leptos::logging::error!("Invalid request type in onupgradeneeded");
            return;
        };
        let Ok(result) = request.result() else {
            leptos::logging::error!("Failed to get result in onupgradeneeded");
            return;
        };
        let Ok(db) = result.dyn_into::<IdbDatabase>() else {
            leptos::logging::error!("Failed to cast to IdbDatabase");
            return;
        };

        let store_names = db.object_store_names();
        let mut found = false;
        for i in 0..store_names.length() {
            if let Some(name) = store_names.get(i) {
                if name == PROJECT_STORE {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            let _ = db.create_object_store(PROJECT_STORE);
        }
    }) as Box<dyn FnMut(_)>);

    open_request.set_onupgradeneeded(Some(onupgradeneeded.as_ref().unchecked_ref()));
    onupgradeneeded.forget();

    let promise = request_to_promise(&open_request);
    let db_result = JsFuture::from(promise).await.map_err(|_| "Failed to open database")?;
    let db: IdbDatabase = db_result.dyn_into().map_err(|_| "Invalid database object")?;

    Ok(db)
}

pub async fn save_project_to_storage(project: &Project) -> Result<(), String> {
    let db = open_db().await?;

    let transaction = db
        .transaction_with_str_and_mode(PROJECT_STORE, IdbTransactionMode::Readwrite)
        .map_err(|_| "Failed to create transaction")?;

    let store = transaction
        .object_store(PROJECT_STORE)
        .map_err(|_| "Failed to get object store")?;

    // Serialize to MessagePack binary format
    let bytes = rmp_serde::to_vec(project)
        .map_err(|e| format!("Failed to serialize project: {}", e))?;

    // Convert to Uint8Array for IndexedDB
    let uint8_array = js_sys::Uint8Array::from(&bytes[..]);
    let js_value: JsValue = uint8_array.into();

    let request = store
        .put_with_key(&js_value, &JsValue::from_str(PROJECT_KEY))
        .map_err(|_| "Failed to save project")?;

    let promise = request_to_promise(&request);
    JsFuture::from(promise).await.map_err(|_| "Failed to complete save")?;

    Ok(())
}

pub async fn load_project_from_storage() -> Result<Project, String> {
    let db = open_db().await?;

    let transaction = db
        .transaction_with_str(PROJECT_STORE)
        .map_err(|_| "Failed to create transaction")?;

    let store = transaction
        .object_store(PROJECT_STORE)
        .map_err(|_| "Failed to get object store")?;

    let request = store
        .get(&JsValue::from_str(PROJECT_KEY))
        .map_err(|_| "Failed to get project")?;

    let promise = request_to_promise(&request);
    let result = JsFuture::from(promise).await.map_err(|_| "Failed to load project")?;

    if result.is_undefined() || result.is_null() {
        return Err("No saved project found".to_string());
    }

    // Convert from Uint8Array back to bytes
    let uint8_array: js_sys::Uint8Array = result.dyn_into().map_err(|_| "Invalid project data")?;
    let bytes = uint8_array.to_vec();

    // Deserialize from MessagePack
    let project: Project = rmp_serde::from_slice(&bytes)
        .map_err(|e| format!("Failed to parse project: {}", e))?;

    Ok(project)
}

pub async fn clear_project_storage() -> Result<(), String> {
    let db = open_db().await?;

    let transaction = db
        .transaction_with_str_and_mode(PROJECT_STORE, IdbTransactionMode::Readwrite)
        .map_err(|_| "Failed to create transaction")?;

    let store = transaction
        .object_store(PROJECT_STORE)
        .map_err(|_| "Failed to get object store")?;

    let request = store
        .delete(&JsValue::from_str(PROJECT_KEY))
        .map_err(|_| "Failed to delete project")?;

    let promise = request_to_promise(&request);
    JsFuture::from(promise).await.map_err(|_| "Failed to complete deletion")?;

    Ok(())
}