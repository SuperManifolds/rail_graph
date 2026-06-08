use nimby_graph::App;
use nimby_graph::components::child_window_router::ChildWindowRouter;

fn main() {
    console_error_panic_hook::set_once();

    let window = web_sys::window().expect("no global window");
    let search = window.location().search().unwrap_or_default();

    if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) {
        if let Some(window_type) = params.get("window") {
            let session = params.get("session").unwrap_or_default();
            leptos::mount_to_body(move || {
                leptos::view! {
                    <ChildWindowRouter window_type=window_type session=session />
                }
            });
            return;
        }
    }

    leptos::mount_to_body(App);
}
