mod indexeddb;

pub use indexeddb::IndexedDbStorage;

use crate::models::{Project, ProjectMetadata};

const GB: f64 = 1_073_741_824.0;
const MB: f64 = 1_048_576.0;
const KB: f64 = 1_024.0;

/// Format bytes into a human-readable string with appropriate units
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let bytes_f = bytes as f64;

    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{bytes} B")
    }
}

/// Storage trait for project persistence
#[allow(async_fn_in_trait)]
pub trait Storage {
    /// Save a project by its ID
    async fn save_project(&self, project: &Project) -> Result<(), String>;

    /// Load a specific project by ID
    async fn load_project(&self, id: &str) -> Result<Project, String>;

    /// Delete a project by ID
    async fn delete_project(&self, id: &str) -> Result<(), String>;

    /// List all saved projects (returns only metadata, not full projects)
    async fn list_projects(&self) -> Result<Vec<ProjectMetadata>, String>;

    /// Set the current project ID (last used project for auto-load)
    async fn set_current_project_id(&self, id: &str) -> Result<(), String>;

    /// Get the current project ID (last used project)
    async fn get_current_project_id(&self) -> Result<Option<String>, String>;

    /// Get storage quota information if available
    /// Returns None if the storage backend doesn't support quota checks
    /// Returns (`used_bytes`, `total_bytes`) tuple if quota is available
    async fn get_storage_quota(&self) -> Result<Option<(u64, u64)>, String> {
        Ok(None)
    }
}
