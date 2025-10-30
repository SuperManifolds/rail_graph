use leptos::{component, view, IntoView, Children, Callback, SignalGet, MaybeSignal, Callable, use_context, ReadSignal, WriteSignal, create_rw_signal};
use web_sys;
use crate::models::{UserSettings, is_mac_platform, is_windows_platform, setup_single_shortcut_handler};

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
        // Get is_capturing_shortcut from context, default to false if not available
        let is_capturing_shortcut = use_context::<ReadSignal<bool>>()
            .unwrap_or_else(|| create_rw_signal(false).read_only());

        setup_single_shortcut_handler(is_capturing_shortcut, shortcut, move |ev| {
            ev.prevent_default();
            let Ok(mouse_ev) = web_sys::MouseEvent::new("click") else { return };
            on_click.call(mouse_ev);
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
