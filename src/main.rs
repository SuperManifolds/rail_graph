use nimby_graph::App;
use nimby_graph::components::child_window_router::ChildWindowRouter;

fn main() {
    console_error_panic_hook::set_once();

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
