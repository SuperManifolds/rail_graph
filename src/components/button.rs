use leptos::{component, view, IntoView, Children, Callback, create_effect, SignalGet, MaybeSignal, Callable};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys;

/// Detects if the current platform uses Cmd (Mac/iOS) or Ctrl (Windows/Linux)
fn is_mac_platform() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Some(navigator) = window.navigator().platform().ok() else { return false };
    navigator.contains("Mac") || navigator.contains("iPhone") || navigator.contains("iPad")
}

/// Formats a keyboard shortcut for display in tooltips
fn format_shortcut(key: &str) -> String {
    if is_mac_platform() {
        format!("⌘⇧{}", key.to_uppercase())
    } else {
        format!("Ctrl+Shift+{}", key.to_uppercase())
    }
}

#[component]
#[must_use]
pub fn Button(
    /// The click handler to call when button is clicked or shortcut is pressed
    on_click: Callback<web_sys::MouseEvent>,
    /// The button contents (icons, text, etc.)
    children: Children,
    /// Optional keyboard shortcut key (e.g., "L", "S", "T", "J")
    /// Will be triggered with Cmd/Ctrl+Shift+{key}
    #[prop(optional, into)]
    shortcut: Option<String>,
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
    // Build the final tooltip with shortcut hint if provided
    let final_title = if let Some(ref key) = shortcut {
        let shortcut_hint = format_shortcut(key);
        match title {
            Some(base_title) => format!("{base_title} ({shortcut_hint})"),
            None => format!("({shortcut_hint})"),
        }
    } else {
        title.unwrap_or_default()
    };

    // Set up keyboard shortcut listener if shortcut is provided
    if let Some(key) = shortcut {
        let key_lower = key.to_lowercase();
        let key_upper = key.to_uppercase();
        let is_mac = is_mac_platform();

        create_effect(move |_| {
            let Some(window) = web_sys::window() else { return };
            let Some(document) = window.document() else { return };

            let key_lower = key_lower.clone();
            let key_upper = key_upper.clone();
            let handler = Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
                // Check for Cmd (Mac) or Ctrl (Windows/Linux) + Shift + key
                let modifier_pressed = if is_mac {
                    ev.meta_key()
                } else {
                    ev.ctrl_key()
                };

                // Early return if modifier keys don't match
                if !modifier_pressed || !ev.shift_key() || ev.alt_key() {
                    return;
                }

                let pressed_key = ev.key();
                let is_match = pressed_key == key_lower || pressed_key == key_upper;
                if !is_match {
                    return;
                }

                ev.prevent_default();
                // Create a synthetic mouse event to pass to the callback
                if let Ok(mouse_ev) = web_sys::MouseEvent::new("click") {
                    on_click.call(mouse_ev);
                }
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
