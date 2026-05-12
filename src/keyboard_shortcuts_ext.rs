use leptos::SignalGet;
use wasm_bindgen::JsCast;
use railgraph_core::models::{KeyboardShortcut, KeyboardShortcuts};

/// Create a `KeyboardShortcut` from a `KeyboardEvent`
#[must_use]
pub fn keyboard_shortcut_from_event(ev: &web_sys::KeyboardEvent) -> KeyboardShortcut {
    KeyboardShortcut::new(
        ev.code(),
        ev.ctrl_key(),
        ev.shift_key(),
        ev.alt_key(),
        ev.meta_key(),
    )
}

/// Detect if running on Mac platform
#[must_use]
pub fn is_mac_platform() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(platform) = window.navigator().platform() else {
        return false;
    };
    platform.contains("Mac") || platform.contains("iPhone") || platform.contains("iPad")
}

/// Detect if running on Windows platform
#[must_use]
pub fn is_windows_platform() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(platform) = window.navigator().platform() else {
        return false;
    };
    platform.contains("Win")
}

/// Check if the event target is an input field where keyboard shortcuts should be ignored
#[must_use]
pub fn is_input_field_target(ev: &web_sys::KeyboardEvent) -> bool {
    let Some(target) = ev.target() else {
        return false;
    };
    let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() else {
        return false;
    };
    let tag_name = element.tag_name().to_lowercase();
    tag_name == "input" || tag_name == "textarea"
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
        // Use try_get() to safely handle disposed signals
        if is_capturing_shortcut.try_get().unwrap_or(false) {
            return;
        }

        // Don't handle keyboard shortcuts when typing in input fields
        if is_input_field_target(&ev) {
            return;
        }

        // Ignore repeat events
        if ev.repeat() {
            return;
        }

        // Find matching action
        // Use try_get() to safely handle disposed signals
        let Some(current_shortcuts) = shortcuts.try_get() else {
            return;
        };
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

/// Helper function to setup a listener for a single specific keyboard shortcut
/// This is useful for components that need to respond to one shortcut
pub fn setup_single_shortcut_handler<F>(
    is_capturing_shortcut: leptos::ReadSignal<bool>,
    shortcut: KeyboardShortcut,
    handler: F,
) where
    F: Fn(&web_sys::KeyboardEvent) + 'static,
{
    leptos::leptos_dom::helpers::window_event_listener(leptos::ev::keydown, move |ev| {
        // Don't handle shortcuts when capturing in the shortcuts editor
        // Use try_get() to safely handle disposed signals
        if is_capturing_shortcut.try_get().unwrap_or(false) {
            return;
        }

        // Don't handle keyboard shortcuts when typing in input fields
        if is_input_field_target(&ev) {
            return;
        }

        // Check if this event matches our shortcut
        let event_shortcut = keyboard_shortcut_from_event(&ev);
        if shortcut != event_shortcut {
            return;
        }

        handler(&ev);
    });
}

/// Frontend wrapper: get default shortcuts using browser platform detection
#[must_use]
pub fn default_shortcuts() -> KeyboardShortcuts {
    KeyboardShortcuts::default_shortcuts_for_platform(is_mac_platform())
}

/// Frontend wrapper: get all metadata using browser platform detection
#[must_use]
pub fn get_all_metadata() -> std::collections::HashMap<String, railgraph_core::models::ShortcutMetadata> {
    KeyboardShortcuts::get_all_metadata(is_mac_platform())
}

/// Frontend wrapper: get all ordered shortcuts using browser platform detection
#[must_use]
pub fn get_all_ordered() -> std::collections::HashMap<
    railgraph_core::models::ShortcutCategory,
    Vec<(String, railgraph_core::models::ShortcutMetadata)>,
> {
    KeyboardShortcuts::get_all_ordered(is_mac_platform())
}

/// Frontend wrapper: merge with defaults using browser platform detection
pub fn merge_with_defaults(shortcuts: &mut KeyboardShortcuts) {
    shortcuts.merge_with_defaults(is_mac_platform());
}
