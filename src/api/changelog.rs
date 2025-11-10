use serde::{Deserialize, Serialize};

const CHANGELOG_API: &str = "/api/changelog";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogRelease {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: String,
}

/// Fetch all changelog releases from the API
///
/// # Errors
///
/// Returns an error if:
/// - The HTTP request fails
/// - The response status is not ok
/// - The response body cannot be deserialized
pub async fn fetch_all_releases() -> Result<Vec<ChangelogRelease>, String> {
    reqwest::get(CHANGELOG_API)
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<Vec<ChangelogRelease>>()
        .await
        .map_err(|e| format!("Failed to deserialize: {e}"))
}
