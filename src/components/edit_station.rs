use crate::components::window::Window;
use crate::components::platform_editor::PlatformEditor;
use crate::models::{RailwayGraph, Platform};
use leptos::{component, create_effect, create_signal, event_target_checked, event_target_value, IntoView, ReadSignal, Signal, SignalGet, SignalSet, SignalGetUntracked, view, For};
use petgraph::stable_graph::{NodeIndex, EdgeIndex};
use petgraph::visit::EdgeRef;
use std::rc::Rc;

type TrackDefaultsCallback = Rc<dyn Fn(EdgeIndex, Option<usize>, Option<usize>)>;

#[derive(Clone, Debug)]
struct ConnectedTrack {
    edge_index: EdgeIndex,
    other_station_name: String,
    is_incoming: bool, // true if arriving at this station, false if departing
    current_default_platform: Option<usize>,
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

                <div class="form-section">
                    <h3>"Default Platforms for Tracks"</h3>
                    <p class="help-text">"Set which platform trains use by default when arriving from each direction"</p>
                    <For
                        each=move || connected_tracks.get()
                        key=|track| track.edge_index.index()
                        children={
                            let on_update = on_update_track_defaults.clone();
                            move |track: ConnectedTrack| {
                                let platform_count = platforms.get().len();
                                let edge_idx = track.edge_index;
                                let is_incoming = track.is_incoming;
                                let on_update = on_update.clone();

                                view! {
                                    <div class="track-default-platform">
                                        <label>{track.other_station_name}</label>
                                        <select
                                            class="platform-select"
                                            on:change={
                                                let on_update = on_update.clone();
                                                move |ev| {
                                                    let value = event_target_value(&ev);
                                                    let platform_idx = if value == "auto" {
                                                        None
                                                    } else {
                                                        value.parse::<usize>().ok()
                                                    };

                                                    // Update the track segment
                                                    if is_incoming {
                                                        on_update(edge_idx, None, platform_idx);
                                                    } else {
                                                        on_update(edge_idx, platform_idx, None);
                                                    }

                                                    // Reload tracks to show updated value
                                                    if let Some(idx) = editing_station.get_untracked() {
                                                        let current_graph = graph.get_untracked();
                                                        set_connected_tracks.set(load_connected_tracks(idx, &current_graph));
                                                    }
                                                }
                                            }
                                    >
                                        <option value="auto" selected=move || track.current_default_platform.is_none()>
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
                                            (0..platform_count).map(|i| {
                                                let platform = platforms.get()[i].clone();
                                                view! {
                                                    <option
                                                        value=i.to_string()
                                                        selected=track.current_default_platform == Some(i)
                                                    >
                                                        {platform.name}
                                                        </option>
                                                    }
                                                }).collect::<Vec<_>>()
                                            }}
                                        </select>
                                    </div>
                                }
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
