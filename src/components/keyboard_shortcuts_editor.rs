use leptos::{component, view, IntoView, create_signal, SignalGet, SignalSet, use_context, spawn_local, SignalUpdate, ReadSignal, WriteSignal};
use crate::models::{UserSettings, KeyboardShortcut, KeyboardShortcuts, ShortcutCategory};
use std::collections::HashMap;

fn is_mac_platform() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(platform) = window.navigator().platform() else {
        return false;
    };
    platform.contains("Mac") || platform.contains("iPhone") || platform.contains("iPad")
}

fn is_windows_platform() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(platform) = window.navigator().platform() else {
        return false;
    };
    platform.contains("Win")
}

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn KeyboardShortcutsEditor() -> impl IntoView {
    // Get user settings from context
    let (user_settings, set_user_settings): (ReadSignal<UserSettings>, WriteSignal<UserSettings>) =
        use_context().expect("UserSettings context not found");

    // Get capturing state from context
    let (_, set_is_capturing_shortcut): (ReadSignal<bool>, WriteSignal<bool>) =
        use_context().expect("is_capturing_shortcut context not found");

    // Local state for shortcut editing
    let (capturing_for, set_capturing_for) = create_signal(None::<String>);
    let (conflict_warning, set_conflict_warning) = create_signal(None::<String>);
    let (browser_warning, set_browser_warning) = create_signal(false);

    let is_mac = is_mac_platform();
    let is_windows = is_windows_platform();

    // Handle keyboard shortcut capture
    let handle_keydown = move |ev: web_sys::KeyboardEvent| {
        if let Some(shortcut_id) = capturing_for.get() {
            ev.prevent_default();
            ev.stop_immediate_propagation();

            if ev.key() == "Escape" {
                set_capturing_for.set(None);
                set_conflict_warning.set(None);
                set_browser_warning.set(false);

                // Delay clearing the capturing flag to ensure any in-flight events see it as true
                let _ = leptos::leptos_dom::helpers::set_timeout_with_handle(
                    move || set_is_capturing_shortcut.set(false),
                    std::time::Duration::from_millis(50),
                );
                return;
            }

            let code = ev.code();

            // Ignore modifier-only key presses
            if code == "Control" || code == "ControlLeft" || code == "ControlRight"
                || code == "Shift" || code == "ShiftLeft" || code == "ShiftRight"
                || code == "Alt" || code == "AltLeft" || code == "AltRight"
                || code == "Meta" || code == "MetaLeft" || code == "MetaRight"
            {
                return;
            }

            let ctrl = ev.ctrl_key();
            let shift = ev.shift_key();
            let alt = ev.alt_key();
            let meta = ev.meta_key();

            let new_shortcut = KeyboardShortcut::new(code, ctrl, shift, alt, meta);

            // Check for conflicts
            let shortcuts = user_settings.get().keyboard_shortcuts.clone();
            let conflicts = shortcuts.check_conflicts(&new_shortcut, Some(&shortcut_id));

            if !conflicts.is_empty() {
                let metadata = KeyboardShortcuts::get_all_metadata();
                let conflict_names: Vec<String> = conflicts.iter()
                    .filter_map(|id| metadata.get(id).map(|m| m.description.to_string()))
                    .collect();
                set_conflict_warning.set(Some(format!("Conflicts with: {}", conflict_names.join(", "))));
                set_browser_warning.set(new_shortcut.is_likely_browser_conflict());
                return;
            }

            // Check for browser conflicts
            set_browser_warning.set(new_shortcut.is_likely_browser_conflict());
            set_conflict_warning.set(None);

            // Update shortcut and get updated settings
            let settings_to_save = set_user_settings.try_update(|settings| {
                settings.keyboard_shortcuts.set(&shortcut_id, Some(new_shortcut));
                settings.clone()
            }).expect("Failed to update settings");

            // Save to IndexedDB
            spawn_local(async move {
                if let Err(e) = settings_to_save.save().await {
                    leptos::logging::error!("Failed to save shortcuts: {}", e);
                } else {
                    leptos::logging::log!("Successfully saved shortcut");
                }
            });

            set_capturing_for.set(None);
            set_browser_warning.set(false);

            // Delay clearing the capturing flag to ensure any in-flight events see it as true
            let _ = leptos::leptos_dom::helpers::set_timeout_with_handle(
                move || set_is_capturing_shortcut.set(false),
                std::time::Duration::from_millis(50),
            );
        }
    };

    // Clear single shortcut (set to None)
    let clear_shortcut = move |shortcut_id: String| {
        let settings_to_save = set_user_settings.try_update(|settings| {
            settings.keyboard_shortcuts.set(&shortcut_id, None);
            settings.clone()
        }).expect("Failed to update settings");

        // Save to IndexedDB
        spawn_local(async move {
            if let Err(e) = settings_to_save.save().await {
                leptos::logging::error!("Failed to save shortcuts: {}", e);
            } else {
                leptos::logging::log!("Successfully cleared shortcut");
            }
        });
    };

    // Reset single shortcut to default
    let reset_shortcut = move |shortcut_id: String| {
        let defaults = KeyboardShortcuts::default_shortcuts();
        if let Some(default_shortcut) = defaults.get(&shortcut_id) {
            let settings_to_save = set_user_settings.try_update(|settings| {
                settings.keyboard_shortcuts.set(&shortcut_id, Some(default_shortcut.clone()));
                settings.clone()
            }).expect("Failed to update settings");

            // Save to IndexedDB
            spawn_local(async move {
                if let Err(e) = settings_to_save.save().await {
                    leptos::logging::error!("Failed to save shortcuts: {}", e);
                } else {
                    leptos::logging::log!("Successfully saved shortcut reset");
                }
            });
        }
    };

    // Reset all shortcuts to defaults
    let reset_all_shortcuts = move |_| {
        let settings_to_save = set_user_settings.try_update(|settings| {
            settings.keyboard_shortcuts = KeyboardShortcuts::default_shortcuts();
            settings.clone()
        }).expect("Failed to update settings");

        // Save to IndexedDB
        spawn_local(async move {
            if let Err(e) = settings_to_save.save().await {
                leptos::logging::error!("Failed to save shortcuts: {}", e);
            } else {
                leptos::logging::log!("Successfully reset all shortcuts");
            }
        });
    };

    // Group shortcuts by category in the order they were defined
    let shortcuts_by_category = move || {
        let ordered_metadata = KeyboardShortcuts::get_all_ordered();
        let shortcuts = user_settings.get().keyboard_shortcuts.clone();

        let mut grouped: HashMap<ShortcutCategory, Vec<(String, String, Option<KeyboardShortcut>)>> = HashMap::new();

        for (category, metadata_list) in ordered_metadata {
            for (id, shortcut_metadata) in metadata_list {
                let shortcut_opt = shortcuts.shortcuts.get(&id).cloned().flatten();
                grouped.entry(category)
                    .or_default()
                    .push((id.clone(), shortcut_metadata.description.to_string(), shortcut_opt));
            }
        }

        grouped
    };

    view! {
        <div class="settings-content" on:keydown=handle_keydown>
            <div class="settings-section">
                <h3>"Keyboard Shortcuts"</h3>
                <p class="section-description">
                    "Click on a shortcut to change it. Press ESC to cancel."
                </p>

                {move || {
                    let categories = vec![
                        (ShortcutCategory::Navigation, "Navigation"),
                        (ShortcutCategory::Infrastructure, "Infrastructure"),
                        (ShortcutCategory::Project, "Project"),
                    ];

                    let grouped = shortcuts_by_category();

                    categories.into_iter().map(|(category, label)| {
                        let shortcuts_in_category = grouped.get(&category).cloned().unwrap_or_default();

                        view! {
                            <div class="shortcut-category">
                                <h4>{label}</h4>
                                <div class="shortcut-list">
                                    {shortcuts_in_category.into_iter().map(|(id, description, shortcut_opt)| {
                                        let id_for_class = id.clone();
                                        let id_for_text = id.clone();
                                        let id_for_clear = id.clone();
                                        let id_for_reset = id.clone();

                                        view! {
                                            <div class="shortcut-row">
                                                <span class="shortcut-description">{description}</span>
                                                <button
                                                    class="shortcut-binding"
                                                    class:capturing=move || capturing_for.get() == Some(id_for_class.clone())
                                                    on:click=move |_| {
                                                        set_capturing_for.set(Some(id.clone()));
                                                        set_is_capturing_shortcut.set(true);
                                                        set_conflict_warning.set(None);
                                                        set_browser_warning.set(false);
                                                    }
                                                >
                                                    {move || {
                                                        if capturing_for.get() == Some(id_for_text.clone()) {
                                                            "Press keys...".to_string()
                                                        } else if let Some(ref shortcut) = shortcut_opt {
                                                            shortcut.format(is_mac, is_windows)
                                                        } else {
                                                            "None".to_string()
                                                        }
                                                    }}
                                                </button>
                                                <button
                                                    class="shortcut-clear"
                                                    on:click=move |_| clear_shortcut(id_for_clear.clone())
                                                    title="Clear shortcut"
                                                >
                                                    <i class="fa-solid fa-times"></i>
                                                </button>
                                                <button
                                                    class="shortcut-reset"
                                                    on:click=move |_| reset_shortcut(id_for_reset.clone())
                                                    title="Reset to default"
                                                >
                                                    <i class="fa-solid fa-undo"></i>
                                                </button>
                                            </div>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}

                {move || conflict_warning.get().map(|warning| {
                    view! {
                        <div class="shortcut-warning conflict-warning">
                            <i class="fa-solid fa-exclamation-triangle"></i>
                            " " {warning}
                        </div>
                    }
                })}

                {move || {
                    if browser_warning.get() {
                        Some(view! {
                            <div class="shortcut-warning browser-warning">
                                <i class="fa-solid fa-info-circle"></i>
                                " This shortcut may conflict with browser shortcuts"
                            </div>
                        })
                    } else {
                        None
                    }
                }}

                <div class="shortcut-actions">
                    <button class="reset-all-button" on:click=reset_all_shortcuts>
                        "Reset All to Defaults"
                    </button>
                </div>
            </div>
        </div>
    }
}
