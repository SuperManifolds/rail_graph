use crate::models::{RailwayGraph, Junctions};
use leptos::{component, create_signal, event_target_value, IntoView, ReadSignal, SignalGet, SignalSet, view};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::EdgeRef;
use std::rc::Rc;

#[component]
pub fn ConnectToStation(
    current_station: ReadSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    on_add_connection: Rc<dyn Fn(NodeIndex)>,
) -> impl IntoView {
    let (connect_to_station, set_connect_to_station) = create_signal(None::<NodeIndex>);

    let handle_add_connection = move |_| {
        if let Some(connect_idx) = connect_to_station.get() {
            on_add_connection(connect_idx);
            set_connect_to_station.set(None);
        }
    };

    view! {
        <div class="form-section">
            <h3>"Add Connection"</h3>
            <div class="form-field">
                <label>"Connect to"</label>
                <div class="connect-station-row">
                    <select
                        class="connect-station-select"
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
                        <option value="">"Select station..."</option>
                        {move || {
                            let current_graph = graph.get();
                            let current_station_idx = current_station.get();

                            // Get list of already connected stations
                            let mut connected_stations = std::collections::HashSet::new();
                            if let Some(station_idx) = current_station_idx {
                                // Outgoing edges
                                for edge_ref in current_graph.graph.edges(station_idx) {
                                    connected_stations.insert(edge_ref.target());
                                }
                                // Incoming edges
                                for edge_ref in current_graph.graph.edges_directed(station_idx, petgraph::Direction::Incoming) {
                                    connected_stations.insert(edge_ref.source());
                                }
                            }

                            current_graph.graph.node_indices().enumerate().filter_map(|(i, idx)| {
                                // Filter out the current station and junctions
                                if Some(idx) == current_station_idx || current_graph.is_junction(idx) {
                                    return None;
                                }
                                // Filter out already connected stations
                                if connected_stations.contains(&idx) {
                                    return None;
                                }
                                current_graph.graph.node_weight(idx).map(|node| {
                                    let name = node.display_name();
                                    view! {
                                        <option value=i.to_string()>{name}</option>
                                    }
                                })
                            }).collect::<Vec<_>>()
                        }}
                    </select>
                    <button
                        class="primary"
                        on:click=handle_add_connection
                        disabled=move || connect_to_station.get().is_none()
                    >
                        "Add Connection"
                    </button>
                </div>
            </div>
        </div>
    }
}
