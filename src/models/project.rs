use serde::{Deserialize, Serialize};
use super::{Line, LineFolder, RailwayGraph, GraphView, ViewportState};
use crate::storage::{CURRENT_PROJECT_VERSION, idb};
use wasm_bindgen::prelude::*;
use chrono::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TrackHandedness {
    #[default]
    RightHand,
    LeftHand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LineSortMode {
    #[default]
    AddedOrder,
    Alphabetical,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSettings {
    #[serde(default)]
    pub track_handedness: TrackHandedness,
    #[serde(default)]
    pub line_sort_mode: LineSortMode,
    #[serde(default = "default_node_distance")]
    pub default_node_distance_grid_squares: f64,
    #[serde(with = "crate::models::line::duration_serde", default = "default_minimum_separation")]
    pub minimum_separation: Duration,
    #[serde(with = "crate::models::line::duration_serde", default = "default_station_margin")]
    pub station_margin: Duration,
    #[serde(default)]
    pub ignore_same_direction_platform_conflicts: bool,
}

fn default_node_distance() -> f64 {
    2.0
}

fn default_minimum_separation() -> Duration {
    Duration::seconds(30)
}

fn default_station_margin() -> Duration {
    Duration::seconds(30)
}

impl Default for ProjectSettings {
    fn default() -> Self {
        Self {
            track_handedness: TrackHandedness::default(),
            line_sort_mode: LineSortMode::default(),
            default_node_distance_grid_squares: default_node_distance(),
            minimum_separation: default_minimum_separation(),
            station_margin: default_station_margin(),
            ignore_same_direction_platform_conflicts: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SpacingMode {
    #[default]
    Equal,
    DistanceBased,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Legend {
    pub show_conflicts: bool,
    pub show_line_blocks: bool,
    #[serde(default)]
    pub spacing_mode: SpacingMode,
}

impl Default for Legend {
    fn default() -> Self {
        Self {
            show_conflicts: true,
            show_line_blocks: false,
            spacing_mode: SpacingMode::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    #[serde(flatten)]
    pub metadata: ProjectMetadata,
    pub lines: Vec<Line>,
    pub graph: RailwayGraph,
    #[serde(default)]
    pub legend: Legend,
    #[serde(default)]
    pub settings: ProjectSettings,
    #[serde(default)]
    pub views: Vec<GraphView>,
    #[serde(default)]
    pub active_tab_id: Option<String>,
    #[serde(default)]
    pub infrastructure_viewport: ViewportState,
    #[serde(default)]
    pub folders: Vec<LineFolder>,
}

impl Project {
    #[must_use]
    pub fn empty() -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Untitled Project".to_string(),
                created_at: now.clone(),
                updated_at: now,
            },
            lines: Vec::new(),
            graph: RailwayGraph::new(),
            legend: Legend::default(),
            settings: ProjectSettings::default(),
            views: Vec::new(),
            active_tab_id: None,
            infrastructure_viewport: ViewportState::default(),
            folders: Vec::new(),
        }
    }

    #[must_use]
    pub fn new(lines: Vec<Line>, graph: RailwayGraph, legend: Legend) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: "Untitled Project".to_string(),
                created_at: now.clone(),
                updated_at: now,
            },
            lines,
            graph,
            legend,
            settings: ProjectSettings::default(),
            views: Vec::new(),
            active_tab_id: None,
            infrastructure_viewport: ViewportState::default(),
            folders: Vec::new(),
        }
    }

    #[must_use]
    pub fn new_with_name(name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name,
                created_at: now.clone(),
                updated_at: now,
            },
            lines: Vec::new(),
            graph: RailwayGraph::new(),
            legend: Legend::default(),
            settings: ProjectSettings::default(),
            views: Vec::new(),
            active_tab_id: None,
            infrastructure_viewport: ViewportState::default(),
            folders: Vec::new(),
        }
    }

    pub fn touch_updated_at(&mut self) {
        self.metadata.updated_at = chrono::Utc::now().to_rfc3339();
    }

    #[must_use]
    pub fn duplicate_with_name(&self, new_name: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            metadata: ProjectMetadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: new_name,
                created_at: now.clone(),
                updated_at: now,
            },
            lines: self.lines.clone(),
            graph: self.graph.clone(),
            legend: self.legend.clone(),
            settings: self.settings.clone(),
            views: self.views.clone(),
            active_tab_id: self.active_tab_id.clone(),
            infrastructure_viewport: self.infrastructure_viewport.clone(),
            folders: self.folders.clone(),
        }
    }
}

// Storage constants
const PROJECTS_STORE: &str = "projects";
const CURRENT_PROJECT_ID_KEY: &str = "current_project_id";

// Project storage implementation
impl Project {
    /// Serialize project to bytes with version header
    fn serialize_to_bytes(&self) -> Result<Vec<u8>, String> {
        let project_bytes =
            rmp_serde::to_vec(self).map_err(|e| format!("Failed to serialize project: {e}"))?;

        // Create versioned format: [4 bytes u32 version][MessagePack data]
        let mut bytes = Vec::with_capacity(4 + project_bytes.len());
        bytes.extend_from_slice(&CURRENT_PROJECT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&project_bytes);

        Ok(bytes)
    }

    /// Deserialize project from bytes with version header
    fn deserialize_from_bytes(bytes: &[u8]) -> Result<Self, String> {
        // Check if this is versioned data (has at least 4 bytes for version)
        if bytes.len() >= 4 {
            // Read version from first 4 bytes
            let version_bytes: [u8; 4] = bytes[0..4]
                .try_into()
                .map_err(|_| "Invalid version bytes".to_string())?;
            let version = u32::from_le_bytes(version_bytes);

            // Extract project data (skip first 4 bytes)
            let project_bytes = &bytes[4..];

            // Handle different versions
            match version {
                1 => {
                    // Version 1 - current format
                    let mut project: Self = rmp_serde::from_slice(project_bytes)
                        .map_err(|e| format!("Failed to parse project: {e}"))?;

                    // Validate and fix any invalid track indices in all lines
                    project.fix_invalid_track_indices();

                    Ok(project)
                }
                _ => Err(format!("Unsupported project version: {version}")),
            }
        } else {
            // Legacy format without version header - treat as error
            Err("Legacy project format not supported. Please re-import your data.".to_string())
        }
    }

    /// Fix invalid track indices in all lines of the project
    /// Only corrects tracks that are out of bounds or have incompatible directions
    pub(crate) fn fix_invalid_track_indices(&mut self) {
        for line in &mut self.lines {
            let fixed_count = line.validate_and_fix_track_indices(&self.graph);
            if fixed_count > 0 {
                #[cfg(target_arch = "wasm32")]
                web_sys::console::log_1(&format!(
                    "Fixed {} invalid track indices in line '{}'",
                    fixed_count, line.name
                ).into());
            }
        }
    }

    /// Save this project to `IndexedDB`
    ///
    /// # Errors
    ///
    /// Returns an error if the project cannot be saved
    pub async fn save_to_db(&self) -> Result<(), String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readwrite(&db, PROJECTS_STORE)?;

        let bytes = self.serialize_to_bytes()?;

        // Convert to Uint8Array for IndexedDB
        let uint8_array = js_sys::Uint8Array::from(&bytes[..]);
        let js_value: JsValue = uint8_array.into();

        idb::put_value(&store, &js_value, &JsValue::from_str(&self.metadata.id)).await?;

        Ok(())
    }

    /// Load a project from `IndexedDB` by ID
    ///
    /// # Errors
    ///
    /// Returns an error if the project cannot be loaded
    pub async fn load_from_db(id: &str) -> Result<Self, String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readonly(&db, PROJECTS_STORE)?;

        let result = idb::get_value(&store, &JsValue::from_str(id)).await?;

        if result.is_undefined() || result.is_null() {
            return Err("Project not found".to_string());
        }

        // Convert from Uint8Array back to bytes
        let uint8_array: js_sys::Uint8Array =
            result.dyn_into().map_err(|_| "Invalid project data".to_string())?;
        let bytes = uint8_array.to_vec();

        Self::deserialize_from_bytes(&bytes)
    }

    /// Delete a project from `IndexedDB` by ID
    ///
    /// # Errors
    ///
    /// Returns an error if the project cannot be deleted
    pub async fn delete_from_db(id: &str) -> Result<(), String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readwrite(&db, PROJECTS_STORE)?;

        idb::delete_value(&store, &JsValue::from_str(id)).await?;

        Ok(())
    }

    /// List all project metadata from `IndexedDB`
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be loaded
    pub async fn list_all_metadata() -> Result<Vec<ProjectMetadata>, String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readonly(&db, PROJECTS_STORE)?;

        // Get all keys and values
        let keys_array = idb::get_all_keys(&store).await?;
        let values_array = idb::get_all_values(&store).await?;

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
                    value.dyn_into().map_err(|_| "Invalid project data".to_string())?;
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

    /// Set the current project ID in `IndexedDB`
    ///
    /// # Errors
    ///
    /// Returns an error if the ID cannot be saved
    pub async fn set_current_id(id: &str) -> Result<(), String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readwrite(&db, PROJECTS_STORE)?;

        idb::put_value(
            &store,
            &JsValue::from_str(id),
            &JsValue::from_str(CURRENT_PROJECT_ID_KEY),
        )
        .await?;

        Ok(())
    }

    /// Get the current project ID from `IndexedDB`
    ///
    /// # Errors
    ///
    /// Returns an error if the ID cannot be loaded
    pub async fn get_current_id() -> Result<Option<String>, String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readonly(&db, PROJECTS_STORE)?;

        let result = idb::get_value(&store, &JsValue::from_str(CURRENT_PROJECT_ID_KEY)).await?;

        if result.is_undefined() || result.is_null() {
            return Ok(None);
        }

        let id_str = result.as_string().ok_or("Invalid project ID format".to_string())?;
        Ok(Some(id_str))
    }

    /// Get storage quota information
    ///
    /// # Errors
    ///
    /// Returns an error if the storage quota cannot be retrieved
    pub async fn get_storage_quota() -> Result<Option<(u64, u64)>, String> {
        let window = web_sys::window().ok_or("No window".to_string())?;
        let navigator = window.navigator();
        let storage_manager = navigator.storage();

        // Get quota estimate
        let estimate_promise = storage_manager
            .estimate()
            .map_err(|_| "Failed to get storage estimate".to_string())?;

        let estimate_result = wasm_bindgen_futures::JsFuture::from(estimate_promise)
            .await
            .map_err(|_| "Failed to await storage estimate".to_string())?;

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
    fn test_project_empty() {
        let project = Project::empty();
        assert_eq!(project.metadata.name, "Untitled Project");
        assert!(project.lines.is_empty());
        assert!(project.views.is_empty());
        assert!(project.active_tab_id.is_none());
    }

    #[test]
    fn test_project_new_with_name() {
        let name = "Test Project".to_string();
        let project = Project::new_with_name(name.clone());
        assert_eq!(project.metadata.name, name);
        assert!(project.lines.is_empty());
        assert!(project.views.is_empty());
    }

    #[test]
    fn test_project_duplicate_with_name() {
        let original = Project::new_with_name("Original".to_string());
        let original_id = original.metadata.id.clone();

        let duplicate = original.duplicate_with_name("Copy".to_string());

        assert_eq!(duplicate.metadata.name, "Copy");
        assert_ne!(duplicate.metadata.id, original_id);
        assert_eq!(duplicate.lines.len(), original.lines.len());
    }

    #[test]
    fn test_touch_updated_at() {
        let mut project = Project::empty();
        let original_updated = project.metadata.updated_at.clone();

        // Sleep a tiny bit to ensure time changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        project.touch_updated_at();
        assert_ne!(project.metadata.updated_at, original_updated);
    }

    #[test]
    fn test_metadata_serialization() {
        let metadata = ProjectMetadata {
            id: "test-id".to_string(),
            name: "Test Project".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            updated_at: "2024-01-02T00:00:00Z".to_string(),
        };

        // Test serialization round-trip
        let serialized = rmp_serde::to_vec(&metadata).expect("Failed to serialize metadata");
        let deserialized: ProjectMetadata = rmp_serde::from_slice(&serialized).expect("Failed to deserialize metadata");

        assert_eq!(deserialized.id, metadata.id);
        assert_eq!(deserialized.name, metadata.name);
        assert_eq!(deserialized.created_at, metadata.created_at);
        assert_eq!(deserialized.updated_at, metadata.updated_at);
    }

    #[test]
    fn test_project_flattened_metadata() {
        let project = Project::new_with_name("Test".to_string());

        // Serialize the full project
        let serialized = rmp_serde::to_vec(&project).expect("Failed to serialize project");

        // Deserialize into just metadata (this tests the flatten optimization)
        let metadata: ProjectMetadata = rmp_serde::from_slice(&serialized).expect("Failed to deserialize metadata from project");

        assert_eq!(metadata.id, project.metadata.id);
        assert_eq!(metadata.name, project.metadata.name);
        assert_eq!(metadata.created_at, project.metadata.created_at);
        assert_eq!(metadata.updated_at, project.metadata.updated_at);
    }

    #[test]
    fn test_project_serialization_round_trip() {
        let original = Project::new_with_name("Round Trip Test".to_string());

        // Serialize
        let serialized = rmp_serde::to_vec(&original).expect("Failed to serialize project");

        // Deserialize
        let deserialized: Project = rmp_serde::from_slice(&serialized).expect("Failed to deserialize project");

        assert_eq!(deserialized.metadata.id, original.metadata.id);
        assert_eq!(deserialized.metadata.name, original.metadata.name);
        assert_eq!(deserialized.metadata.created_at, original.metadata.created_at);
        assert_eq!(deserialized.metadata.updated_at, original.metadata.updated_at);
        assert_eq!(deserialized.lines.len(), original.lines.len());
        assert_eq!(deserialized.views.len(), original.views.len());
    }
}
