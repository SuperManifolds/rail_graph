use crate::components::add_station_quick::{AddStationQuick, QuickEntryStation};
use crate::components::platform_editor::PlatformEditor;
use crate::models::{Platform, Track, TrackDirection};
use crate::window_protocol::{AddStationInit, AddStationResult, AddStationUpdate};
use leptos::{
    component, create_signal, event_target_checked, event_target_value, view, IntoView, SignalGet,
    SignalSet,
};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::rc::Rc;

#[component]
#[allow(clippy::too_many_lines)]
#[must_use]
pub fn AddStationChild(init: AddStationInit, session: String) -> impl IntoView {
    let (graph, _) = create_signal(init.graph);
    let (station_name, set_station_name) = create_signal(init.suggested_name);
    let (is_passing_loop, set_is_passing_loop) = create_signal(false);
    let (connect_to_station, set_connect_to_station) =
        create_signal(init.last_added_station.map(NodeIndex::new));
    let (platforms, set_platforms) = create_signal(vec![
        Platform {
            name: "1".to_string(),
        },
        Platform {
            name: "2".to_string(),
        },
    ]);

    // Quick entry mode signals
    let (quick_entry_mode, set_quick_entry_mode) = create_signal(false);
    let (station_entries, set_station_entries) = create_signal(vec![QuickEntryStation {
        name: String::new(),
        distance_from_previous: 0.0,
        is_passing_loop: false,
    }]);
    let (tracks, set_tracks) = create_signal(vec![Track {
        direction: TrackDirection::Bidirectional,
    }]);

    let (clicked_segment, set_clicked_segment) = create_signal(None::<EdgeIndex>);

    // Listen for update-data events from the main window (map clicks)
    {
        let session_for_listen = session.clone();
        leptos::spawn_local(async move {
            let event_name = format!("update-data:{session_for_listen}");
            match crate::tauri_bridge::listen_event(&event_name, move |payload| {
                if let Ok(update) = serde_json::from_str::<AddStationUpdate>(&payload) {
                    set_clicked_segment.set(update.clicked_edge_idx.map(EdgeIndex::new));
                }
            })
            .await
            {
                Ok(unlisten) => {
                    // Store unlisten handle to prevent GC; window close cleans up
                    std::mem::forget(unlisten);
                }
                Err(e) => {
                    leptos::logging::error!("Failed to listen for update-data: {}", e);
                }
            }
        });
    }

    // Settings signal for quick entry
    let (settings, _) = create_signal(crate::models::ProjectSettings {
        track_handedness: init.track_handedness,
        default_node_distance_grid_squares: init.default_node_distance_grid_squares,
        ..Default::default()
    });

    // Single-station add
    let session_add = session.clone();
    let handle_add = move |_| {
        let name = station_name.get();
        let current_platforms = platforms.get();
        if name.is_empty() || current_platforms.is_empty() {
            return;
        }

        let result = AddStationResult::Add {
            name,
            is_passing_loop: is_passing_loop.get(),
            connect_to: connect_to_station.get().map(NodeIndex::index),
            platforms: current_platforms,
        };
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_add}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    };

    // Batch add (from quick entry)
    let session_batch = session.clone();
    let on_add_batch = Rc::new(
        move |entries: Vec<QuickEntryStation>,
              connect_to: Option<NodeIndex>,
              plats: Vec<Platform>,
              trks: Vec<Track>| {
            let result = AddStationResult::AddBatch {
                entries,
                connect_to: connect_to.map(NodeIndex::index),
                platforms: plats,
                tracks: trks,
            };
            let json = serde_json::to_string(&result).unwrap_or_default();
            let event_name = format!("result:{session_batch}");
            leptos::spawn_local(async move {
                let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
            });
        },
    );

    let handle_cancel = move |_| {
        leptos::spawn_local(async move {
            crate::tauri_bridge::close_current_window().await;
        });
    };

    let on_close_for_quick = Rc::new(move || {
        leptos::spawn_local(async move {
            crate::tauri_bridge::close_current_window().await;
        });
    });

    view! {
        <div class="add-station-form">
            // Quick Entry Mode Toggle
            <div class="form-field">
                <label>
                    <input
                        type="checkbox"
                        prop:checked=move || quick_entry_mode.get()
                        on:change=move |ev| set_quick_entry_mode.set(event_target_checked(&ev))
                    />
                    " Quick Entry Mode"
                </label>
            </div>

            {
                let on_close_quick = on_close_for_quick.clone();
                let on_add_batch_for_quick = on_add_batch;
                let handle_add_for_normal = handle_add.clone();
                move || {
                    if quick_entry_mode.get() {
                        view! {
                            <AddStationQuick
                                on_close=on_close_quick.clone()
                                on_add_batch=on_add_batch_for_quick.clone()
                                graph=graph
                                connect_to_station=connect_to_station
                                set_connect_to_station=set_connect_to_station
                                clicked_segment=clicked_segment
                                platforms=platforms
                                set_platforms=set_platforms
                                station_entries=station_entries
                                set_station_entries=set_station_entries
                                tracks=tracks
                                set_tracks=set_tracks
                                settings=settings
                            />
                        }.into_view()
                    } else {
                        view! {
                            <div class="form-field">
                                <label>"Station Name"</label>
                                <input
                                    type="text"
                                    value=move || station_name.get()
                                    on:input=move |ev| set_station_name.set(event_target_value(&ev))
                                />
                            </div>
                            <div class="form-field">
                                <label>
                                    <input
                                        type="checkbox"
                                        checked=move || is_passing_loop.get()
                                        on:change=move |ev| set_is_passing_loop.set(event_target_checked(&ev))
                                    />
                                    " Passing Loop"
                                </label>
                            </div>
                            <PlatformEditor
                                platforms=platforms
                                set_platforms=set_platforms
                                is_passing_loop=is_passing_loop
                            />

                            <div class="form-field">
                                <label>"Connect to (optional)"</label>
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
                            <div class="form-buttons">
                                <button on:click=handle_cancel>"Cancel"</button>
                                <button class="primary" on:click={
                                    let handle_add_clone = handle_add_for_normal.clone();
                                    move |ev| handle_add_clone(ev)
                                }>"Add"</button>
                            </div>
                        }.into_view()
                    }
                }
            }
        </div>
    }
}
