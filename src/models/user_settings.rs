use serde::{Deserialize, Serialize};
use super::keyboard_shortcuts::KeyboardShortcuts;

const LOCAL_STORAGE_KEY: &str = "nimby_user_settings";

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

    /// Load user settings from `localStorage`
    ///
    /// # Errors
    ///
    /// Returns an error if the settings cannot be loaded
    pub fn load() -> Result<Self, String> {
        let window = web_sys::window().ok_or("No window")?;
        let storage = window
            .local_storage()
            .map_err(|_| "Failed to access localStorage")?
            .ok_or("localStorage not available")?;

        let Some(json_str) = storage
            .get_item(LOCAL_STORAGE_KEY)
            .map_err(|_| "Failed to read from localStorage")?
        else {
            return Ok(Self::default());
        };

        let mut settings: Self = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse settings: {e}"))?;

        settings.keyboard_shortcuts.merge_with_defaults();

        Ok(settings)
    }

    /// Save user settings to `localStorage`
    ///
    /// # Errors
    ///
    /// Returns an error if the settings cannot be saved
    pub fn save(&self) -> Result<(), String> {
        let window = web_sys::window().ok_or("No window")?;
        let storage = window
            .local_storage()
            .map_err(|_| "Failed to access localStorage")?
            .ok_or("localStorage not available")?;

        let json_str = serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;

        storage
            .set_item(LOCAL_STORAGE_KEY, &json_str)
            .map_err(|_| "Failed to write to localStorage".to_string())
    }
}
