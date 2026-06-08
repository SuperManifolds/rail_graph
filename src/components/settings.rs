use crate::components::button::Button;
use crate::components::native_window::NativeWindow;
use crate::models::ProjectSettings;
use crate::window_protocol::{SettingsInit, SettingsResult};
use leptos::{component, create_signal, view, IntoView, Signal, SignalGet, SignalSet};

#[component]
pub fn Settings(
    settings: Signal<ProjectSettings>,
    set_settings: impl Fn(ProjectSettings) + 'static + Copy,
    #[prop(optional)] on_open_changelog: Option<impl Fn() + 'static + Copy>,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);
    let _ = on_open_changelog;

    let result_handler: Box<dyn Fn(String)> = Box::new(move |json: String| {
        match serde_json::from_str::<SettingsResult>(&json) {
            Ok(SettingsResult { settings: new_settings }) => {
                set_settings(new_settings);
            }
            Err(e) => {
                leptos::logging::error!("Failed to parse SettingsResult: {}", e);
            }
        }
    });

    view! {
        <Button
            class="import-button"
            on_click=leptos::Callback::new(move |_| set_is_open.set(true))
            shortcut_id="open_settings"
            title="Project Settings"
        >
            <i class="fa-solid fa-cog"></i>
        </Button>

        <NativeWindow
            is_open=Signal::derive(move || is_open.get())
            title=Signal::derive(|| "Settings".to_string())
            on_close=move || set_is_open.set(false)
            window_type="settings"
            init_data=Signal::derive(move || {
                serde_json::to_string(&SettingsInit {
                    settings: settings.get(),
                }).unwrap_or_default()
            })
            on_result=result_handler
            size=(600, 500)
            position_key="settings"
        />
    }
}
