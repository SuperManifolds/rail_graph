use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

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
/// - The window object is not available
/// - The HTTP request fails
/// - The response status is not ok
/// - The response body cannot be deserialized
pub async fn fetch_all_releases() -> Result<Vec<ChangelogRelease>, String> {
    let Some(window) = web_sys::window() else {
        return Err("No window".to_string());
    };

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");

    let request = web_sys::Request::new_with_str_and_init(CHANGELOG_API, &opts)
        .map_err(|_| "Failed to create request".to_string())?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|_| "Fetch failed".to_string())?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Invalid response".to_string())?;

    if !resp.ok() {
        return Err(format!("HTTP error: {}", resp.status()));
    }

    let text = JsFuture::from(resp.text().map_err(|_| "Failed to get text".to_string())?)
        .await
        .map_err(|_| "Failed to parse text".to_string())?;

    let text_str = text.as_string().ok_or("Response is not a string")?;

    serde_json::from_str(&text_str)
        .map_err(|e| format!("Failed to deserialize: {e:?}"))
}
