use serde::{Deserialize, Serialize};
use super::keyboard_shortcuts::KeyboardShortcuts;

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
}
