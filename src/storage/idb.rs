#[allow(unused_imports)]
use crate::logging::log;
use leptos::wasm_bindgen;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbObjectStore, IdbRequest, IdbTransactionMode};
use std::cell::RefCell;

// Database configuration
const DB_NAME: &str = "rail_graph_db";
const DB_VERSION: u32 = 5;
const ALL_STORES: &[&str] = &["projects", "user_settings"];

// Shared database instance
thread_local! {
    static DB_INSTANCE: RefCell<Option<IdbDatabase>> = const { RefCell::new(None) };
}

/// Get `IndexedDB` factory - works in both main thread (Window) and web workers (`WorkerGlobalScope`)
fn get_indexed_db() -> Result<web_sys::IdbFactory, String> {
    // Try window first (main thread)
    if let Some(window) = web_sys::window() {
        return window
            .indexed_db()
            .map_err(|_| "IndexedDB not supported")?
            .ok_or_else(|| "IndexedDB not available".to_string());
    }

    // Fall back to worker global scope
    let global = js_sys::global();
    let worker_scope: web_sys::WorkerGlobalScope = global
        .dyn_into()
        .map_err(|_| "Not running in Window or WorkerGlobalScope")?;

    worker_scope
        .indexed_db()
        .map_err(|_| "IndexedDB not supported in worker")?
        .ok_or_else(|| "IndexedDB not available in worker".to_string())
}

/// Convert an IDB request to a promise
pub fn request_to_promise(request: &IdbRequest) -> js_sys::Promise {
    let request = request.clone();
    js_sys::Promise::new(&mut |resolve, reject| {
        let reject_clone = reject.clone();
        let onsuccess = Closure::wrap(Box::new(move |event: web_sys::Event| {
            let Some(target) = event.target() else {
                let _ = reject_clone.call1(&JsValue::NULL, &JsValue::from_str("No event target"));
                return;
            };
            let Ok(request) = target.dyn_into::<IdbRequest>() else {
                let _ =
                    reject_clone.call1(&JsValue::NULL, &JsValue::from_str("Invalid request type"));
                return;
            };
            let Ok(result) = request.result() else {
                let _ =
                    reject_clone.call1(&JsValue::NULL, &JsValue::from_str("Failed to get result"));
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

/// Get or create the shared database instance with all stores
///
/// # Errors
///
/// Returns an error if the database cannot be opened or if `IndexedDB` is not available
pub async fn get_db() -> Result<IdbDatabase, String> {
    // Check if we already have a connection
    let existing = DB_INSTANCE.with(|db| db.borrow().clone());
    if let Some(db) = existing {
        log!("Using cached database connection");
        // Check if the database connection is still valid
        let store_names = db.object_store_names();
        log!("Database has {} stores", store_names.length());
        return Ok(db);
    }

    log!("Opening new database connection");

    // Open a new connection - works in both main thread and web workers
    let idb = get_indexed_db()?;

    let open_request = idb
        .open_with_u32(DB_NAME, DB_VERSION)
        .map_err(|_| "Failed to open database")?;

    // Setup onupgradeneeded to create object stores
    let stores_to_create: Vec<String> = ALL_STORES.iter().map(|s| (*s).to_string()).collect();
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

        let existing_stores = db.object_store_names();
        let mut existing_set = std::collections::HashSet::new();
        for i in 0..existing_stores.length() {
            if let Some(name) = existing_stores.get(i) {
                existing_set.insert(name);
            }
        }

        // Create any missing stores
        for store_name in &stores_to_create {
            if !existing_set.contains(store_name.as_str()) {
                let _ = db.create_object_store(store_name);
            }
        }
    }) as Box<dyn FnMut(_)>);

    open_request.set_onupgradeneeded(Some(onupgradeneeded.as_ref().unchecked_ref()));
    onupgradeneeded.forget();

    let promise = request_to_promise(&open_request);
    let db_result = JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to open database")?;
    let db: IdbDatabase = db_result
        .dyn_into()
        .map_err(|_| "Invalid database object")?;

    // Store the connection for reuse
    DB_INSTANCE.with(|db_ref| {
        *db_ref.borrow_mut() = Some(db.clone());
    });

    Ok(db)
}

/// Get an object store for reading
///
/// # Errors
///
/// Returns an error if the transaction or object store cannot be created
pub fn get_store_readonly(db: &IdbDatabase, store_name: &str) -> Result<IdbObjectStore, String> {
    let transaction = db
        .transaction_with_str(store_name)
        .map_err(|_| "Failed to create transaction".to_string())?;

    transaction
        .object_store(store_name)
        .map_err(|_| "Failed to get object store".to_string())
}

/// Get an object store for reading and writing
///
/// # Errors
///
/// Returns an error if the transaction or object store cannot be created
pub fn get_store_readwrite(db: &IdbDatabase, store_name: &str) -> Result<IdbObjectStore, String> {
    let transaction = db
        .transaction_with_str_and_mode(store_name, IdbTransactionMode::Readwrite)
        .map_err(|e| {
            let error_msg = format!("Failed to create transaction for store '{store_name}': {e:?}");
            web_sys::console::error_1(&error_msg.clone().into());
            error_msg
        })?;

    transaction
        .object_store(store_name)
        .map_err(|e| {
            let error_msg = format!("Failed to get object store '{store_name}': {e:?}");
            web_sys::console::error_1(&error_msg.clone().into());
            error_msg
        })
}

/// Get a value from a store
///
/// # Errors
///
/// Returns an error if the value cannot be retrieved
pub async fn get_value(store: &IdbObjectStore, key: &JsValue) -> Result<JsValue, String> {
    let request = store.get(key).map_err(|_| "Failed to get value".to_string())?;

    let promise = request_to_promise(&request);
    JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to load value".to_string())
}

/// Put a value into a store
///
/// # Errors
///
/// Returns an error if the value cannot be saved
pub async fn put_value(
    store: &IdbObjectStore,
    value: &JsValue,
    key: &JsValue,
) -> Result<(), String> {
    let request = store
        .put_with_key(value, key)
        .map_err(|_| "Failed to put value".to_string())?;

    let promise = request_to_promise(&request);
    JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to complete save".to_string())?;

    Ok(())
}

/// Delete a value from a store
///
/// # Errors
///
/// Returns an error if the value cannot be deleted
pub async fn delete_value(store: &IdbObjectStore, key: &JsValue) -> Result<(), String> {
    let request = store.delete(key).map_err(|_| "Failed to delete value".to_string())?;

    let promise = request_to_promise(&request);
    JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to complete deletion".to_string())?;

    Ok(())
}

/// Get all keys from a store
///
/// # Errors
///
/// Returns an error if the keys cannot be retrieved
pub async fn get_all_keys(store: &IdbObjectStore) -> Result<js_sys::Array, String> {
    let request = store.get_all_keys().map_err(|_| "Failed to get all keys".to_string())?;
    let promise = request_to_promise(&request);
    let result = JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to load keys".to_string())?;
    result.dyn_into().map_err(|_| "Invalid keys array".to_string())
}

/// Get all values from a store
///
/// # Errors
///
/// Returns an error if the values cannot be retrieved
pub async fn get_all_values(store: &IdbObjectStore) -> Result<js_sys::Array, String> {
    let request = store.get_all().map_err(|_| "Failed to get all values".to_string())?;
    let promise = request_to_promise(&request);
    let result = JsFuture::from(promise)
        .await
        .map_err(|_| "Failed to load values".to_string())?;
    result.dyn_into().map_err(|_| "Invalid values array".to_string())
}
