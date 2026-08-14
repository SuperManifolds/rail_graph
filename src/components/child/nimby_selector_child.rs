use crate::components::nimby_line_selector::NimbyLineSelector;
use crate::import::nimby::NimbyImportConfig;
use crate::window_protocol::{ImporterNimbyInit, ImporterNimbyResult, ImporterNimbyUpdate};
use leptos::{component, create_signal, view, Callback, IntoView, Signal, SignalGet, SignalSet};

#[component]
#[must_use]
pub fn NimbySelectorChild(init: ImporterNimbyInit, session: String) -> impl IntoView {
    let (data, _) = create_signal(init.data);
    let (handedness, _) = create_signal(init.handedness);
    let station_spacing = init.station_spacing;
    let (import_error, set_import_error) = create_signal(init.error);

    // Listen for update-data events carrying import errors raised by the parent.
    {
        let session_for_listen = session.clone();
        leptos::spawn_local(async move {
            let event_name = format!("update-data:{session_for_listen}");
            match crate::tauri_bridge::listen_event(&event_name, move |payload| {
                if let Ok(update) = serde_json::from_str::<ImporterNimbyUpdate>(&payload) {
                    set_import_error.set(update.error);
                }
            })
            .await
            {
                Ok(unlisten) => std::mem::forget(unlisten),
                Err(e) => leptos::logging::error!("Failed to listen for update-data: {}", e),
            }
        });
    }

    let session_import = session.clone();
    let handle_import = move |cfg: NimbyImportConfig| {
        let result = ImporterNimbyResult::Import(cfg);
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_import}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    };

    let handle_cancel = move |()| {
        leptos::spawn_local(async move {
            crate::tauri_bridge::close_current_window().await;
        });
    };

    view! {
        <div class="child-window-content">
            <NimbyLineSelector
                data=Signal::derive(move || data.get())
                handedness=Signal::derive(move || handedness.get())
                station_spacing=Signal::derive(move || station_spacing)
                on_cancel=Callback::new(handle_cancel)
                on_import=Callback::new(handle_import)
                import_error=import_error
            />
        </div>
    }
}
