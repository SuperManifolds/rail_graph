use crate::components::window::Window;
use crate::components::platform_editor::PlatformEditor;
use crate::components::add_station_quick::{AddStationQuick, QuickEntryStation};
use crate::models::{RailwayGraph, Platform, Track, TrackDirection};
use leptos::{component, create_effect, create_signal, event_target_checked, event_target_value, IntoView, ReadSignal, Signal, SignalGet, SignalSet, SignalUpdate, use_context, view, WriteSignal};
use petgraph::stable_graph::NodeIndex;
use petgraph::visit::{EdgeRef, IntoEdgeReferences};
use std::rc::Rc;

type AddStationCallback = Rc<dyn Fn(String, bool, Option<NodeIndex>, Vec<Platform>)>;
pub type AddStationsBatchCallback = Rc<dyn Fn(Vec<QuickEntryStation>, Option<NodeIndex>, Vec<Platform>, Vec<Track>)>;

#[component]
#[allow(clippy::too_many_lines)]
pub fn AddStation(
    is_open: ReadSignal<bool>,
    on_close: Rc<dyn Fn()>,
    on_add: AddStationCallback,
    on_add_batch: AddStationsBatchCallback,
    graph: ReadSignal<RailwayGraph>,
    last_added_station: ReadSignal<Option<NodeIndex>>,
    clicked_segment: ReadSignal<Option<petgraph::stable_graph::EdgeIndex>>,
    settings: ReadSignal<crate::models::ProjectSettings>,
) -> impl IntoView {
    let (station_name, set_station_name) = create_signal(String::new());
    let (is_passing_loop, set_is_passing_loop) = create_signal(false);
    let (connect_to_station, set_connect_to_station) = create_signal(None::<NodeIndex>);
    let (platforms, set_platforms) = create_signal(vec![
        Platform { name: "1".to_string() },
        Platform { name: "2".to_string() },
    ]);

    // Quick entry mode signals
    let (quick_entry_mode, set_quick_entry_mode) = create_signal(false);
    let (station_entries, set_station_entries) = create_signal(vec![
        QuickEntryStation {
            name: String::new(),
            distance_from_previous: 0.0,
            is_passing_loop: false,
        }
    ]);
    let (tracks, set_tracks) = create_signal(vec![Track { direction: TrackDirection::Bidirectional }]);

    // Reset form when dialog opens
    create_effect(move |_| {
        if is_open.get() {
            set_station_name.set(format!("Station {}", graph.get().graph.node_count() + 1));
            set_is_passing_loop.set(false);
            // Default to last added station if available
            set_connect_to_station.set(last_added_station.get());
            set_platforms.set(vec![
                Platform { name: "1".to_string() },
                Platform { name: "2".to_string() },
            ]);
            // Reset quick entry mode
            set_quick_entry_mode.set(false);
            set_station_entries.set(vec![
                QuickEntryStation {
                    name: String::new(),
                    distance_from_previous: 0.0,
                    is_passing_loop: false,
                }
            ]);
            set_tracks.set(vec![Track { direction: TrackDirection::Bidirectional }]);
        }
    });

    let on_close_clone = on_close.clone();
    let handle_add = move |_| {
        let name = station_name.get();
        let current_platforms = platforms.get();
        if !name.is_empty() && !current_platforms.is_empty() {
            on_add(name, is_passing_loop.get(), connect_to_station.get(), current_platforms);
        }
    };


    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Add New Station".to_string())
            on_close=move || on_close_clone()
            position_key="add-station"
        >
            <div class="add-station-form">
                {
                    // Trigger window resize when quick entry mode changes
                    if let Some(set_resize_trigger) = use_context::<WriteSignal<u32>>() {
                        create_effect(move |_| {
                            let _mode = quick_entry_mode.get();
                            set_resize_trigger.update(|n| *n = n.wrapping_add(1));
                        });
                    }
                    // Return empty view
                    view! { <></> }
                }
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
                    let on_close_quick = on_close.clone();
                    let on_close_normal = on_close.clone();
                    let on_add_batch_for_quick = on_add_batch.clone();
                    let handle_add_for_normal = handle_add.clone();
                    move || {
                        if quick_entry_mode.get() {
                            // Render quick entry component
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
                            // Render normal single-station form
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

                {move || {
                    if let Some(edge_idx) = clicked_segment.get() {
                        let current_graph = graph.get();
                        if let Some(edge_ref) = current_graph.graph.edge_references().find(|e| e.id() == edge_idx) {
                            let from_node = edge_ref.source();
                            let to_node = edge_ref.target();
                            let from_name = current_graph.graph.node_weight(from_node)
                                .map_or_else(|| "Unknown".to_string(), crate::models::Node::display_name);
                            let to_name = current_graph.graph.node_weight(to_node)
                                .map_or_else(|| "Unknown".to_string(), crate::models::Node::display_name);

                            return view! {
                                <div class="form-field">
                                    <label>"Connection"</label>
                                    <div class="connection-info">
                                        {format!("Connecting to {from_name} and {to_name}")}
                                    </div>
                                </div>
                            }.into_view();
                        }
                    }

                    view! {
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
                    }.into_view()
                }}
                                <div class="form-buttons">
                                    <button on:click={
                                        let on_close_clone2 = on_close_normal.clone();
                                        move |_| on_close_clone2()
                                    }>"Cancel"</button>
                                    <button class="primary" on:click={
                                        let handle_add_clone2 = handle_add_for_normal.clone();
                                        move |ev| handle_add_clone2(ev)
                                    }>"Add"</button>
                                </div>
                        }.into_view()
                    }
                }
                }
            </div>
        </Window>
    }
}
