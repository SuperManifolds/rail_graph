use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use leptos::SignalGet;
use wasm_bindgen::JsCast;

/// Keyboard shortcut definition
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct KeyboardShortcut {
    /// The key code (e.g., `"KeyW"`, `"Equal"`, `"Space"`)
    pub code: String,
    /// Whether Ctrl (or Cmd on Mac) is required
    pub ctrl: bool,
    /// Whether Shift is required
    pub shift: bool,
    /// Whether Alt is required
    pub alt: bool,
    /// Whether Meta (Cmd on Mac) is required
    pub meta: bool,
}

impl KeyboardShortcut {
    /// Create a new keyboard shortcut
    #[must_use]
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn new(code: String, ctrl: bool, shift: bool, alt: bool, meta: bool) -> Self {
        Self {
            code,
            ctrl,
            shift,
            alt,
            meta,
        }
    }

    /// Create a shortcut with just a key code (no modifiers)
    #[must_use]
    pub fn key_only(code: &str) -> Self {
        Self::new(code.to_string(), false, false, false, false)
    }

    /// Create a shortcut with Ctrl + Shift + key
    #[must_use]
    pub fn ctrl_shift(code: &str) -> Self {
        Self::new(code.to_string(), true, true, false, false)
    }

    /// Create a shortcut with Cmd (Meta) + Shift + key
    #[must_use]
    pub fn meta_shift(code: &str) -> Self {
        Self::new(code.to_string(), false, true, false, true)
    }

    /// Format the shortcut for display
    #[must_use]
    pub fn format(&self, is_mac: bool, is_windows: bool) -> String {
        let mut parts = Vec::new();

        let (ctrl_name, alt_name, shift_name, meta_name, separator) = match (is_mac, is_windows) {
            (true, _) => ("⌃", "⌥", "⇧", "⌘", ""),
            (false, true) => ("Ctrl", "Alt", "Shift", "Win", "+"),
            (false, false) => ("Ctrl", "Alt", "Shift", "Meta", "+"),
        };

        if self.meta {
            parts.push(meta_name);
        }
        if self.ctrl {
            parts.push(ctrl_name);
        }
        if self.alt {
            parts.push(alt_name);
        }
        if self.shift {
            parts.push(shift_name);
        }

        // Convert key code to display name
        let key_name = self.code_to_display_name();
        parts.push(&key_name);

        parts.join(separator)
    }

    /// Convert a key code to a display name
    fn code_to_display_name(&self) -> String {
        match self.code.as_str() {
            "Space" => "Space".to_string(),
            "Equal" => "=".to_string(),
            "Minus" => "-".to_string(),
            "Comma" => ",".to_string(),
            "Period" => ".".to_string(),
            "Semicolon" => ";".to_string(),
            "Quote" => "'".to_string(),
            "Backquote" => "`".to_string(),
            "Slash" => "/".to_string(),
            "Backslash" => "\\".to_string(),
            "BracketLeft" => "[".to_string(),
            "BracketRight" => "]".to_string(),
            "ArrowUp" => "↑".to_string(),
            "ArrowDown" => "↓".to_string(),
            "ArrowLeft" => "←".to_string(),
            "ArrowRight" => "→".to_string(),
            "NumpadAdd" => "Numpad+".to_string(),
            "NumpadSubtract" => "Numpad-".to_string(),
            "NumpadMultiply" => "Numpad*".to_string(),
            "NumpadDivide" => "Numpad/".to_string(),
            "NumpadDecimal" => "Numpad.".to_string(),
            code if code.starts_with("Key") => code[3..].to_string(),
            code if code.starts_with("Digit") => code[5..].to_string(),
            _ => self.code.clone(),
        }
    }

    /// Check if this shortcut conflicts with another
    #[must_use]
    pub fn conflicts_with(&self, other: &KeyboardShortcut) -> bool {
        self == other
    }

    /// Check if this shortcut is likely to conflict with common browser/OS shortcuts
    #[must_use]
    pub fn is_likely_browser_conflict(&self) -> bool {
        // Common browser shortcuts that should be warned about
        let conflicts = [
            // Ctrl/Cmd + single key
            ("KeyT", true, false, false), // New tab
            ("KeyW", true, false, false), // Close tab
            ("KeyN", true, false, false), // New window
            ("KeyR", true, false, false), // Reload
            ("KeyF", true, false, false), // Find
            ("KeyP", true, false, false), // Print
            ("KeyS", true, false, false), // Save
            ("KeyO", true, false, false), // Open
            ("KeyA", true, false, false), // Select all
            ("KeyC", true, false, false), // Copy
            ("KeyV", true, false, false), // Paste
            ("KeyX", true, false, false), // Cut
            ("KeyZ", true, false, false), // Undo
            ("KeyY", true, false, false), // Redo
        ];

        for (code, ctrl, shift, alt) in conflicts {
            if self.code == code
                && ctrl && (self.ctrl || self.meta)
                && self.shift == shift
                && self.alt == alt
            {
                return true;
            }
        }

        false
    }
}

/// Shortcut category
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShortcutCategory {
    Navigation,
    Infrastructure,
    Project,
}

impl ShortcutCategory {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            ShortcutCategory::Navigation => "Navigation",
            ShortcutCategory::Infrastructure => "Infrastructure",
            ShortcutCategory::Project => "Project",
        }
    }
}

/// Shortcut entry with metadata
pub struct ShortcutEntry {
    pub id: &'static str,
    pub description: &'static str,
    pub category: ShortcutCategory,
    pub default_shortcut: KeyboardShortcut,
}

/// Detect if running on Mac platform
fn is_mac_platform() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(platform) = window.navigator().platform() else {
        return false;
    };
    platform.contains("Mac") || platform.contains("iPhone") || platform.contains("iPad")
}

/// Get all shortcut definitions
/// TO ADD A NEW SHORTCUT: Just add one entry to this function!
fn get_all_shortcut_definitions() -> Vec<ShortcutEntry> {
    let is_mac = is_mac_platform();
    let primary_shift = if is_mac {
        KeyboardShortcut::meta_shift
    } else {
        KeyboardShortcut::ctrl_shift
    };

    vec![
        // Navigation
        ShortcutEntry {
            id: "pan_up",
            description: "Pan Up",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("KeyW"),
        },
        ShortcutEntry {
            id: "pan_left",
            description: "Pan Left",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("KeyA"),
        },
        ShortcutEntry {
            id: "pan_down",
            description: "Pan Down",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("KeyS"),
        },
        ShortcutEntry {
            id: "pan_right",
            description: "Pan Right",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("KeyD"),
        },
        ShortcutEntry {
            id: "pan_toggle",
            description: "Toggle Pan Mode",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("Space"),
        },
        ShortcutEntry {
            id: "zoom_in",
            description: "Zoom In",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("Equal"),
        },
        ShortcutEntry {
            id: "zoom_out",
            description: "Zoom Out",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("Minus"),
        },
        ShortcutEntry {
            id: "horizontal_scale_increase",
            description: "Horizontal Scale Increase",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("BracketRight"),
        },
        ShortcutEntry {
            id: "horizontal_scale_decrease",
            description: "Horizontal Scale Decrease",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("BracketLeft"),
        },
        ShortcutEntry {
            id: "reset_view",
            description: "Reset View",
            category: ShortcutCategory::Navigation,
            default_shortcut: KeyboardShortcut::key_only("KeyR"),
        },
        // Infrastructure
        ShortcutEntry {
            id: "add_station",
            description: "Add Station",
            category: ShortcutCategory::Infrastructure,
            default_shortcut: primary_shift("KeyS"),
        },
        ShortcutEntry {
            id: "add_track",
            description: "Add Track",
            category: ShortcutCategory::Infrastructure,
            default_shortcut: primary_shift("KeyT"),
        },
        ShortcutEntry {
            id: "add_junction",
            description: "Add Junction",
            category: ShortcutCategory::Infrastructure,
            default_shortcut: primary_shift("KeyJ"),
        },
        ShortcutEntry {
            id: "create_view",
            description: "Create View",
            category: ShortcutCategory::Infrastructure,
            default_shortcut: primary_shift("KeyN"),
        },
        // Project
        ShortcutEntry {
            id: "manage_projects",
            description: "Manage Projects",
            category: ShortcutCategory::Project,
            default_shortcut: primary_shift("KeyM"),
        },
        ShortcutEntry {
            id: "create_line",
            description: "Create Line",
            category: ShortcutCategory::Project,
            default_shortcut: primary_shift("KeyL"),
        },
        ShortcutEntry {
            id: "import_data",
            description: "Import Data",
            category: ShortcutCategory::Project,
            default_shortcut: primary_shift("KeyO"),
        },
        ShortcutEntry {
            id: "open_settings",
            description: "Open Settings",
            category: ShortcutCategory::Project,
            default_shortcut: KeyboardShortcut::new("Comma".to_string(), true, false, false, false),
        },
        ShortcutEntry {
            id: "undo",
            description: "Undo",
            category: ShortcutCategory::Project,
            default_shortcut: if is_mac {
                KeyboardShortcut::new("KeyZ".to_string(), false, false, false, true)
            } else {
                KeyboardShortcut::new("KeyZ".to_string(), true, false, false, false)
            },
        },
        ShortcutEntry {
            id: "redo",
            description: "Redo",
            category: ShortcutCategory::Project,
            default_shortcut: if is_mac {
                KeyboardShortcut::new("KeyZ".to_string(), false, true, false, true)
            } else {
                KeyboardShortcut::new("KeyY".to_string(), true, false, false, false)
            },
        },
    ]
}

/// Shortcut metadata (description and category)
#[derive(Debug, Clone)]
pub struct ShortcutMetadata {
    pub description: &'static str,
    pub category: ShortcutCategory,
}

/// All available keyboard shortcuts
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KeyboardShortcuts {
    pub shortcuts: HashMap<String, Option<KeyboardShortcut>>,
    #[serde(skip)]
    index: HashMap<KeyboardShortcut, String>,
}

// Custom Deserialize implementation to rebuild index after deserialization
impl<'de> Deserialize<'de> for KeyboardShortcuts {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct KeyboardShortcutsHelper {
            shortcuts: HashMap<String, Option<KeyboardShortcut>>,
        }

        let helper = KeyboardShortcutsHelper::deserialize(deserializer)?;
        let mut instance = KeyboardShortcuts {
            shortcuts: helper.shortcuts,
            index: HashMap::new(),
        };
        instance.rebuild_index();
        Ok(instance)
    }
}

impl KeyboardShortcuts {
    /// Get the default keyboard shortcuts
    #[must_use]
    pub fn default_shortcuts() -> Self {
        let shortcuts = get_all_shortcut_definitions()
            .into_iter()
            .map(|entry| (entry.id.to_string(), Some(entry.default_shortcut)))
            .collect();

        let mut instance = Self {
            shortcuts,
            index: HashMap::new(),
        };
        instance.rebuild_index();
        instance
    }

    /// Rebuild the inverted index for fast lookups
    fn rebuild_index(&mut self) {
        self.index.clear();
        for (id, shortcut_opt) in &self.shortcuts {
            if let Some(shortcut) = shortcut_opt {
                self.index.insert(shortcut.clone(), id.clone());
            }
        }
    }

    /// Get metadata for all shortcuts
    #[must_use]
    pub fn get_all_metadata() -> HashMap<String, ShortcutMetadata> {
        get_all_shortcut_definitions()
            .into_iter()
            .map(|entry| {
                (
                    entry.id.to_string(),
                    ShortcutMetadata {
                        description: entry.description,
                        category: entry.category,
                    },
                )
            })
            .collect()
    }

    /// Get all shortcuts in the order they were defined, grouped by category
    #[must_use]
    pub fn get_all_ordered() -> HashMap<ShortcutCategory, Vec<(String, ShortcutMetadata)>> {
        let mut grouped: HashMap<ShortcutCategory, Vec<(String, ShortcutMetadata)>> = HashMap::new();

        for entry in get_all_shortcut_definitions() {
            grouped.entry(entry.category)
                .or_default()
                .push((
                    entry.id.to_string(),
                    ShortcutMetadata {
                        description: entry.description,
                        category: entry.category,
                    },
                ));
        }

        grouped
    }

    /// Get a shortcut by ID
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&KeyboardShortcut> {
        self.shortcuts.get(id).and_then(|opt| opt.as_ref())
    }

    /// Set a shortcut by ID and rebuild the index
    pub fn set(&mut self, id: &str, shortcut: Option<KeyboardShortcut>) {
        self.shortcuts.insert(id.to_string(), shortcut);
        self.rebuild_index();
    }

    /// Find which action matches the given keyboard event (O(1) lookup)
    #[must_use]
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn find_action(&self, code: &str, ctrl: bool, shift: bool, alt: bool, meta: bool) -> Option<&str> {
        let shortcut = KeyboardShortcut::new(code.to_string(), ctrl, shift, alt, meta);
        self.index.get(&shortcut).map(String::as_str)
    }

    /// Check if a shortcut would conflict with any existing shortcuts
    #[must_use]
    pub fn check_conflicts(&self, new_shortcut: &KeyboardShortcut, exclude_id: Option<&str>) -> Vec<String> {
        if let Some(conflicting_id) = self.index.get(new_shortcut) {
            if let Some(exclude) = exclude_id {
                if conflicting_id == exclude {
                    return Vec::new();
                }
            }
            vec![conflicting_id.clone()]
        } else {
            Vec::new()
        }
    }

    /// Merge in any new shortcuts from defaults that aren't in the current settings
    /// This is used for migrating settings when new shortcuts are added to the codebase
    pub fn merge_with_defaults(&mut self) {
        let defaults = Self::default_shortcuts();
        let mut needs_rebuild = false;

        for (id, default_shortcut) in defaults.shortcuts {
            if let std::collections::hash_map::Entry::Vacant(e) = self.shortcuts.entry(id) {
                e.insert(default_shortcut);
                needs_rebuild = true;
            }
        }

        if needs_rebuild {
            self.rebuild_index();
        }
    }
}

impl Default for KeyboardShortcuts {
    fn default() -> Self {
        Self::default_shortcuts()
    }
}

/// Helper function to setup keyboard shortcut handlers with common filtering logic
pub fn setup_shortcut_handler<F, S>(
    is_capturing_shortcut: leptos::ReadSignal<bool>,
    shortcuts: S,
    handler: F,
) where
    F: Fn(&str, &web_sys::KeyboardEvent) + 'static,
    S: SignalGet<Value = KeyboardShortcuts> + Copy + 'static,
{
    leptos::leptos_dom::helpers::window_event_listener(leptos::ev::keydown, move |ev| {
        // Don't handle shortcuts when capturing in the shortcuts editor
        if is_capturing_shortcut.get() {
            return;
        }

        // Don't handle keyboard shortcuts when typing in input fields
        let Some(target) = ev.target() else { return };
        let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() else { return };
        let tag_name = element.tag_name().to_lowercase();
        if tag_name == "input" || tag_name == "textarea" {
            return;
        }

        // Ignore repeat events
        if ev.repeat() {
            return;
        }

        // Find matching action
        let current_shortcuts = shortcuts.get();
        let action = current_shortcuts.find_action(
            &ev.code(),
            ev.ctrl_key(),
            ev.shift_key(),
            ev.alt_key(),
            ev.meta_key(),
        );

        // Call handler if action found
        if let Some(action_id) = action {
            handler(action_id, &ev);
        }
    });
}
