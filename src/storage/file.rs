use crate::models::Project;
use crate::storage::CURRENT_PROJECT_VERSION;
use wasm_bindgen::JsCast;
use web_sys;

/// Serialize a project to bytes with version header
///
/// # Errors
/// Returns an error if `MessagePack` serialization fails
pub fn serialize_project_to_bytes(project: &Project) -> Result<Vec<u8>, String> {
    let project_bytes =
        rmp_serde::to_vec(project).map_err(|e| format!("Failed to serialize project: {e}"))?;

    // Create versioned format: [4 bytes u32 version][`MessagePack` data]
    let mut bytes = Vec::with_capacity(4 + project_bytes.len());
    bytes.extend_from_slice(&CURRENT_PROJECT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&project_bytes);

    Ok(bytes)
}

/// Deserialize a project from bytes with version header validation
///
/// # Errors
/// Returns an error if the file is invalid, version is unsupported, or deserialization fails
pub fn deserialize_project_from_bytes(bytes: &[u8]) -> Result<Project, String> {
    // Validate minimum size
    if bytes.len() < 4 {
        return Err("Invalid .rgproject file: too small".to_string());
    }

    // Validate version header
    let version_bytes: [u8; 4] = bytes[0..4]
        .try_into()
        .map_err(|_| "Invalid version header")?;
    let version = u32::from_le_bytes(version_bytes);

    if version != CURRENT_PROJECT_VERSION {
        return Err(format!("Unsupported project version: {version}"));
    }

    // Deserialize project
    let project_bytes = &bytes[4..];
    let mut project: Project = rmp_serde::from_slice(project_bytes)
        .map_err(|e| format!("Failed to parse project: {e}"))?;

    // Validate and fix any invalid track indices in all lines
    project.fix_invalid_track_indices();

    // Populate missing line codes from line names
    project.populate_missing_line_codes();

    Ok(project)
}

/// Create a download filename for a project
#[must_use]
pub fn create_export_filename(project_name: &str) -> String {
    let now = chrono::Utc::now();
    format!(
        "{}.{}.rgproject",
        project_name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"),
        now.format("%Y-%m-%d-%H%M%S")
    )
}

/// Trigger a browser download of bytes as a file
///
/// # Errors
/// Returns an error if blob creation, URL generation, or DOM manipulation fails
pub fn trigger_download(bytes: &[u8], filename: &str) -> Result<(), String> {
    let uint8_array = js_sys::Uint8Array::from(bytes);
    let array = js_sys::Array::new();
    array.push(&uint8_array);

    let blob_options = web_sys::BlobPropertyBag::new();
    blob_options.set_type("application/octet-stream");

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&array, &blob_options)
        .map_err(|_| "Failed to create blob")?;

    let window = web_sys::window().ok_or("No window available")?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|_| "Failed to create object URL")?;

    let document = window.document().ok_or("No document available")?;
    let anchor = document
        .create_element("a")
        .map_err(|_| "Failed to create anchor element")?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "Failed to cast to anchor element")?;

    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();

    let _ = web_sys::Url::revoke_object_url(&url);

    Ok(())
}

/// Generate a new project with fresh IDs and timestamps
#[must_use]
pub fn regenerate_project_ids(mut project: Project, new_name: Option<String>) -> Project {
    let now = chrono::Utc::now().to_rfc3339();
    project.metadata.id = uuid::Uuid::new_v4().to_string();
    project.metadata.created_at.clone_from(&now);
    project.metadata.updated_at = now;
    if let Some(name) = new_name {
        project.metadata.name = name;
    }
    project
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_round_trip() {
        let project = Project::new_with_name("Test Project".to_string());
        let bytes = serialize_project_to_bytes(&project).expect("Failed to serialize");
        let deserialized = deserialize_project_from_bytes(&bytes).expect("Failed to deserialize");

        assert_eq!(deserialized.metadata.name, project.metadata.name);
    }

    #[test]
    fn test_deserialize_invalid_size() {
        let bytes = vec![0u8, 1u8, 2u8];
        let result = deserialize_project_from_bytes(&bytes);
        assert!(result.is_err());
        assert!(result.expect_err("Expected error").contains("too small"));
    }

    #[test]
    fn test_deserialize_invalid_version() {
        let mut bytes = vec![0u8; 8];
        let invalid_version = 99u32;
        bytes[0..4].copy_from_slice(&invalid_version.to_le_bytes());
        let result = deserialize_project_from_bytes(&bytes);
        assert!(result.is_err());
        assert!(result.expect_err("Expected error").contains("Unsupported project version"));
    }

    #[test]
    fn test_create_export_filename() {
        let filename = create_export_filename("My Project");
        assert!(filename.starts_with("My Project."));
        assert!(filename.ends_with(".rgproject"));
    }

    #[test]
    fn test_create_export_filename_sanitizes_invalid_chars() {
        let filename = create_export_filename("My/Project\\Name:Test");
        assert!(!filename.contains('/'));
        assert!(!filename.contains('\\'));
        assert!(!filename.contains(':'));
        assert!(filename.ends_with(".rgproject"));
    }

    #[test]
    fn test_regenerate_project_ids() {
        let original = Project::new_with_name("Original".to_string());
        let original_id = original.metadata.id.clone();
        let original_created = original.metadata.created_at.clone();

        let prepared = regenerate_project_ids(original.clone(), None);

        assert_ne!(prepared.metadata.id, original_id);
        assert_ne!(prepared.metadata.created_at, original_created);
        assert_eq!(prepared.metadata.name, original.metadata.name);
    }

    #[test]
    fn test_regenerate_project_ids_with_new_name() {
        let original = Project::new_with_name("Original".to_string());
        let new_name = "Imported".to_string();

        let prepared = regenerate_project_ids(original.clone(), Some(new_name.clone()));

        assert_eq!(prepared.metadata.name, new_name);
        assert_ne!(prepared.metadata.name, original.metadata.name);
    }

}
