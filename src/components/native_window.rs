use leptos::{
    component, create_effect, create_signal, view, IntoView, MaybeSignal, Signal,
    SignalGet, SignalSet, StoredValue, store_value,
};

use crate::tauri_bridge;

type ResultCallback = StoredValue<Option<Box<dyn Fn(String)>>>;

async fn open_native_window(
    session: String,
    init_data: String,
    on_result: ResultCallback,
    on_close: StoredValue<impl Fn() + 'static>,
    label: String,
    window_type_str: &'static str,
    title: String,
    size: (u32, u32),
) {
    let session_for_ready = session.clone();
    let init_data_for_send = init_data;
    let ready_event = format!("child-ready:{session_for_ready}");
    if let Err(e) = tauri_bridge::listen_event_once(&ready_event, move |_| {
        let session = session_for_ready.clone();
        let data = init_data_for_send.clone();
        leptos::spawn_local(async move {
            let init_event = format!("init-data:{session}");
            if let Err(e) = tauri_bridge::emit_event(&init_event, &data).await {
                leptos::logging::error!("Failed to emit init-data: {}", e);
            }
        });
    })
    .await
    {
        leptos::logging::error!("Failed to listen for child-ready: {}", e);
        return;
    }

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
        leptos::logging::error!("Failed to listen for result: {}", e);
    }

    let url = format!("index.html?window={window_type_str}&session={session}");
    if let Err(e) = tauri_bridge::create_native_window(&label, &url, &title, size).await {
        leptos::logging::error!("Failed to create window: {}", e);
        on_close.with_value(|f| f());
        return;
    }

    if let Err(e) =
        tauri_bridge::listen_window_close(&label, move || on_close.with_value(|f| f()))
    {
        leptos::logging::error!("Failed to listen for window close: {}", e);
    }
}

/// A component that manages a native Tauri window instead of an HTML overlay.
/// When `is_open` becomes true, creates a native OS window. When false, closes it.
/// Renders nothing in the DOM.
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

    // Track current session nonce so update_data effect can emit to the right child
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

            leptos::spawn_local(open_native_window(
                session,
                current_init_data,
                on_result,
                on_close,
                label,
                window_type_str,
                current_title,
                size,
            ));
        } else if !currently_open && was_open {
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
            // Skip the initial value (only send on subsequent changes)
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
