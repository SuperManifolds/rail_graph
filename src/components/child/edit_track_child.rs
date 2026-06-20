use crate::components::track_editor::TrackEditor;
use crate::import::shared::create_tracks_with_count;
use crate::models::TrackDirection;
use crate::window_protocol::{EditTrackInit, EditTrackResult};
use leptos::{
    component, create_signal, event_target_value, view, IntoView, SignalGet, SignalSet, SignalUpdate,
};

#[component]
#[must_use]
pub fn EditTrackChild(init: EditTrackInit, session: String) -> impl IntoView {
    let edge_idx = init.edge_idx;
    let handedness = init.track_handedness;
    let (tracks, set_tracks) = create_signal(init.tracks);
    let (distance, set_distance) = create_signal(
        init.distance.map(|d| d.to_string()).unwrap_or_default(),
    );
    let (from_station_name, _) = create_signal(init.from_station_name);
    let (to_station_name, _) = create_signal(init.to_station_name);
    let affected_lines = init.affected_lines;

    let session_save = session.clone();
    let handle_save = move |_| {
        let current_tracks = tracks.get();
        if current_tracks.is_empty() {
            return;
        }
        let parsed_distance = distance
            .get()
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|d| *d > 0.0);

        let result = EditTrackResult::Save {
            edge_idx,
            tracks: current_tracks,
            distance: parsed_distance,
        };
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_save}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    };

    let session_delete = session.clone();
    let handle_delete = move |_| {
        let result = EditTrackResult::Delete { edge_idx };
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_delete}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    };

    let handle_cancel = move |_| {
        leptos::spawn_local(async move {
            crate::tauri_bridge::close_current_window().await;
        });
    };

    let handle_add_track = move || {
        set_tracks.update(|t| {
            let new_count = t.len() + 1;
            *t = create_tracks_with_count(new_count, handedness);
        });
    };

    let handle_remove_track = move |_index: usize| {
        set_tracks.update(|t| {
            if t.len() > 1 {
                let new_count = t.len() - 1;
                *t = create_tracks_with_count(new_count, handedness);
            }
        });
    };

    let handle_change_direction = move |index: usize, new_direction: TrackDirection| {
        set_tracks.update(|t| {
            if index < t.len() {
                t[index].direction = new_direction;
            }
        });
    };

    view! {
        <div class="add-station-form">
            <div class="track-stations">
                <strong>{move || from_station_name.get()}</strong>
                " ↔ "
                <strong>{move || to_station_name.get()}</strong>
            </div>

            {if affected_lines.is_empty() {
                leptos::view! {}.into_view()
            } else {
                let lines_text = affected_lines.join(", ");
                leptos::view! {
                    <div class="track-warning">
                        <i class="fa-solid fa-triangle-exclamation"></i>
                        <div class="warning-content">
                            <strong>"Warning:"</strong>
                            " Changes to this track will affect the following lines: "
                            <span class="affected-lines">{lines_text}</span>
                            <div class="warning-note">
                                "These lines may need to be updated if track directions no longer match their routes."
                            </div>
                        </div>
                    </div>
                }.into_view()
            }}

            <div class="form-field">
                <label>"Distance (km, optional)"</label>
                <input
                    type="text"
                    placeholder="e.g., 5.2"
                    prop:value=move || distance.get()
                    on:input=move |ev| set_distance.set(event_target_value(&ev))
                />
            </div>

            <div class="form-field">
                <label>"Tracks"</label>
                <TrackEditor
                    tracks=tracks
                    from_station_name=from_station_name
                    to_station_name=to_station_name
                    on_add_track=handle_add_track
                    on_remove_track=handle_remove_track
                    on_change_direction=handle_change_direction
                />
            </div>

            <div class="form-buttons">
                <button class="danger" on:click=handle_delete>"Delete Track"</button>
                <div class="flex-spacer"></div>
                <button on:click=handle_cancel>"Cancel"</button>
                <button class="primary" on:click=handle_save>"Save"</button>
            </div>
        </div>
    }
}
