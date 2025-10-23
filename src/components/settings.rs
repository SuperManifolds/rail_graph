use leptos::{component, view, Signal, IntoView, create_signal, create_rw_signal, SignalGet, SignalSet};
use crate::components::window::Window;
use crate::components::button::Button;
use crate::components::tab_view::{TabView, TabPanel, Tab};
use crate::models::{ProjectSettings, TrackHandedness};

#[component]
pub fn Settings(
    settings: Signal<ProjectSettings>,
    set_settings: impl Fn(ProjectSettings) + 'static + Copy,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);
    let active_tab = create_rw_signal("project".to_string());

    let handle_handedness_change = move |handedness: TrackHandedness| {
        set_settings(ProjectSettings {
            track_handedness: handedness,
        });
    };

    let tabs = vec![
        Tab {
            id: "project".to_string(),
            label: "Project Settings".to_string(),
        },
    ];

    view! {
        <Button
            class="import-button"
            on_click=leptos::Callback::new(move |_| set_is_open.set(true))
            title="Project Settings"
        >
            <i class="fa-solid fa-cog"></i>
        </Button>

        <Window
            is_open=Signal::derive(move || is_open.get())
            title=Signal::derive(|| "Settings".to_string())
            on_close=move || set_is_open.set(false)
        >
            <TabView tabs=tabs active_tab=active_tab>
                <TabPanel when=Signal::derive(move || active_tab.get() == "project")>
                    <div class="settings-content">
                        <div class="settings-section">
                            <h3>"Track Operation"</h3>
                            <p class="section-description">
                                "Configure default platform and track direction assignments"
                            </p>

                            <div class="radio-group">
                                <label class="radio-label">
                                    <input
                                        type="radio"
                                        name="handedness"
                                        checked=move || matches!(settings.get().track_handedness, TrackHandedness::RightHand)
                                        on:change=move |_| handle_handedness_change(TrackHandedness::RightHand)
                                    />
                                    <span class="radio-text">
                                        <strong>"Right-hand traffic"</strong>
                                        <span class="radio-description">
                                            "Trains drive on the right (forward trains use right platforms, right tracks go forward)"
                                        </span>
                                    </span>
                                </label>

                                <label class="radio-label">
                                    <input
                                        type="radio"
                                        name="handedness"
                                        checked=move || matches!(settings.get().track_handedness, TrackHandedness::LeftHand)
                                        on:change=move |_| handle_handedness_change(TrackHandedness::LeftHand)
                                    />
                                    <span class="radio-text">
                                        <strong>"Left-hand traffic"</strong>
                                        <span class="radio-description">
                                            "Trains drive on the left (forward trains use left platforms, left tracks go forward)"
                                        </span>
                                    </span>
                                </label>
                            </div>
                        </div>
                    </div>
                </TabPanel>
            </TabView>
        </Window>
    }
}
