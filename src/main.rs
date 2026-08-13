use nimby_graph::components::child_window_router::ChildWindowRouter;
use nimby_graph::App;
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = __railgraph_report_panic)]
    fn report_panic(msg: &str);
}

/// Installs a panic hook that keeps the standard console output and
/// additionally forwards the panic (message plus `file:line` location) to
/// Sentry. The Sentry forwarding is a no-op when the plugin global is absent
/// (see `static/sentry_bridge.js`).
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        report_panic(&info.to_string());
    }));
}

fn main() {
    install_panic_hook();

    let window = web_sys::window().expect("no global window");
    let search = window.location().search().unwrap_or_default();

    if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) {
        // Child dialog windows (edit track, settings, etc.)
        if let Some(window_type) = params.get("window") {
            let session = params.get("session").unwrap_or_default();
            leptos::mount_to_body(move || {
                leptos::view! {
                    <ChildWindowRouter window_type=window_type session=session />
                }
            });
            return;
        }

        // Secondary main windows (created by tab tear-off or "New Window")
        if params.get("main_window").is_some() {
            let window_id = params.get("window_id").unwrap_or_default();
            let initial_tab = params.get("initial_tab").unwrap_or_default();
            leptos::mount_to_body(move || {
                leptos::view! {
                    <App
                        window_id=window_id
                        is_secondary=true
                        initial_tab=initial_tab
                    />
                }
            });
            return;
        }
    }

    // Primary main window (all props use defaults)
    leptos::mount_to_body(move || {
        leptos::view! { <App /> }
    });
}
