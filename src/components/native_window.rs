use leptos::{
    component, create_effect, create_signal, view, IntoView, MaybeSignal, Signal,
    SignalGet, SignalSet, StoredValue, store_value,
};

use crate::tauri_bridge;

type ResultCallback = StoredValue<Option<Box<dyn Fn(String)>>>;

fn store_init_data(session: &str, data: &str) {
    let Some(window) = web_sys::window() else { return };
    let Ok(Some(storage)) = window.local_storage() else { return };
    let key = format!("__tauri_init_{session}");
    let _ = storage.set_item(&key, data);
}

fn clear_init_data(session: &str) {
    let Some(window) = web_sys::window() else { return };
    let Ok(Some(storage)) = window.local_storage() else { return };
    let key = format!("__tauri_init_{session}");
    let _ = storage.remove_item(&key);
}

async fn open_native_window(
    session: String,
    on_result: ResultCallback,
    on_close: StoredValue<impl Fn() + 'static>,
    label: String,
    window_type_str: &'static str,
    title: String,
    size: (u32, u32),
) {
    // Register result listener (continuous — supports live updates like settings)
    let result_event = format!("result:{session}");
    if let Err(e) = tauri_bridge::listen_event(&result_event, move |payload| {
        on_result.with_value(|r| {
            if let Some(callback) = r {
                callback(payload);
            }
        });
    })
    .await
    {
        leptos::logging::error!("Failed to listen for result: {e}");
    }

    // Create the native window — init data is already in localStorage
    let url = format!("/?window={window_type_str}&session={session}");
    if let Err(e) = tauri_bridge::create_native_window(&label, &url, &title, size).await {
        leptos::logging::error!("Failed to create window: {e}");
        on_close.with_value(|f| f());
        return;
    }

    if let Err(e) =
        tauri_bridge::listen_window_close(&label, move || on_close.with_value(|f| f()))
    {
        leptos::logging::error!("Failed to listen for window close: {e}");
    }
}

/// A component that manages a native Tauri window instead of an HTML overlay.
/// When `is_open` becomes true, creates a native OS window. When false, closes it.
/// Renders nothing in the DOM.
///
/// Init data is passed via localStorage (keyed by session nonce) to avoid
/// event-based handshake race conditions.
#[component]
pub fn NativeWindow(
    #[prop(into)] is_open: MaybeSignal<bool>,
    title: Signal<String>,
    on_close: impl Fn() + 'static,
    window_type: &'static str,
    #[prop(into)] init_data: Signal<String>,
    #[prop(optional)] on_result: Option<Box<dyn Fn(String)>>,
    #[prop(default = (600, 400))] size: (u32, u32),
    #[prop(optional, into)] position_key: Option<String>,
    #[prop(optional, into)] update_data: Option<Signal<String>>,
) -> impl IntoView {
    let on_close = store_value(on_close);
    let on_result = store_value(on_result);
    let window_type_str = window_type;
    let label_base = position_key
        .as_deref()
        .unwrap_or(window_type)
        .to_string();

    let (current_session, set_current_session) = create_signal(Option::<String>::None);

    create_effect(move |prev_open: Option<bool>| {
        let currently_open = is_open.get();
        let was_open = prev_open.unwrap_or(false);

        if currently_open && !was_open {
            let session = uuid::Uuid::new_v4().to_string();
            set_current_session.set(Some(session.clone()));
            let label = format!("child-{label_base}");
            let current_title = title.get();
            let current_init_data = init_data.get();

            // Write init data to localStorage — child will read it on startup
            store_init_data(&session, &current_init_data);

            leptos::spawn_local(open_native_window(
                session,
                on_result,
                on_close,
                label,
                window_type_str,
                current_title,
                size,
            ));
        } else if !currently_open && was_open {
            if let Some(session) = current_session.get() {
                clear_init_data(&session);
            }
            set_current_session.set(None);
            let label = format!("child-{label_base}");
            leptos::spawn_local(async move {
                let _ = tauri_bridge::close_native_window(&label).await;
            });
        }

        currently_open
    });

    // Push update-data events to child when update_data signal changes
    if let Some(update_signal) = update_data {
        create_effect(move |prev: Option<String>| {
            let data = update_signal.get();
            let Some(session) = current_session.get() else {
                return data;
            };
            if prev.is_some() && !data.is_empty() {
                let event_name = format!("update-data:{session}");
                let data_for_emit = data.clone();
                leptos::spawn_local(async move {
                    let _ = tauri_bridge::emit_event(&event_name, &data_for_emit).await;
                });
            }
            data
        });
    }

    view! {}
}
