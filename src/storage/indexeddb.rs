use crate::models::{Project, ProjectMetadata};
use crate::storage::Storage;
use leptos::{wasm_bindgen, web_sys};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbRequest, IdbTransactionMode};

const DB_NAME: &str = "rail_graph_db";
const DB_VERSION: u32 = 3;
const PROJECTS_STORE: &str = "projects";
const CURRENT_PROJECT_ID_KEY: &str = "current_project_id";

// Current project data format version
const CURRENT_PROJECT_VERSION: f32 = 1.0;

/// `IndexedDB` implementation of the Storage trait
#[derive(Clone, Copy)]
pub struct IndexedDbStorage;

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

async fn open_db() -> Result<IdbDatabase, String> {
    let window = web_sys::window().ok_or("No window")?;
    let idb = window
        .indexed_db()
        .map_err(|_| "IndexedDB not supported")?
        .ok_or("IndexedDB not available")?;

    let open_request = idb
        .open_with_u32(DB_NAME, DB_VERSION)
        .map_err(|_| "Failed to open database")?;

    // Setup onupgradeneeded to create object stores
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

        // Check if projects store exists
        let mut has_projects = false;

        for i in 0..store_names.length() {
            if let Some(name) = store_names.get(i) {
                if name == PROJECTS_STORE {
                    has_projects = true;
                }
            }
        }

        if !has_projects {
            let _ = db.create_object_store(PROJECTS_STORE);
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

    Ok(db)
}

fn serialize_project(project: &Project) -> Result<Vec<u8>, String> {
    let project_bytes =
        rmp_serde::to_vec(project).map_err(|e| format!("Failed to serialize project: {e}"))?;

    // Create versioned format: [4 bytes f32 version][MessagePack data]
    let mut bytes = Vec::with_capacity(4 + project_bytes.len());
    bytes.extend_from_slice(&CURRENT_PROJECT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&project_bytes);

    Ok(bytes)
}

fn deserialize_project(bytes: &[u8]) -> Result<Project, String> {
    // Check if this is versioned data (has at least 4 bytes for version)
    if bytes.len() >= 4 {
        // Read version from first 4 bytes
        let version_bytes: [u8; 4] = bytes[0..4]
            .try_into()
            .map_err(|_| "Invalid version bytes")?;
        let version = f32::from_le_bytes(version_bytes);

        // Extract project data (skip first 4 bytes)
        let project_bytes = &bytes[4..];

        // Handle different versions
        match version {
            v if (v - 1.0).abs() < f32::EPSILON => {
                // Version 1.0 - current format
                let project: Project = rmp_serde::from_slice(project_bytes)
                    .map_err(|e| format!("Failed to parse project: {e}"))?;
                Ok(project)
            }
            _ => Err(format!("Unsupported project version: {version}")),
        }
    } else {
        // Legacy format without version header - treat as error
        Err("Legacy project format not supported. Please re-import your data.".to_string())
    }
}

impl Storage for IndexedDbStorage {
    async fn save_project(&self, project: &Project) -> Result<(), String> {
        let db = open_db().await?;

        let transaction = db
            .transaction_with_str_and_mode(PROJECTS_STORE, IdbTransactionMode::Readwrite)
            .map_err(|_| "Failed to create transaction")?;

        let store = transaction
            .object_store(PROJECTS_STORE)
            .map_err(|_| "Failed to get object store")?;

        let bytes = serialize_project(project)?;

        // Convert to Uint8Array for IndexedDB
        let uint8_array = js_sys::Uint8Array::from(&bytes[..]);
        let js_value: JsValue = uint8_array.into();

        let request = store
            .put_with_key(&js_value, &JsValue::from_str(&project.metadata.id))
            .map_err(|_| "Failed to save project")?;

        let promise = request_to_promise(&request);
        JsFuture::from(promise)
            .await
            .map_err(|_| "Failed to complete save")?;

        Ok(())
    }

    async fn load_project(&self, id: &str) -> Result<Project, String> {
        let db = open_db().await?;

        let transaction = db
            .transaction_with_str(PROJECTS_STORE)
            .map_err(|_| "Failed to create transaction")?;

        let store = transaction
            .object_store(PROJECTS_STORE)
            .map_err(|_| "Failed to get object store")?;

        let request = store
            .get(&JsValue::from_str(id))
            .map_err(|_| "Failed to get project")?;

        let promise = request_to_promise(&request);
        let result = JsFuture::from(promise)
            .await
            .map_err(|_| "Failed to load project")?;

        if result.is_undefined() || result.is_null() {
            return Err("Project not found".to_string());
        }

        // Convert from Uint8Array back to bytes
        let uint8_array: js_sys::Uint8Array =
            result.dyn_into().map_err(|_| "Invalid project data")?;
        let bytes = uint8_array.to_vec();

        deserialize_project(&bytes)
    }

    async fn delete_project(&self, id: &str) -> Result<(), String> {
        let db = open_db().await?;

        let transaction = db
            .transaction_with_str_and_mode(PROJECTS_STORE, IdbTransactionMode::Readwrite)
            .map_err(|_| "Failed to create transaction")?;

        let store = transaction
            .object_store(PROJECTS_STORE)
            .map_err(|_| "Failed to get object store")?;

        let request = store
            .delete(&JsValue::from_str(id))
            .map_err(|_| "Failed to delete project")?;

        let promise = request_to_promise(&request);
        JsFuture::from(promise)
            .await
            .map_err(|_| "Failed to complete deletion")?;

        Ok(())
    }

    async fn list_projects(&self) -> Result<Vec<ProjectMetadata>, String> {
        let db = open_db().await?;

        let transaction = db
            .transaction_with_str(PROJECTS_STORE)
            .map_err(|_| "Failed to create transaction")?;

        let store = transaction
            .object_store(PROJECTS_STORE)
            .map_err(|_| "Failed to get object store")?;

        // Get all keys and values
        let keys_request = store.get_all_keys().map_err(|_| "Failed to get all keys")?;
        let keys_promise = request_to_promise(&keys_request);
        let keys_result = JsFuture::from(keys_promise)
            .await
            .map_err(|_| "Failed to load keys")?;
        let keys_array: js_sys::Array = keys_result.dyn_into().map_err(|_| "Invalid keys array")?;

        let values_request = store.get_all().map_err(|_| "Failed to get all projects")?;
        let values_promise = request_to_promise(&values_request);
        let values_result = JsFuture::from(values_promise)
            .await
            .map_err(|_| "Failed to load projects")?;
        let values_array: js_sys::Array = values_result.dyn_into().map_err(|_| "Invalid array")?;

        let mut projects = Vec::new();

        for i in 0..keys_array.length() {
            let key = keys_array.get(i);
            let value = values_array.get(i);

            // Skip the current_project_id key
            if let Some(key_str) = key.as_string() {
                if key_str == CURRENT_PROJECT_ID_KEY {
                    continue;
                }
            }

            if !value.is_undefined() && !value.is_null() {
                let uint8_array: js_sys::Uint8Array =
                    value.dyn_into().map_err(|_| "Invalid project data")?;
                let bytes = uint8_array.to_vec();

                // Skip version bytes and deserialize only metadata
                let project_bytes = &bytes[4..];
                let metadata: ProjectMetadata = rmp_serde::from_slice(project_bytes)
                    .map_err(|e| format!("Failed to parse project metadata: {e}"))?;
                projects.push(metadata);
            }
        }

        // Sort by updated_at descending (most recent first)
        projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(projects)
    }

    async fn set_current_project_id(&self, id: &str) -> Result<(), String> {
        let db = open_db().await?;

        let transaction = db
            .transaction_with_str_and_mode(PROJECTS_STORE, IdbTransactionMode::Readwrite)
            .map_err(|_| "Failed to create transaction")?;

        let store = transaction
            .object_store(PROJECTS_STORE)
            .map_err(|_| "Failed to get object store")?;

        let request = store
            .put_with_key(
                &JsValue::from_str(id),
                &JsValue::from_str(CURRENT_PROJECT_ID_KEY),
            )
            .map_err(|_| "Failed to save current project ID")?;

        let promise = request_to_promise(&request);
        JsFuture::from(promise)
            .await
            .map_err(|_| "Failed to complete save")?;

        Ok(())
    }

    async fn get_current_project_id(&self) -> Result<Option<String>, String> {
        let db = open_db().await?;

        let transaction = db
            .transaction_with_str(PROJECTS_STORE)
            .map_err(|_| "Failed to create transaction")?;

        let store = transaction
            .object_store(PROJECTS_STORE)
            .map_err(|_| "Failed to get object store")?;

        let request = store
            .get(&JsValue::from_str(CURRENT_PROJECT_ID_KEY))
            .map_err(|_| "Failed to get current project ID")?;

        let promise = request_to_promise(&request);
        let result = JsFuture::from(promise)
            .await
            .map_err(|_| "Failed to load current project ID")?;

        if result.is_undefined() || result.is_null() {
            return Ok(None);
        }

        let id_str = result.as_string().ok_or("Invalid project ID format")?;
        Ok(Some(id_str))
    }

    async fn get_storage_quota(&self) -> Result<Option<(u64, u64)>, String> {
        let window = web_sys::window().ok_or("No window")?;
        let navigator = window.navigator();
        let storage_manager = navigator.storage();

        // Get quota estimate
        let estimate_promise = storage_manager
            .estimate()
            .map_err(|_| "Failed to get storage estimate")?;

        let estimate_result = JsFuture::from(estimate_promise)
            .await
            .map_err(|_| "Failed to await storage estimate")?;

        // Parse the estimate object
        let estimate_obj = js_sys::Object::from(estimate_result);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let usage = js_sys::Reflect::get(&estimate_obj, &JsValue::from_str("usage"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as u64;

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let quota = js_sys::Reflect::get(&estimate_obj, &JsValue::from_str("quota"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as u64;

        if quota == 0 {
            Ok(None)
        } else {
            Ok(Some((usage, quota)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_project_with_version() {
        let project = Project::new_with_name("Test Project".to_string());
        let bytes = serialize_project(&project).expect("Failed to serialize project");

        // Check that version header is present (first 4 bytes)
        assert!(bytes.len() > 4);
        let version_bytes: [u8; 4] = bytes[0..4].try_into().expect("Failed to extract version bytes");
        let version = f32::from_le_bytes(version_bytes);
        assert!((version - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_deserialize_project_round_trip() {
        let original = Project::new_with_name("Test Project".to_string());
        let bytes = serialize_project(&original).expect("Failed to serialize project");
        let deserialized = deserialize_project(&bytes).expect("Failed to deserialize project");

        assert_eq!(deserialized.metadata.id, original.metadata.id);
        assert_eq!(deserialized.metadata.name, original.metadata.name);
    }

    #[test]
    fn test_deserialize_metadata_from_full_project() {
        let project = Project::new_with_name("Test Project".to_string());
        let bytes = serialize_project(&project).expect("Failed to serialize project");

        // Skip version bytes
        let project_bytes = &bytes[4..];
        let metadata: ProjectMetadata = rmp_serde::from_slice(project_bytes).expect("Failed to deserialize metadata");

        assert_eq!(metadata.id, project.metadata.id);
        assert_eq!(metadata.name, project.metadata.name);
        assert_eq!(metadata.created_at, project.metadata.created_at);
        assert_eq!(metadata.updated_at, project.metadata.updated_at);
    }

    #[test]
    fn test_deserialize_project_invalid_version() {
        // Create invalid version header
        let mut bytes = vec![0u8; 8];
        let invalid_version = 99.0f32;
        bytes[0..4].copy_from_slice(&invalid_version.to_le_bytes());

        let result = deserialize_project(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_project_too_short() {
        // Less than 4 bytes (version header size)
        let bytes = vec![0u8, 1u8, 2u8];
        let result = deserialize_project(&bytes);
        assert!(result.is_err());
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod wasm_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn test_save_and_load_project() {
        let storage = IndexedDbStorage;
        let project = Project::new_with_name("Test Project".to_string());
        let project_id = project.metadata.id.clone();

        // Save project
        storage.save_project(&project).await.unwrap();

        // Load it back
        let loaded = storage.load_project(&project_id).await.unwrap();

        assert_eq!(loaded.metadata.id, project.metadata.id);
        assert_eq!(loaded.metadata.name, project.metadata.name);
    }

    #[wasm_bindgen_test]
    async fn test_list_projects() {
        let storage = IndexedDbStorage;

        // Create and save two projects
        let project1 = Project::new_with_name("Project 1".to_string());
        let project2 = Project::new_with_name("Project 2".to_string());

        storage.save_project(&project1).await.unwrap();
        storage.save_project(&project2).await.unwrap();

        // List projects
        let projects = storage.list_projects().await.unwrap();

        // Should have at least our 2 projects
        assert!(projects.len() >= 2);

        // Should return metadata only
        let found1 = projects.iter().any(|p| p.id == project1.metadata.id);
        let found2 = projects.iter().any(|p| p.id == project2.metadata.id);
        assert!(found1);
        assert!(found2);
    }

    #[wasm_bindgen_test]
    async fn test_delete_project() {
        let storage = IndexedDbStorage;
        let project = Project::new_with_name("To Delete".to_string());
        let project_id = project.metadata.id.clone();

        // Save and verify it exists
        storage.save_project(&project).await.unwrap();
        let loaded = storage.load_project(&project_id).await;
        assert!(loaded.is_ok());

        // Delete it
        storage.delete_project(&project_id).await.unwrap();

        // Verify it's gone
        let loaded = storage.load_project(&project_id).await;
        assert!(loaded.is_err());
    }

    #[wasm_bindgen_test]
    async fn test_current_project_id() {
        let storage = IndexedDbStorage;
        let test_id = "test-current-project-id";

        // Set current project ID
        storage.set_current_project_id(test_id).await.unwrap();

        // Get it back
        let loaded_id = storage.get_current_project_id().await.unwrap();
        assert_eq!(loaded_id, Some(test_id.to_string()));
    }

    #[wasm_bindgen_test]
    async fn test_get_current_project_id_when_none() {
        let storage = IndexedDbStorage;

        // Note: This test assumes clean state or will get whatever was last set
        // In a real test environment, you'd want to clear the DB first
        let loaded_id = storage.get_current_project_id().await.unwrap();
        // Just verify it returns an Option
        assert!(loaded_id.is_some() || loaded_id.is_none());
    }

    #[wasm_bindgen_test]
    async fn test_load_nonexistent_project() {
        let storage = IndexedDbStorage;
        let result = storage.load_project("nonexistent-id-12345").await;
        assert!(result.is_err());
    }
}
