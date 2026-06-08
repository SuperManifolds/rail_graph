use crate::components::native_window::NativeWindow;
use crate::models::{Platform, RailwayGraph};
use crate::window_protocol::{EditStationInit, EditStationResult};
use leptos::{component, event_target_value, view, IntoView, ReadSignal, Signal, SignalGet, SignalGetUntracked, SignalSet, WriteSignal};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use std::rc::Rc;

pub type TrackDefaultsCallback = Rc<dyn Fn(EdgeIndex, Option<usize>, Option<usize>)>;

#[derive(Clone, Debug)]
pub struct ConnectedTrack {
    pub edge_index: EdgeIndex,
    pub other_station_name: String,
    pub is_incoming: bool,
    pub current_default_platform: Option<usize>,
}

#[component]
pub fn TrackPlatformSelect(
    edge_index: EdgeIndex,
    other_station_name: String,
    is_incoming: bool,
    platforms: ReadSignal<Vec<Platform>>,
    connected_tracks: ReadSignal<Vec<ConnectedTrack>>,
    on_update: TrackDefaultsCallback,
    editing_station: ReadSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    set_connected_tracks: WriteSignal<Vec<ConnectedTrack>>,
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

#[must_use]
pub fn load_connected_tracks(station_idx: NodeIndex, graph: &RailwayGraph) -> Vec<ConnectedTrack> {
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
    on_add_connection: Rc<dyn Fn(NodeIndex, NodeIndex)>,
) -> impl IntoView {
    let is_open = Signal::derive(move || editing_station.get().is_some());

    let on_close_for_window = on_close.clone();
    let on_close_for_result = on_close.clone();

    let result_handler: Box<dyn Fn(String)> = Box::new(move |json: String| {
        match serde_json::from_str::<EditStationResult>(&json) {
            Ok(EditStationResult::Save {
                station_idx,
                name,
                is_passing_loop,
                platforms,
                track_defaults,
                new_connections,
            }) => {
                let station_node_idx = NodeIndex::new(station_idx);

                // Apply track default changes
                for change in &track_defaults {
                    on_update_track_defaults(
                        EdgeIndex::new(change.edge_idx),
                        change.default_platform_source,
                        change.default_platform_target,
                    );
                }

                // Apply new connections
                for &connect_idx in &new_connections {
                    on_add_connection(station_node_idx, NodeIndex::new(connect_idx));
                }

                on_save(station_node_idx, name, is_passing_loop, platforms);
            }
            Ok(EditStationResult::Delete { station_idx }) => {
                on_delete(NodeIndex::new(station_idx));
            }
            Err(e) => {
                leptos::logging::error!("Failed to parse EditStationResult: {}", e);
            }
        }
        on_close_for_result();
    });

    view! {
        <NativeWindow
            is_open=is_open
            title=Signal::derive(|| "Edit Station".to_string())
            on_close=move || on_close_for_window()
            window_type="edit-station"
            init_data=Signal::derive(move || {
                let Some(station_idx) = editing_station.get() else {
                    return String::new();
                };
                let current_graph = graph.get();

                let (station_name, is_passing_loop, platforms) = current_graph
                    .graph
                    .node_weight(station_idx)
                    .and_then(|node| node.as_station())
                    .map(|station| {
                        (
                            station.name.clone(),
                            station.passing_loop,
                            station.platforms.clone(),
                        )
                    })
                    .unwrap_or_default();

                serde_json::to_string(&EditStationInit {
                    station_idx: station_idx.index(),
                    station_name,
                    is_passing_loop,
                    platforms,
                    graph: current_graph,
                }).unwrap_or_default()
            })
            on_result=result_handler
            size=(500, 550)
            position_key="edit-station"
        />
    }
}
