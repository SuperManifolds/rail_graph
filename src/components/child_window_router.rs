use leptos::{
    component, create_signal, view, IntoView,
    provide_context,
};

use crate::models::UserSettings;
use crate::user_settings_ext::UserSettingsStorage;

/// Read init data from localStorage (written by the main window before creating this window).
fn read_init_data(session: &str) -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    let key = format!("__tauri_init_{session}");
    let data = storage.get_item(&key).ok()??;
    let _ = storage.remove_item(&key);
    Some(data)
}

/// Router for child windows. Reads `window_type` and `session` from URL params,
/// loads init data from localStorage, then renders the appropriate child component.
#[component]
#[must_use]
pub fn ChildWindowRouter(window_type: String, session: String) -> impl IntoView {
    let (is_capturing_shortcut, set_is_capturing_shortcut) = create_signal(false);

    // Load user settings (from localStorage, same as main window)
    let user_settings = UserSettings::load().unwrap_or_default();
    let (user_settings_sig, set_user_settings_sig) = create_signal(user_settings);
    provide_context((user_settings_sig, set_user_settings_sig));
    provide_context((is_capturing_shortcut, set_is_capturing_shortcut));

    // Provide a no-op resize trigger (native windows handle their own sizing)
    let set_resize_trigger = create_signal(0u32).1;
    provide_context(set_resize_trigger);

    // Read init data synchronously from localStorage (written by main window)
    let init_data = read_init_data(&session);

    view! {
        <div class="child-window-root">
            {if let Some(data) = init_data {
                render_child_content(&window_type, &session, &data)
            } else {
                leptos::logging::error!("No init data found for session {}", session);
                leptos::view! { <div class="child-window-loading">"Failed to load window data"</div> }.into_view()
            }}
        </div>
    }
}

fn render_child_content(window_type: &str, session: &str, init_data: &str) -> leptos::View {
    use crate::components::child::add_station_child::AddStationChild;
    use crate::components::child::create_view_child::CreateViewChild;
    use crate::components::child::csv_mapper_child::CsvMapperChild;
    use crate::components::child::edit_junction_child::EditJunctionChild;
    use crate::components::child::edit_station_child::EditStationChild;
    use crate::components::child::edit_track_child::EditTrackChild;
    use crate::components::child::line_editor_child::LineEditorChild;
    use crate::components::child::nimby_selector_child::NimbySelectorChild;
    use crate::components::child::project_manager_child::ProjectManagerChild;
    use crate::components::child::settings_child::SettingsChild;

    match window_type {
        "edit-track" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::EditTrackInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize EditTrackInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <EditTrackChild init=init session=session.to_string() /> }.into_view()
        }
        "edit-junction" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::EditJunctionInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize EditJunctionInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <EditJunctionChild init=init session=session.to_string() /> }.into_view()
        }
        "edit-station" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::EditStationInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize EditStationInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <EditStationChild init=init session=session.to_string() /> }.into_view()
        }
        "line-editor" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::LineEditorInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize LineEditorInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <LineEditorChild init=init session=session.to_string() /> }.into_view()
        }
        "settings" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::SettingsInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize SettingsInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <SettingsChild init=init session=session.to_string() /> }.into_view()
        }
        "importer-csv" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::ImporterCsvInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize ImporterCsvInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <CsvMapperChild init=init session=session.to_string() /> }.into_view()
        }
        "importer-nimby" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::ImporterNimbyInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize ImporterNimbyInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <NimbySelectorChild init=init session=session.to_string() /> }.into_view()
        }
        "add-station" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::AddStationInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize AddStationInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <AddStationChild init=init session=session.to_string() /> }.into_view()
        }
        "create-view" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::CreateViewInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize CreateViewInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <CreateViewChild init=init session=session.to_string() /> }.into_view()
        }
        "project-manager" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::ProjectManagerInit>(init_data) else {
                leptos::logging::error!("Failed to deserialize ProjectManagerInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <ProjectManagerChild init=init session=session.to_string() /> }.into_view()
        }
        _ => {
            leptos::logging::error!("Unknown child window type: {}", window_type);
            leptos::view! { <div>"Unknown window type"</div> }.into_view()
        }
    }
}
