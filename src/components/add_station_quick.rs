use crate::components::platform_editor::PlatformEditor;
use crate::components::track_editor::TrackEditor;
use crate::models::{RailwayGraph, Platform, Track};
use crate::import::shared::create_tracks_with_count;
use leptos::{component, create_signal, event_target_checked, event_target_value, IntoView, ReadSignal, SignalGet, SignalSet, SignalUpdate, view, For, WriteSignal};
use leptos::wasm_bindgen::JsCast;
use petgraph::stable_graph::NodeIndex;
use std::rc::Rc;
use web_sys::KeyboardEvent;

#[derive(Clone, Debug)]
pub struct QuickEntryStation {
    pub name: String,
    pub distance_from_previous: f64,
    pub is_passing_loop: bool,
}

type AddStationsBatchCallback = Rc<dyn Fn(Vec<QuickEntryStation>, Option<NodeIndex>, Vec<Platform>, Vec<Track>)>;

#[component]
#[allow(clippy::too_many_lines)]
pub fn AddStationQuick(
    on_close: Rc<dyn Fn()>,
    on_add_batch: AddStationsBatchCallback,
    graph: ReadSignal<RailwayGraph>,
    connect_to_station: ReadSignal<Option<NodeIndex>>,
    set_connect_to_station: WriteSignal<Option<NodeIndex>>,
    clicked_segment: ReadSignal<Option<petgraph::stable_graph::EdgeIndex>>,
    platforms: ReadSignal<Vec<Platform>>,
    set_platforms: WriteSignal<Vec<Platform>>,
    station_entries: ReadSignal<Vec<QuickEntryStation>>,
    set_station_entries: WriteSignal<Vec<QuickEntryStation>>,
    tracks: ReadSignal<Vec<Track>>,
    set_tracks: WriteSignal<Vec<Track>>,
    settings: ReadSignal<crate::models::ProjectSettings>,
) -> impl IntoView {
    // Validation function for enabling/disabling the add button
    let is_valid = move || {
        let entries = station_entries.get();
        let current_platforms = platforms.get();
        let current_tracks = tracks.get();

        if entries.is_empty() || current_platforms.is_empty() || current_tracks.is_empty() {
            return false;
        }

        // Allow the last entry to be empty if there are multiple entries and it's completely empty
        let entries_to_check = if entries.len() > 1 {
            if let Some(last) = entries.last() {
                if last.name.trim().is_empty() && last.distance_from_previous == 0.0 {
                    &entries[..entries.len() - 1]
                } else {
                    &entries[..]
                }
            } else {
                &entries[..]
            }
        } else {
            &entries[..]
        };

        // All non-last entries must have names
        entries_to_check.iter().all(|e| !e.name.trim().is_empty())
    };

    let on_close_clone = on_close.clone();
    let handle_add_batch = move |_| {
        let mut entries = station_entries.get();
        let current_platforms = platforms.get();
        let current_tracks = tracks.get();

        // Remove the last entry if it's completely empty and there are multiple entries
        if entries.len() > 1 {
            if let Some(last) = entries.last() {
                if last.name.trim().is_empty() && last.distance_from_previous == 0.0 {
                    entries.pop();
                }
            }
        }

        // Validate: all entries must have names
        let all_valid = entries.iter().all(|e| !e.name.trim().is_empty());

        if all_valid && !entries.is_empty() && !current_platforms.is_empty() && !current_tracks.is_empty() {
            on_add_batch(entries, connect_to_station.get(), current_platforms, current_tracks);
            on_close_clone();
        }
    };

    let (from_station_name, _) = create_signal("First station".to_string());
    let (to_station_name, _) = create_signal("Last station".to_string());

    let station_entries_with_index = move || {
        station_entries.get()
            .into_iter()
            .enumerate()
            .collect::<Vec<_>>()
    };

    view! {
            // Station list
            <div class="form-section">
                <h3>"Stations"</h3>
            <div class="station-list">
                <For
                    each=station_entries_with_index
                    key=|(idx, _)| *idx
                    let:data
                >
                {
                    let (idx, entry) = data;
                        view! {
                            <div class="quick-station-row">
                                <div class="station-name-field">
                                    <input
                                        type="text"
                                        placeholder="Station Name"
                                        value=entry.name.clone()
                                        on:input=move |ev| {
                                            let new_name = event_target_value(&ev);
                                            set_station_entries.update(|entries| {
                                                if let Some(e) = entries.get_mut(idx) {
                                                    e.name = new_name;
                                                }
                                            });
                                        }
                                        on:keydown=move |ev: KeyboardEvent| {
                                            if ev.key() == "Enter" {
                                                ev.prevent_default();
                                                // Focus distance field - would need ref
                                            }
                                        }
                                    />
                                </div>
                                <div class="station-distance-field">
                                    <input
                                        type="number"
                                        placeholder="Distance (km)"
                                        value=if entry.distance_from_previous == 0.0 { String::new() } else { entry.distance_from_previous.to_string() }
                                        on:input=move |ev| {
                                            let val = event_target_value(&ev);
                                            if let Ok(distance) = val.parse::<f64>() {
                                                set_station_entries.update(|entries| {
                                                    if let Some(e) = entries.get_mut(idx) {
                                                        e.distance_from_previous = distance;
                                                    }
                                                });
                                            } else if val.is_empty() {
                                                set_station_entries.update(|entries| {
                                                    if let Some(e) = entries.get_mut(idx) {
                                                        e.distance_from_previous = 0.0;
                                                    }
                                                });
                                            }
                                        }
                                        on:keydown=move |ev: KeyboardEvent| {
                                            if ev.key() == "Enter" {
                                                ev.prevent_default();
                                                // Add new row
                                                set_station_entries.update(|entries| {
                                                    entries.push(QuickEntryStation {
                                                        name: String::new(),
                                                        distance_from_previous: 0.0,
                                                        is_passing_loop: false,
                                                    });
                                                });

                                                // Focus the first input of the new row after DOM updates
                                                if let Some(target) = ev.target() {
                                                    if let Ok(input_el) = target.dyn_into::<web_sys::HtmlElement>() {
                                                        if let Some(row) = input_el.closest(".quick-station-row").ok().flatten() {
                                                            if let Some(next_row) = row.next_element_sibling() {
                                                                // Use requestAnimationFrame to wait for DOM update
                                                                let callback = leptos::wasm_bindgen::closure::Closure::once(move || {
                                                                    if let Some(first_input) = next_row.query_selector("input[type='text']").ok().flatten() {
                                                                        if let Ok(input) = first_input.dyn_into::<web_sys::HtmlInputElement>() {
                                                                            let _ = input.focus();
                                                                        }
                                                                    }
                                                                });
                                                                if let Some(window) = web_sys::window() {
                                                                    let _ = window.request_animation_frame(callback.as_ref().unchecked_ref());
                                                                    callback.forget();
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    />
                                </div>
                                <label>
                                    <input
                                        type="checkbox"
                                        checked=entry.is_passing_loop
                                        on:change=move |ev| {
                                            let checked = event_target_checked(&ev);
                                            set_station_entries.update(|entries| {
                                                if let Some(e) = entries.get_mut(idx) {
                                                    e.is_passing_loop = checked;
                                                }
                                            });
                                        }
                                    />
                                    " Loop"
                                </label>
                                <button
                                    class="danger"
                                    on:click=move |_| {
                                        set_station_entries.update(|entries| {
                                            if entries.len() > 1 {
                                                entries.remove(idx);
                                            }
                                        });
                                    }
                                    disabled=move || station_entries.get().len() <= 1
                                    title="Remove station"
                                >
                                    <i class="fa-solid fa-xmark"></i>
                                </button>
                            </div>
                        }
                }
                </For>
                <button
                    on:click=move |_| {
                        set_station_entries.update(|entries| {
                            entries.push(QuickEntryStation {
                                name: String::new(),
                                distance_from_previous: 0.0,
                                is_passing_loop: false,
                            });
                        });
                    }
                >
                    <i class="fa-solid fa-plus"></i>
                    " Add Station"
                </button>
            </div>
            </div>

            // Shared settings
            <div class="form-section">
                <h3>"Shared Settings"</h3>
                <p class="help-text">"These settings apply to all stations being added"</p>

                <PlatformEditor
                    platforms=platforms
                    set_platforms=set_platforms
                    is_passing_loop=create_signal(false).0
                />

                <h4>"Tracks"</h4>
                <TrackEditor
                    tracks=tracks
                    from_station_name=from_station_name
                    to_station_name=to_station_name
                    on_add_track=move || {
                        set_tracks.update(|t| {
                            let new_count = t.len() + 1;
                            let handedness = settings.get().track_handedness;
                            *t = create_tracks_with_count(new_count, handedness);
                        });
                    }
                    on_remove_track=move |_| {
                        set_tracks.update(|t| {
                            if t.len() > 1 {
                                let new_count = t.len() - 1;
                                let handedness = settings.get().track_handedness;
                                *t = create_tracks_with_count(new_count, handedness);
                            }
                        });
                    }
                    on_change_direction=move |idx, direction| {
                        set_tracks.update(|t| {
                            if let Some(track) = t.get_mut(idx) {
                                track.direction = direction;
                            }
                        });
                    }
                />

                {move || {
                    if clicked_segment.get().is_none() {
                        view! {
                            <div class="form-field">
                                <label>"Connect First Station To"</label>
                                <select
                                    prop:value=move || {
                                        connect_to_station.get().and_then(|selected_idx| {
                                            let current_graph = graph.get();
                                            current_graph.graph.node_indices()
                                                .enumerate()
                                                .find(|(_, idx)| *idx == selected_idx)
                                                .map(|(i, _)| i.to_string())
                                        }).unwrap_or_default()
                                    }
                                    on:change=move |ev| {
                                        let value = event_target_value(&ev);
                                        if value.is_empty() {
                                            set_connect_to_station.set(None);
                                        } else if let Ok(array_idx) = value.parse::<usize>() {
                                            let current_graph = graph.get();
                                            let stations: Vec<NodeIndex> = current_graph.graph.node_indices().collect();
                                            if let Some(&node_idx) = stations.get(array_idx) {
                                                set_connect_to_station.set(Some(node_idx));
                                            }
                                        }
                                    }
                                >
                                    <option value="">"None"</option>
                                    {move || {
                                        let current_graph = graph.get();
                                        current_graph.graph.node_indices().enumerate().filter_map(|(i, idx)| {
                                            current_graph.graph.node_weight(idx).map(|node| {
                                                let name = node.display_name();
                                                view! {
                                                    <option value=i.to_string()>{name}</option>
                                                }
                                            })
                                        }).collect::<Vec<_>>()
                                    }}
                                </select>
                            </div>
                        }.into_view()
                    } else {
                        view! { <div></div> }.into_view()
                    }
                }}
            </div>

            <div class="form-buttons">
                <button on:click=move |_| on_close()>"Cancel"</button>
                <button class="primary" on:click=handle_add_batch disabled=move || !is_valid()>"Add All Stations"</button>
            </div>
    }
}
