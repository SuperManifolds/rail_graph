use leptos::{
    component, create_signal, view, IntoView, SignalGet, SignalSet,
    provide_context, create_effect,
};

use crate::models::UserSettings;
use crate::user_settings_ext::UserSettingsStorage;
use crate::tauri_bridge;

/// Router for child windows. Reads `window_type` and `session` from URL params,
/// waits for init data from the main window via Tauri events, then renders
/// the appropriate child component.
#[component]
#[must_use]
pub fn ChildWindowRouter(window_type: String, session: String) -> impl IntoView {
    let (init_data, set_init_data) = create_signal(None::<String>);
    let (is_capturing_shortcut, set_is_capturing_shortcut) = create_signal(false);

    // Load user settings (from localStorage, same as main window)
    let user_settings = UserSettings::load();
    let (user_settings_sig, set_user_settings_sig) = create_signal(user_settings);
    provide_context((user_settings_sig, set_user_settings_sig));
    provide_context((is_capturing_shortcut, set_is_capturing_shortcut));

    // Provide a no-op resize trigger (native windows handle their own sizing)
    let (resize_trigger, set_resize_trigger) = create_signal(0u32);
    let _ = resize_trigger;
    provide_context(set_resize_trigger);

    // Signal the main window that we're ready, then listen for init data
    let session_clone = session.clone();
    create_effect(move |_| {
        let session = session_clone.clone();
        leptos::spawn_local(async move {
            // Emit child-ready event
            let ready_event = format!("child-ready:{session}");
            if let Err(e) = tauri_bridge::emit_event(&ready_event, "").await {
                leptos::logging::error!("Failed to emit child-ready: {}", e);
                return;
            }

            // Listen for init-data event
            let init_event = format!("init-data:{session}");
            let set_init = set_init_data;
            if let Err(e) = tauri_bridge::listen_event_once(&init_event, move |payload| {
                set_init.set(Some(payload));
            }).await {
                leptos::logging::error!("Failed to listen for init-data: {}", e);
            }
        });
    });

    let window_type_owned = window_type.clone();
    let session_owned = session.clone();

    view! {
        <div class="child-window-root">
            {move || {
                let Some(data) = init_data.get() else {
                    return view! { <div class="child-window-loading">"Loading..."</div> }.into_view();
                };
                render_child_content(&window_type_owned, &session_owned, &data)
            }}
        </div>
    }
}

fn render_child_content(window_type: &str, session: &str, init_data: &str) -> leptos::View {
    use crate::components::child::edit_track_child::EditTrackChild;

    if window_type == "edit-track" {
        let Ok(init) = serde_json::from_str::<crate::window_protocol::EditTrackInit>(init_data) else {
            leptos::logging::error!("Failed to deserialize EditTrackInit");
            return leptos::view! { <div>"Failed to load data"</div> }.into_view();
        };
        leptos::view! { <EditTrackChild init=init session=session.to_string() /> }.into_view()
    } else {
        leptos::logging::error!("Unknown child window type: {}", window_type);
        leptos::view! { <div>"Unknown window type"</div> }.into_view()
    }
}
