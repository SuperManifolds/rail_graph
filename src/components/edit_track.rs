use crate::components::window::Window;
use crate::models::{RailwayGraph, Track, TrackDirection, Line};
use leptos::*;
use petgraph::graph::EdgeIndex;
use std::rc::Rc;

#[component]
pub fn EditTrack(
    editing_track: ReadSignal<Option<EdgeIndex>>,
    on_close: Rc<dyn Fn()>,
    on_save: Rc<dyn Fn(EdgeIndex, Vec<Track>)>,
    on_delete: Rc<dyn Fn(EdgeIndex)>,
    graph: ReadSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
) -> impl IntoView {
    let (tracks, set_tracks) = create_signal(Vec::<Track>::new());
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

                // Get station names
                if let Some((from, to)) = current_graph.graph.edge_endpoints(edge_idx) {
                    if let Some(from_name) = current_graph.get_station_name(from) {
                        set_from_station_name.set(from_name.to_string());
                    }
                    if let Some(to_name) = current_graph.get_station_name(to) {
                        set_to_station_name.set(to_name.to_string());
                    }
                }
            }

            // Find lines using this edge
            let edge_index = edge_idx.index();
            let affected: Vec<String> = current_lines
                .iter()
                .filter(|line| {
                    line.route.iter().any(|segment| segment.edge_index == edge_index)
                })
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
                on_save(edge_idx, current_tracks);
            }
        }
    };

    let handle_delete = move |_| {
        if let Some(edge_idx) = editing_track.get() {
            on_delete(edge_idx);
        }
    };

    let handle_add_track = move |_| {
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
            initial_size=(450.0, 450.0)
        >
            <div class="add-station-form">
                <div class="track-stations">
                    <strong>{move || from_station_name.get()}</strong>
                    " ↔ "
                    <strong>{move || to_station_name.get()}</strong>
                </div>

                {move || {
                    let affected = affected_lines.get();
                    if !affected.is_empty() {
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
                    } else {
                        view! {}.into_view()
                    }
                }}

                <div class="form-field">
                    <label>"Tracks"</label>
                    <div class="tracks-visual">
                        <div class="station-label station-top">{move || from_station_name.get()}</div>
                        <div class="tracks-horizontal">
                            {move || {
                                tracks.get().iter().enumerate().map(|(i, track)| {
                                    let direction = track.direction;
                                    view! {
                                        <div class="track-column">
                                            <div class="track-number">{i + 1}</div>
                                            <button
                                                class="direction-button"
                                                on:click=move |_| {
                                                    let new_dir = match direction {
                                                        TrackDirection::Bidirectional => TrackDirection::Forward,
                                                        TrackDirection::Forward => TrackDirection::Backward,
                                                        TrackDirection::Backward => TrackDirection::Bidirectional,
                                                    };
                                                    handle_change_direction(i, new_dir);
                                                }
                                                title=move || match direction {
                                                    TrackDirection::Bidirectional => "Bidirectional".to_string(),
                                                    TrackDirection::Forward => format!("{} → {}", from_station_name.get(), to_station_name.get()),
                                                    TrackDirection::Backward => format!("{} → {}", to_station_name.get(), from_station_name.get()),
                                                }
                                            >
                                                <i class=move || match direction {
                                                    TrackDirection::Bidirectional => "fa-solid fa-arrows-up-down",
                                                    TrackDirection::Forward => "fa-solid fa-arrow-down",
                                                    TrackDirection::Backward => "fa-solid fa-arrow-up",
                                                }></i>
                                            </button>
                                            {if tracks.get().len() > 1 {
                                                view! {
                                                    <button
                                                        class="remove-track-button-small"
                                                        on:click=move |_| handle_remove_track(i)
                                                        title="Remove track"
                                                    >
                                                        <i class="fa-solid fa-xmark"></i>
                                                    </button>
                                                }.into_view()
                                            } else {
                                                view! { <div class="track-spacer"></div> }.into_view()
                                            }}
                                        </div>
                                    }
                                }).collect::<Vec<_>>()
                            }}
                            <button class="add-track-button-inline" on:click=handle_add_track title="Add Track">
                                <i class="fa-solid fa-plus"></i>
                            </button>
                        </div>
                        <div class="station-label station-bottom">{move || to_station_name.get()}</div>
                    </div>
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
