use serde::{Deserialize, Serialize};
use super::keyboard_shortcuts::KeyboardShortcuts;
use crate::storage::idb;
use wasm_bindgen::JsValue;

const USER_SETTINGS_STORE: &str = "user_settings";
const USER_SETTINGS_KEY: &str = "settings";

/// User settings that persist across projects
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct UserSettings {
    #[serde(default)]
    pub keyboard_shortcuts: KeyboardShortcuts,
}

impl UserSettings {
    /// Create new settings with defaults
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load user settings from `IndexedDB`
    ///
    /// # Errors
    ///
    /// Returns an error if the settings cannot be loaded
    pub async fn load() -> Result<Self, String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readonly(&db, USER_SETTINGS_STORE)?;

        let result = idb::get_value(&store, &JsValue::from_str(USER_SETTINGS_KEY)).await?;

        if result.is_undefined() || result.is_null() {
            // No settings found, return defaults
            return Ok(Self::default());
        }

        // Parse JSON
        let json_str = result.as_string().ok_or("Invalid settings format")?;

        let mut settings: Self = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse settings: {e}"))?;

        // Merge in any new shortcuts that were added since the settings were last saved
        settings.keyboard_shortcuts.merge_with_defaults();

        Ok(settings)
    }

    /// Save user settings to `IndexedDB`
    ///
    /// # Errors
    ///
    /// Returns an error if the settings cannot be saved
    pub async fn save(&self) -> Result<(), String> {
        let db = idb::get_db().await?;
        let store = idb::get_store_readwrite(&db, USER_SETTINGS_STORE)?;

        // Serialize to JSON
        let json_str = serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;

        idb::put_value(
            &store,
            &JsValue::from_str(&json_str),
            &JsValue::from_str(USER_SETTINGS_KEY),
        )
        .await?;

        Ok(())
    }
}
