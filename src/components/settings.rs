use leptos::{component, view, Signal, IntoView, create_signal, create_rw_signal, SignalGet, SignalSet};
use crate::components::window::Window;
use crate::components::button::Button;
use crate::components::tab_view::{TabView, TabPanel, Tab};
use crate::components::keyboard_shortcuts_editor::KeyboardShortcutsEditor;
use crate::components::duration_input::DurationInput;
use crate::models::{ProjectSettings, TrackHandedness};
use chrono::Duration;

#[component]
#[allow(clippy::too_many_lines)]
pub fn Settings(
    settings: Signal<ProjectSettings>,
    set_settings: impl Fn(ProjectSettings) + 'static + Copy,
    #[prop(optional)] on_open_changelog: Option<impl Fn() + 'static + Copy>,
) -> impl IntoView {
    let (is_open, set_is_open) = create_signal(false);
    let active_tab = create_rw_signal("project".to_string());

    let handle_handedness_change = move |handedness: TrackHandedness| {
        let current = settings.get();
        set_settings(ProjectSettings {
            track_handedness: handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: current.default_node_distance_grid_squares,
            minimum_separation: current.minimum_separation,
            station_margin: current.station_margin,
        });
    };

    let handle_node_distance_change = move |distance: f64| {
        let clamped_distance = distance.clamp(1.0, 20.0);
        let current = settings.get();
        set_settings(ProjectSettings {
            track_handedness: current.track_handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: clamped_distance,
            minimum_separation: current.minimum_separation,
            station_margin: current.station_margin,
        });
    };

    let handle_minimum_separation_change = move |duration: Duration| {
        let current = settings.get();
        set_settings(ProjectSettings {
            track_handedness: current.track_handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: current.default_node_distance_grid_squares,
            minimum_separation: duration,
            station_margin: current.station_margin,
        });
    };

    let handle_station_margin_change = move |duration: Duration| {
        let current = settings.get();
        set_settings(ProjectSettings {
            track_handedness: current.track_handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: current.default_node_distance_grid_squares,
            minimum_separation: current.minimum_separation,
            station_margin: duration,
        });
    };

    let tabs = vec![
        Tab {
            id: "project".to_string(),
            label: "Project Settings".to_string(),
        },
        Tab {
            id: "shortcuts".to_string(),
            label: "Keyboard Shortcuts".to_string(),
        },
    ];

    view! {
        <Button
            class="import-button"
            on_click=leptos::Callback::new(move |_| set_is_open.set(true))
            shortcut_id="open_settings"
            title="Project Settings"
        >
            <i class="fa-solid fa-cog"></i>
        </Button>

        <Window
            is_open=Signal::derive(move || is_open.get())
            title=Signal::derive(|| "Settings".to_string())
            on_close=move || set_is_open.set(false)
            position_key="settings"
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

                        <div class="settings-section">
                            <h3>"Layout"</h3>
                            <p class="section-description">
                                "Configure default spacing for station positioning in infrastructure editor"
                            </p>

                            <div class="form-field">
                                <label>
                                    "Default Node Distance "
                                    <span class="help-text">
                                        {move || {
                                            #[allow(clippy::cast_possible_truncation)]
                                            let grid_squares = settings.get().default_node_distance_grid_squares.round() as i32;
                                            format!("(grid squares, {} px)", grid_squares * 30)
                                        }}
                                    </span>
                                </label>
                                <input
                                    type="number"
                                    min="1"
                                    max="20"
                                    step="1"
                                    prop:value=move || {
                                        #[allow(clippy::cast_possible_truncation)]
                                        let grid_squares = settings.get().default_node_distance_grid_squares.round() as i32;
                                        grid_squares.to_string()
                                    }
                                    on:input=move |ev| {
                                        if let Ok(val) = leptos::event_target_value(&ev).parse::<f64>() {
                                            handle_node_distance_change(val);
                                        }
                                    }
                                />
                                <p class="help-text">
                                    "Affects auto-layout, alignment, and rotation operations. Range: 1-20. Default: 4 (120 px)."
                                </p>
                            </div>
                        </div>

                        <div class="settings-section">
                            <h3>"Train Buffers"</h3>
                            <p class="section-description">
                                "Configure timing buffers for conflict detection"
                            </p>

                            <div class="form-field">
                                <label>
                                    "Minimum Separation"
                                </label>
                                <DurationInput
                                    duration=Signal::derive(move || settings.get().minimum_separation)
                                    on_change=handle_minimum_separation_change
                                />
                                <p class="help-text">
                                    "Minimum time separation between trains."
                                </p>
                            </div>

                            <div class="form-field">
                                <label>
                                    "Station Crossing Margin"
                                </label>
                                <DurationInput
                                    duration=Signal::derive(move || settings.get().station_margin)
                                    on_change=handle_station_margin_change
                                />
                                <p class="help-text">
                                    "Time margin for determining if track intersections near stations are valid crossings."
                                </p>
                            </div>
                        </div>

                        <div class="settings-section">
                            <Button
                                on_click=leptos::Callback::new(move |_| {
                                    if let Some(handler) = on_open_changelog {
                                        handler();
                                        set_is_open.set(false);
                                    }
                                })
                            >
                                "About"
                            </Button>
                        </div>
                    </div>
                </TabPanel>

                <TabPanel when=Signal::derive(move || active_tab.get() == "shortcuts")>
                    <KeyboardShortcutsEditor />
                </TabPanel>
            </TabView>
        </Window>
    }
}
