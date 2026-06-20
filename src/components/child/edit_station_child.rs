use crate::components::connect_to_station::ConnectToStation;
use crate::components::edit_station::{
    load_connected_tracks, ConnectedTrack, TrackDefaultsCallback, TrackPlatformSelect,
};
use crate::components::platform_editor::PlatformEditor;
use crate::models::{Track, TrackDirection, Tracks};
use crate::window_protocol::{EditStationInit, EditStationResult, TrackDefaultChange};
use leptos::{
    component, create_signal, event_target_checked, event_target_value, view, For, IntoView,
    SignalGet, SignalGetUntracked, SignalSet,
};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::rc::Rc;

fn build_track_defaults_handler(
    graph: leptos::ReadSignal<crate::models::RailwayGraph>,
    set_graph: leptos::WriteSignal<crate::models::RailwayGraph>,
    track_default_changes: leptos::ReadSignal<Vec<TrackDefaultChange>>,
    set_track_default_changes: leptos::WriteSignal<Vec<TrackDefaultChange>>,
) -> TrackDefaultsCallback {
    Rc::new(
        move |edge_idx: EdgeIndex,
              source_platform: Option<usize>,
              target_platform: Option<usize>| {
            let mut current_graph = graph.get();
            if let Some(track_segment) = current_graph.graph.edge_weight_mut(edge_idx) {
                if let Some(src) = source_platform {
                    track_segment.default_platform_source = Some(src);
                }
                if let Some(tgt) = target_platform {
                    track_segment.default_platform_target = Some(tgt);
                }
            }
            set_graph.set(current_graph);

            // Record the change
            let mut changes = track_default_changes.get_untracked();
            if let Some(existing) = changes.iter_mut().find(|c| c.edge_idx == edge_idx.index()) {
                if source_platform.is_some() {
                    existing.default_platform_source = source_platform;
                }
                if target_platform.is_some() {
                    existing.default_platform_target = target_platform;
                }
            } else {
                changes.push(TrackDefaultChange {
                    edge_idx: edge_idx.index(),
                    default_platform_source: source_platform,
                    default_platform_target: target_platform,
                });
            }
            set_track_default_changes.set(changes);
        },
    )
}

fn build_add_connection_handler(
    station_node_idx: NodeIndex,
    graph: leptos::ReadSignal<crate::models::RailwayGraph>,
    set_graph: leptos::WriteSignal<crate::models::RailwayGraph>,
    set_connected_tracks: leptos::WriteSignal<Vec<ConnectedTrack>>,
    new_connections: leptos::ReadSignal<Vec<usize>>,
    set_new_connections: leptos::WriteSignal<Vec<usize>>,
) -> Rc<dyn Fn(NodeIndex)> {
    Rc::new(move |connect_idx: NodeIndex| {
        let mut current_graph = graph.get();
        current_graph.add_track(
            station_node_idx,
            connect_idx,
            vec![Track {
                direction: TrackDirection::Bidirectional,
            }],
            None,
        );
        set_connected_tracks.set(load_connected_tracks(station_node_idx, &current_graph));
        set_graph.set(current_graph);

        let mut conns = new_connections.get_untracked();
        conns.push(connect_idx.index());
        set_new_connections.set(conns);
    })
}

#[component]
#[must_use]
pub fn EditStationChild(init: EditStationInit, session: String) -> impl IntoView {
    let station_idx_raw = init.station_idx;
    let station_node_idx = NodeIndex::new(station_idx_raw);

    let (graph, set_graph) = create_signal(init.graph);
    let (editing_station, _) = create_signal(Some(station_node_idx));
    let (station_name, set_station_name) = create_signal(init.station_name);
    let (is_passing_loop, set_is_passing_loop) = create_signal(init.is_passing_loop);
    let (platforms, set_platforms) = create_signal(init.platforms);

    let initial_tracks = load_connected_tracks(station_node_idx, &graph.get_untracked());
    let (connected_tracks, set_connected_tracks) = create_signal(initial_tracks);

    let (track_default_changes, set_track_default_changes) =
        create_signal(Vec::<TrackDefaultChange>::new());
    let (new_connections, set_new_connections) = create_signal(Vec::<usize>::new());

    let on_update_track_defaults =
        build_track_defaults_handler(graph, set_graph, track_default_changes, set_track_default_changes);

    let handle_add_connection = build_add_connection_handler(
        station_node_idx,
        graph,
        set_graph,
        set_connected_tracks,
        new_connections,
        set_new_connections,
    );

    let session_save = session.clone();
    let handle_save = move |_| {
        let name = station_name.get();
        let current_platforms = platforms.get();
        if name.is_empty() || current_platforms.is_empty() {
            return;
        }

        let result = EditStationResult::Save {
            station_idx: station_idx_raw,
            name,
            is_passing_loop: is_passing_loop.get(),
            platforms: current_platforms,
            track_defaults: track_default_changes.get(),
            new_connections: new_connections.get(),
        };
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_save}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    };

    let session_delete = session.clone();
    let handle_delete = move |_| {
        let result = EditStationResult::Delete {
            station_idx: station_idx_raw,
        };
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

    view! {
        <div class="add-station-form">
            <div class="form-field">
                <label>"Station Name"</label>
                <input
                    type="text"
                    prop:value=move || station_name.get()
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

            <ConnectToStation
                current_station=editing_station
                graph=graph
                on_add_connection=handle_add_connection
            />

            <div class="form-section">
                <h3>"Default Platforms for Tracks"</h3>
                <p class="help-text">"Set which platform trains use by default when arriving from each direction"</p>
                <For
                    each=move || connected_tracks.get()
                    key=|track| track.edge_index.index()
                    children=move |track: ConnectedTrack| {
                        view! {
                            <TrackPlatformSelect
                                edge_index=track.edge_index
                                other_station_name=track.other_station_name
                                is_incoming=track.is_incoming
                                platforms=platforms
                                connected_tracks=connected_tracks
                                on_update=on_update_track_defaults.clone()
                                editing_station=editing_station
                                graph=graph
                                set_connected_tracks=set_connected_tracks
                            />
                        }
                    }
                />
            </div>

            <div class="form-buttons">
                <button class="danger" on:click=handle_delete>"Delete"</button>
                <div class="flex-spacer"></div>
                <button on:click=handle_cancel>"Cancel"</button>
                <button class="primary" on:click=handle_save>"Save"</button>
            </div>
        </div>
    }
}
