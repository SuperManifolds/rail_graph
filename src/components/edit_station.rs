use crate::components::window::Window;
use crate::components::platform_editor::PlatformEditor;
use crate::components::connect_to_station::ConnectToStation;
use crate::models::{RailwayGraph, Platform};
use leptos::{component, create_effect, create_signal, event_target_checked, event_target_value, IntoView, ReadSignal, Signal, SignalGet, SignalSet, SignalGetUntracked, view, For};
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::EdgeRef;
use std::rc::Rc;

type TrackDefaultsCallback = Rc<dyn Fn(EdgeIndex, Option<usize>, Option<usize>)>;
type AddConnectionCallback = Rc<dyn Fn(NodeIndex, NodeIndex)>;

#[derive(Clone, Debug)]
struct ConnectedTrack {
    edge_index: EdgeIndex,
    other_station_name: String,
    is_incoming: bool, // true if arriving at this station, false if departing
    current_default_platform: Option<usize>,
}

#[component]
fn TrackPlatformSelect(
    edge_index: EdgeIndex,
    other_station_name: String,
    is_incoming: bool,
    platforms: ReadSignal<Vec<Platform>>,
    connected_tracks: ReadSignal<Vec<ConnectedTrack>>,
    on_update: TrackDefaultsCallback,
    editing_station: ReadSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_connected_tracks: leptos::WriteSignal<Vec<ConnectedTrack>>,
) -> impl IntoView {
    view! {
        <div class="track-default-platform">
            <label>{other_station_name}</label>
            <select
                class="platform-select"
                prop:value=move || {
                    // Look up current value from signal to ensure reactivity
                    let current = connected_tracks.get()
                        .iter()
                        .find(|t| t.edge_index == edge_index)
                        .and_then(|t| t.current_default_platform);

                    current.map_or_else(|| "auto".to_string(), |i| i.to_string())
                }
                on:change=move |ev| {
                    let value = event_target_value(&ev);
                    let platform_idx = if value == "auto" {
                        None
                    } else {
                        value.parse::<usize>().ok()
                    };

                    // Update the track segment
                    if is_incoming {
                        on_update(edge_index, None, platform_idx);
                    } else {
                        on_update(edge_index, platform_idx, None);
                    }

                    // Reload tracks to show updated value
                    if let Some(idx) = editing_station.get_untracked() {
                        let current_graph = graph.get_untracked();
                        set_connected_tracks.set(load_connected_tracks(idx, &current_graph));
                    }
                }
            >
                <option value="auto">
                    {move || {
                        let all_platforms = platforms.get();
                        if is_incoming {
                            if let Some(last_platform) = all_platforms.last() {
                                format!("Auto ({})", last_platform.name)
                            } else {
                                "Auto".to_string()
                            }
                        } else if let Some(first_platform) = all_platforms.first() {
                            format!("Auto ({})", first_platform.name)
                        } else {
                            "Auto".to_string()
                        }
                    }}
                </option>
                {move || {
                    let current_platforms = platforms.get();
                    (0..current_platforms.len()).map(|i| {
                        let platform = current_platforms[i].clone();
                        view! {
                            <option value=i.to_string()>
                                {platform.name}
                            </option>
                        }
                    }).collect::<Vec<_>>()
                }}
            </select>
        </div>
    }
}

fn load_connected_tracks(station_idx: NodeIndex, graph: &RailwayGraph) -> Vec<ConnectedTrack> {
    let mut tracks = Vec::new();

    // Outgoing edges (departing from this station)
    for edge_ref in graph.graph.edges(station_idx) {
        let target = edge_ref.target();
        let edge_idx = edge_ref.id();
        let track_segment = edge_ref.weight();

        if let Some(other_name) = graph.get_node_name(target) {
            tracks.push(ConnectedTrack {
                edge_index: edge_idx,
                other_station_name: format!("→ {other_name}"),
                is_incoming: false,
                current_default_platform: track_segment.default_platform_source,
            });
        }
    }

    // Incoming edges (arriving at this station)
    for edge_ref in graph.graph.edges_directed(station_idx, petgraph::Direction::Incoming) {
        let source = edge_ref.source();
        let edge_idx = edge_ref.id();
        let track_segment = edge_ref.weight();

        if let Some(other_name) = graph.get_node_name(source) {
            tracks.push(ConnectedTrack {
                edge_index: edge_idx,
                other_station_name: format!("← {other_name}"),
                is_incoming: true,
                current_default_platform: track_segment.default_platform_target,
            });
        }
    }

    tracks
}

#[component]
pub fn EditStation(
    editing_station: ReadSignal<Option<NodeIndex>>,
    on_close: Rc<dyn Fn()>,
    on_save: Rc<dyn Fn(NodeIndex, String, bool, Vec<Platform>)>,
    on_delete: Rc<dyn Fn(NodeIndex)>,
    graph: ReadSignal<RailwayGraph>,
    on_update_track_defaults: TrackDefaultsCallback,
    on_add_connection: AddConnectionCallback,
) -> impl IntoView {
    let (station_name, set_station_name) = create_signal(String::new());
    let (is_passing_loop, set_is_passing_loop) = create_signal(false);
    let (platforms, set_platforms) = create_signal(Vec::<Platform>::new());
    let (connected_tracks, set_connected_tracks) = create_signal(Vec::<ConnectedTrack>::new());

    // Load current station data when dialog opens
    create_effect(move |_| {
        if let Some(idx) = editing_station.get() {
            let current_graph = graph.get();
            if let Some(node) = current_graph.graph.node_weight(idx) {
                if let Some(station) = node.as_station() {
                    set_station_name.set(station.name.clone());
                    set_is_passing_loop.set(station.passing_loop);
                    set_platforms.set(station.platforms.clone());
                    set_connected_tracks.set(load_connected_tracks(idx, &current_graph));
                }
            }
        }
    });

    let on_close_clone = on_close.clone();
    let handle_save = move |_| {
        if let Some(idx) = editing_station.get() {
            let name = station_name.get();
            let current_platforms = platforms.get();
            if !name.is_empty() && !current_platforms.is_empty() {
                on_save(idx, name, is_passing_loop.get(), current_platforms);
            }
        }
    };

    let handle_delete = move |_| {
        if let Some(idx) = editing_station.get() {
            on_delete(idx);
        }
    };

    let handle_add_connection = Rc::new(move |connect_idx: NodeIndex| {
        if let Some(station_idx) = editing_station.get_untracked() {
            on_add_connection(station_idx, connect_idx);
            // Reload connected tracks to show new connection
            let current_graph = graph.get_untracked();
            set_connected_tracks.set(load_connected_tracks(station_idx, &current_graph));
        }
    });

    let is_open = Signal::derive(move || editing_station.get().is_some());

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Edit Station".to_string())
            on_close=move || on_close_clone()
        >
            <div class="add-station-form">
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
                    <div style="flex: 1;"></div>
                    <button on:click=move |_| on_close()>"Cancel"</button>
                    <button class="primary" on:click=handle_save>"Save"</button>
                </div>
            </div>
        </Window>
    }
}
