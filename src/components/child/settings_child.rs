use crate::components::duration_input::DurationInput;
use crate::components::keyboard_shortcuts_editor::KeyboardShortcutsEditor;
use crate::components::tab_view::{Tab, TabPanel, TabView};
use crate::models::{ProjectSettings, TrackHandedness};
use crate::window_protocol::{SettingsInit, SettingsResult};
use chrono::Duration;
use leptos::{
    component, create_rw_signal, create_signal, view, IntoView, Signal, SignalGet, SignalSet,
};

#[component]
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn SettingsChild(init: SettingsInit, session: String) -> impl IntoView {
    let (settings, set_settings) = create_signal(init.settings);
    let active_tab = create_rw_signal("project".to_string());

    let handle_handedness_change = move |handedness: TrackHandedness| {
        let current = settings.get();
        set_settings.set(ProjectSettings {
            track_handedness: handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: current.default_node_distance_grid_squares,
            minimum_separation: current.minimum_separation,
            station_margin: current.station_margin,
            ignore_same_direction_platform_conflicts: current
                .ignore_same_direction_platform_conflicts,
        });
    };

    let handle_node_distance_change = move |distance: f64| {
        let clamped_distance = distance.clamp(1.0, 20.0);
        let current = settings.get();
        set_settings.set(ProjectSettings {
            track_handedness: current.track_handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: clamped_distance,
            minimum_separation: current.minimum_separation,
            station_margin: current.station_margin,
            ignore_same_direction_platform_conflicts: current
                .ignore_same_direction_platform_conflicts,
        });
    };

    let handle_minimum_separation_change = move |duration: Duration| {
        let current = settings.get();
        set_settings.set(ProjectSettings {
            track_handedness: current.track_handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: current.default_node_distance_grid_squares,
            minimum_separation: duration,
            station_margin: current.station_margin,
            ignore_same_direction_platform_conflicts: current
                .ignore_same_direction_platform_conflicts,
        });
    };

    let handle_station_margin_change = move |duration: Duration| {
        let current = settings.get();
        set_settings.set(ProjectSettings {
            track_handedness: current.track_handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: current.default_node_distance_grid_squares,
            minimum_separation: current.minimum_separation,
            station_margin: duration,
            ignore_same_direction_platform_conflicts: current
                .ignore_same_direction_platform_conflicts,
        });
    };

    let handle_ignore_same_direction_change = move |checked: bool| {
        let current = settings.get();
        set_settings.set(ProjectSettings {
            track_handedness: current.track_handedness,
            line_sort_mode: current.line_sort_mode,
            default_node_distance_grid_squares: current.default_node_distance_grid_squares,
            minimum_separation: current.minimum_separation,
            station_margin: current.station_margin,
            ignore_same_direction_platform_conflicts: checked,
        });
    };

    // Emit settings on every change for live updates
    let session_for_effect = session.clone();
    leptos::create_effect(move |prev: Option<ProjectSettings>| {
        let current = settings.get();
        if prev.is_some() {
            let result = SettingsResult {
                settings: current.clone(),
            };
            let json = serde_json::to_string(&result).unwrap_or_default();
            let event_name = format!("result:{session_for_effect}");
            leptos::spawn_local(async move {
                let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
            });
        }
        current
    });

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
        <div class="child-window-content">
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

                            <div>
                                <label class="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked=move || settings.get().ignore_same_direction_platform_conflicts
                                        on:change=move |ev| handle_ignore_same_direction_change(leptos::event_target_checked(&ev))
                                    />
                                    <span>"Ignore platform conflicts for same-direction arrivals"</span>
                                </label>
                                <p class="help-text">
                                    "When enabled, trains arriving at the same platform from the same track will not generate conflicts."
                                </p>
                            </div>
                        </div>

                        <p class="help-text settings-live-note">"Changes apply immediately."</p>
                    </div>
                </TabPanel>

                <TabPanel when=Signal::derive(move || active_tab.get() == "shortcuts")>
                    <KeyboardShortcutsEditor />
                </TabPanel>
            </TabView>
        </div>
    }
}
