use leptos::{component, create_signal, provide_context, view, IntoView};

use crate::models::UserSettings;
use crate::user_settings_ext::UserSettingsStorage;
use wasm_bindgen::JsCast;

/// Delay before measuring content, so layout and fonts have settled.
const FIT_DELAY_MS: u64 = 150;
/// Breathing room added beyond the measured content size.
const FIT_PADDING_PX: f64 = 12.0;
/// Overflow smaller than this is ignored (rounding noise, deliberate 1px cuts).
const MIN_OVERFLOW_PX: f64 = 4.0;
/// Space kept free between the window and the screen edge when clamping.
const SCREEN_MARGIN_PX: f64 = 80.0;

/// Largest content overflow (scroll size minus client size) across the
/// document, horizontally and vertically. This is how much taller/wider the
/// window must grow for the content to fit without scrolling.
fn measure_overflow(document: &web_sys::Document) -> (f64, f64) {
    let mut overflow_x: f64 = 0.0;
    let mut overflow_y: f64 = 0.0;
    let Ok(nodes) = document.query_selector_all("*") else {
        return (0.0, 0.0);
    };
    for i in 0..nodes.length() {
        let Some(el) = nodes.item(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok()) else {
            continue;
        };
        if el.client_height() > 0 {
            overflow_y = overflow_y.max(f64::from(el.scroll_height() - el.client_height()));
        }
        if el.client_width() > 0 {
            overflow_x = overflow_x.max(f64::from(el.scroll_width() - el.client_width()));
        }
    }
    (overflow_x, overflow_y)
}

/// Grow the window (never shrink) so its content fits without scrolling,
/// clamped to the screen. Child windows are created at fixed sizes that don't
/// always fit their content; this sizes them to what actually rendered.
fn fit_window_to_content() {
    let Some(window) = web_sys::window() else { return };
    let Some(document) = window.document() else { return };
    let (overflow_x, overflow_y) = measure_overflow(&document);
    if overflow_x < MIN_OVERFLOW_PX && overflow_y < MIN_OVERFLOW_PX {
        return;
    }

    let inner_w = window.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let inner_h = window.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    if inner_w <= 0.0 || inner_h <= 0.0 {
        return;
    }
    let (max_w, max_h) = window.screen().map_or((f64::MAX, f64::MAX), |s| {
        (
            s.avail_width().map_or(f64::MAX, |w| f64::from(w) - SCREEN_MARGIN_PX),
            s.avail_height().map_or(f64::MAX, |h| f64::from(h) - SCREEN_MARGIN_PX),
        )
    });

    let desired_w = (inner_w + overflow_x + FIT_PADDING_PX).min(max_w).max(inner_w);
    let desired_h = (inner_h + overflow_y + FIT_PADDING_PX).min(max_h).max(inner_h);
    if desired_w - inner_w < MIN_OVERFLOW_PX && desired_h - inner_h < MIN_OVERFLOW_PX {
        return;
    }

    leptos::spawn_local(async move {
        if let Err(e) =
            crate::tauri_bridge::set_current_window_logical_size(desired_w, desired_h).await
        {
            leptos::logging::error!("failed to fit window to content: {e}");
        }
    });
}

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

    // Size the window to its rendered content once layout has settled.
    leptos::set_timeout(
        fit_window_to_content,
        std::time::Duration::from_millis(FIT_DELAY_MS),
    );

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
            let Ok(init) = serde_json::from_str::<crate::window_protocol::EditTrackInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize EditTrackInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <EditTrackChild init=init session=session.to_string() /> }.into_view()
        }
        "edit-junction" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::EditJunctionInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize EditJunctionInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <EditJunctionChild init=init session=session.to_string() /> }
                .into_view()
        }
        "edit-station" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::EditStationInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize EditStationInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <EditStationChild init=init session=session.to_string() /> }.into_view()
        }
        "line-editor" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::LineEditorInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize LineEditorInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <LineEditorChild init=init session=session.to_string() /> }.into_view()
        }
        "settings" => {
            let Ok(init) = serde_json::from_str::<crate::window_protocol::SettingsInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize SettingsInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <SettingsChild init=init session=session.to_string() /> }.into_view()
        }
        "importer-csv" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::ImporterCsvInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize ImporterCsvInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <CsvMapperChild init=init session=session.to_string() /> }.into_view()
        }
        "importer-nimby" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::ImporterNimbyInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize ImporterNimbyInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <NimbySelectorChild init=init session=session.to_string() /> }
                .into_view()
        }
        "add-station" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::AddStationInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize AddStationInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <AddStationChild init=init session=session.to_string() /> }.into_view()
        }
        "create-view" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::CreateViewInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize CreateViewInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <CreateViewChild init=init session=session.to_string() /> }.into_view()
        }
        "project-manager" => {
            let Ok(init) =
                serde_json::from_str::<crate::window_protocol::ProjectManagerInit>(init_data)
            else {
                leptos::logging::error!("Failed to deserialize ProjectManagerInit");
                return leptos::view! { <div>"Failed to load data"</div> }.into_view();
            };
            leptos::view! { <ProjectManagerChild init=init session=session.to_string() /> }
                .into_view()
        }
        _ => {
            leptos::logging::error!("Unknown child window type: {}", window_type);
            leptos::view! { <div>"Unknown window type"</div> }.into_view()
        }
    }
}
