use railgraph_core::models::UserSettings;
use crate::keyboard_shortcuts_ext;

const LOCAL_STORAGE_KEY: &str = "nimby_user_settings";

/// Extension trait for `UserSettings` to add frontend-specific load/save functionality
pub trait UserSettingsStorage {
    /// Load user settings from `localStorage`
    ///
    /// # Errors
    ///
    /// Returns an error if the settings cannot be loaded
    fn load() -> Result<UserSettings, String>;

    /// Save user settings to `localStorage`
    ///
    /// # Errors
    ///
    /// Returns an error if the settings cannot be saved
    fn save(&self) -> Result<(), String>;
}

impl UserSettingsStorage for UserSettings {
    fn load() -> Result<UserSettings, String> {
        let window = web_sys::window().ok_or("No window")?;
        let storage = window
            .local_storage()
            .map_err(|_| "Failed to access localStorage")?
            .ok_or("localStorage not available")?;

        let Some(json_str) = storage
            .get_item(LOCAL_STORAGE_KEY)
            .map_err(|_| "Failed to read from localStorage")?
        else {
            return Ok(UserSettings::default());
        };

        let mut settings: UserSettings = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse settings: {e}"))?;

        keyboard_shortcuts_ext::merge_with_defaults(&mut settings.keyboard_shortcuts);

        Ok(settings)
    }

    fn save(&self) -> Result<(), String> {
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
