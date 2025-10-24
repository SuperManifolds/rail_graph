use leptos::{component, view, IntoView, Children, Callback, create_effect, SignalGet, MaybeSignal, Callable, use_context, ReadSignal, WriteSignal};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys;
use crate::models::{UserSettings, KeyboardShortcut};

/// Detects if the current platform uses Cmd (Mac/iOS) or Ctrl (Windows/Linux)
fn is_mac_platform() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Some(navigator) = window.navigator().platform().ok() else { return false };
    navigator.contains("Mac") || navigator.contains("iPhone") || navigator.contains("iPad")
}

/// Detects if the current platform is Windows
fn is_windows_platform() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Some(navigator) = window.navigator().platform().ok() else { return false };
    navigator.contains("Win")
}

#[component]
#[must_use]
pub fn Button(
    /// The click handler to call when button is clicked or shortcut is pressed
    on_click: Callback<web_sys::MouseEvent>,
    /// The button contents (icons, text, etc.)
    children: Children,
    /// Optional keyboard shortcut ID (e.g., "add_station", "add_track")
    /// Will look up the shortcut from user settings
    #[prop(optional, into)]
    shortcut_id: Option<String>,
    /// Optional CSS class for the button
    #[prop(optional, into)]
    class: MaybeSignal<String>,
    /// Optional active state - when true, appends " active" to the class
    #[prop(optional, into)]
    active: MaybeSignal<bool>,
    /// Optional title/tooltip for the button
    #[prop(optional, into)]
    title: Option<String>,
) -> impl IntoView {
    // Get user settings from context to look up shortcuts
    let user_settings_context = use_context::<(ReadSignal<UserSettings>, WriteSignal<UserSettings>)>();

    // Look up the keyboard shortcut if a shortcut_id is provided
    let shortcut_info = shortcut_id.as_ref().and_then(|id| {
        user_settings_context.as_ref().and_then(|(user_settings, _)| {
            user_settings.get().keyboard_shortcuts.get(id).cloned()
        })
    });

    // Build the final tooltip with shortcut hint if provided
    let is_mac = is_mac_platform();
    let is_windows = is_windows_platform();
    let final_title = if let Some(ref shortcut) = shortcut_info {
        let shortcut_hint = shortcut.format(is_mac, is_windows);
        match title {
            Some(base_title) => format!("{base_title} ({shortcut_hint})"),
            None => format!("({shortcut_hint})"),
        }
    } else {
        title.unwrap_or_default()
    };

    // Set up keyboard shortcut listener if shortcut is provided
    if let Some(shortcut) = shortcut_info {
        create_effect(move |_| {
            let Some(window) = web_sys::window() else { return };
            let Some(document) = window.document() else { return };

            let shortcut = shortcut.clone();
            let handler = Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
                // Don't handle keyboard shortcuts when typing in input fields
                let Some(target) = ev.target() else { return };
                let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() else { return };
                let tag_name = element.tag_name().to_lowercase();
                if tag_name == "input" || tag_name == "textarea" {
                    return;
                }

                // Check if this event matches our shortcut
                let event_shortcut = KeyboardShortcut::new(
                    ev.code(),
                    ev.ctrl_key(),
                    ev.shift_key(),
                    ev.alt_key(),
                    ev.meta_key()
                );
                if shortcut != event_shortcut {
                    return;
                }

                ev.prevent_default();
                let Ok(mouse_ev) = web_sys::MouseEvent::new("click") else { return };
                on_click.call(mouse_ev);
            }) as Box<dyn FnMut(_)>);

            let _ = document.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
            handler.forget();
        });
    }

    view! {
        <button
            class=move || {
                let base_class = class.get();
                if active.get() {
                    format!("{base_class} active")
                } else {
                    base_class
                }
            }
            on:click=move |ev| on_click.call(ev)
            title=final_title
        >
            {children()}
        </button>
    }
}
