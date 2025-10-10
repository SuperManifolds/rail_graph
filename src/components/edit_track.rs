use crate::components::window::Window;
use crate::components::track_editor::TrackEditor;
use crate::models::{RailwayGraph, Track, TrackDirection, Line};
use leptos::{component, create_effect, create_signal, event_target_value, IntoView, ReadSignal, Signal, SignalGet, SignalSet, SignalUpdate, view};
use petgraph::graph::EdgeIndex;
use std::rc::Rc;

type SaveTrackCallback = Rc<dyn Fn(EdgeIndex, Vec<Track>, Option<f64>)>;

#[component]
pub fn EditTrack(
    editing_track: ReadSignal<Option<EdgeIndex>>,
    on_close: Rc<dyn Fn()>,
    on_save: SaveTrackCallback,
    on_delete: Rc<dyn Fn(EdgeIndex)>,
    graph: ReadSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
) -> impl IntoView {
    let (tracks, set_tracks) = create_signal(Vec::<Track>::new());
    let (distance, set_distance) = create_signal(String::new());
    let (from_station_name, set_from_station_name) = create_signal(String::new());
    let (to_station_name, set_to_station_name) = create_signal(String::new());
    let (affected_lines, set_affected_lines) = create_signal(Vec::<String>::new());

    // Load current track data when dialog opens
    create_effect(move |_| {
        if let Some(edge_idx) = editing_track.get() {
            let current_graph = graph.get();
            let current_lines = lines.get();

            if let Some(track_segment) = current_graph.graph.edge_weight(edge_idx) {
                set_tracks.set(track_segment.tracks.clone());

                // Load distance if available
                set_distance.set(track_segment.distance.map(|d| d.to_string()).unwrap_or_default());
            }

            // Get station names
            if let Some((from, to)) = current_graph.graph.edge_endpoints(edge_idx) {
                if let Some(from_name) = current_graph.get_station_name(from) {
                    set_from_station_name.set(from_name.to_string());
                }
                if let Some(to_name) = current_graph.get_station_name(to) {
                    set_to_station_name.set(to_name.to_string());
                }
            }

            // Find lines using this edge
            let edge_index = edge_idx.index();
            let affected: Vec<String> = current_lines
                .iter()
                .filter(|line| line.uses_edge(edge_index))
                .map(|line| line.id.clone())
                .collect();
            set_affected_lines.set(affected);
        }
    });

    let on_close_clone = on_close.clone();
    let handle_save = move |_| {
        if let Some(edge_idx) = editing_track.get() {
            let current_tracks = tracks.get();
            if !current_tracks.is_empty() {
                // Parse distance, treating empty string as None
                let parsed_distance = distance.get()
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|d| *d > 0.0); // Only accept positive distances

                on_save(edge_idx, current_tracks, parsed_distance);
            }
        }
    };

    let handle_delete = move |_| {
        if let Some(edge_idx) = editing_track.get() {
            on_delete(edge_idx);
        }
    };

    let handle_add_track = move || {
        set_tracks.update(|t| {
            t.push(Track {
                direction: TrackDirection::Bidirectional,
            });
        });
    };

    let handle_remove_track = move |index: usize| {
        set_tracks.update(|t| {
            if t.len() > 1 {
                t.remove(index);
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

    let is_open = Signal::derive(move || editing_track.get().is_some());

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Edit Track".to_string())
            on_close=move || on_close_clone()
        >
            <div class="add-station-form">
                <div class="track-stations">
                    <strong>{move || from_station_name.get()}</strong>
                    " ↔ "
                    <strong>{move || to_station_name.get()}</strong>
                </div>

                {move || {
                    let affected = affected_lines.get();
                    if affected.is_empty() {
                        view! {}.into_view()
                    } else {
                        view! {
                            <div class="track-warning">
                                <i class="fa-solid fa-triangle-exclamation"></i>
                                <div class="warning-content">
                                    <strong>"Warning:"</strong>
                                    " Changes to this track will affect the following lines: "
                                    <span class="affected-lines">{affected.join(", ")}</span>
                                    <div class="warning-note">
                                        "These lines may need to be updated if track directions no longer match their routes."
                                    </div>
                                </div>
                            </div>
                        }.into_view()
                    }
                }}

                <div class="form-field">
                    <label>"Distance (km, optional)"</label>
                    <input
                        type="text"
                        placeholder="e.g., 5.2"
                        value=move || distance.get()
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
                    <div style="flex: 1;"></div>
                    <button on:click=move |_| on_close()>"Cancel"</button>
                    <button class="primary" on:click=handle_save>"Save"</button>
                </div>
            </div>
        </Window>
    }
}
