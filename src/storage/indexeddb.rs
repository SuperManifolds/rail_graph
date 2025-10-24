use crate::models::{Project, ProjectMetadata};
use crate::storage::Storage;

/// `IndexedDB` implementation of the Storage trait
/// This is now just a thin wrapper around Project's storage methods
#[derive(Clone, Copy)]
pub struct IndexedDbStorage;

impl Storage for IndexedDbStorage {
    async fn save_project(&self, project: &Project) -> Result<(), String> {
        project.save_to_db().await
    }

    async fn load_project(&self, id: &str) -> Result<Project, String> {
        Project::load_from_db(id).await
    }

    async fn delete_project(&self, id: &str) -> Result<(), String> {
        Project::delete_from_db(id).await
    }

    async fn list_projects(&self) -> Result<Vec<ProjectMetadata>, String> {
        Project::list_all_metadata().await
    }

    async fn set_current_project_id(&self, id: &str) -> Result<(), String> {
        Project::set_current_id(id).await
    }

    async fn get_current_project_id(&self) -> Result<Option<String>, String> {
        Project::get_current_id().await
    }

    async fn get_storage_quota(&self) -> Result<Option<(u64, u64)>, String> {
        Project::get_storage_quota().await
    }
}
