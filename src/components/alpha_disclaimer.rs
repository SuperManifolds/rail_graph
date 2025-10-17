use leptos::{component, view, IntoView, create_signal, SignalGet, SignalSet, Signal};
use crate::components::window::Window;

const DISCLAIMER_KEY: &str = "rail_graph_disclaimer_accepted";

#[component]
#[must_use]
pub fn AlphaDisclaimer() -> impl IntoView {
    let (is_open, set_is_open) = create_signal(should_show_disclaimer());

    let on_accept = move || {
        mark_disclaimer_accepted();
        set_is_open.set(false);
    };

    view! {
        {move || if is_open.get() {
            view! {
                <div class="disclaimer-overlay">
                    <Window
                        is_open=Signal::derive(|| true)
                        title=Signal::derive(|| "Alpha Version Disclaimer".to_string())
                        on_close=move || {}
                    >
                        <div class="disclaimer-content">
                            <p>"This is an alpha version of the Railway Time Graph application."</p>
                            <p class="disclaimer-warning">
                                <strong>"Important: "</strong>
                                "You may lose project data. Project file compatibility is not guaranteed between versions."
                            </p>
                            <p>"Please save backups of your work regularly using the export functionality."</p>
                            <div class="disclaimer-feedback">
                                <p><strong>"Issues and Feedback:"</strong></p>
                                <ul>
                                    <li>
                                        "Report issues at: "
                                        <a href="https://github.com/SuperManifolds/rail_graph/issues" target="_blank" rel="noopener noreferrer">
                                            "github.com/SuperManifolds/rail_graph/issues"
                                        </a>
                                    </li>
                                    <li>
                                        "DM Alex (supermanifolds) in the NIMBY Rails Discord"
                                    </li>
                                </ul>
                            </div>
                            <div class="disclaimer-buttons">
                                <button class="primary" on:click=move |_| on_accept()>
                                    "I Understand"
                                </button>
                            </div>
                        </div>
                    </Window>
                </div>
            }.into_view()
        } else {
            ().into_view()
        }}
    }
}

fn should_show_disclaimer() -> bool {
    let Some(window) = web_sys::window() else { return false };
    let Ok(Some(storage)) = window.local_storage() else { return false };

    match storage.get_item(DISCLAIMER_KEY) {
        Ok(Some(value)) => value != "true",
        _ => true, // Show if not set or error
    }
}

fn mark_disclaimer_accepted() {
    let Some(window) = web_sys::window() else { return };
    let Ok(Some(storage)) = window.local_storage() else { return };

    let _ = storage.set_item(DISCLAIMER_KEY, "true");
}
