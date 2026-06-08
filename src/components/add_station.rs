use crate::components::add_station_quick::QuickEntryStation;
use crate::components::native_window::NativeWindow;
use crate::models::{Platform, RailwayGraph, Track};
use crate::window_protocol::{AddStationInit, AddStationResult};
use leptos::{component, IntoView, ReadSignal, Signal, SignalGet, view};
use petgraph::stable_graph::NodeIndex;
use std::rc::Rc;

type AddStationCallback = Rc<dyn Fn(String, bool, Option<NodeIndex>, Vec<Platform>)>;
pub type AddStationsBatchCallback = Rc<dyn Fn(Vec<QuickEntryStation>, Option<NodeIndex>, Vec<Platform>, Vec<Track>)>;

#[component]
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
    let _ = clicked_segment; // Not used in native window (no map interaction)

    let on_close_for_window = on_close.clone();
    let on_close_for_result = on_close.clone();

    let result_handler: Box<dyn Fn(String)> = Box::new(move |json: String| {
        match serde_json::from_str::<AddStationResult>(&json) {
            Ok(AddStationResult::Add {
                name,
                is_passing_loop,
                connect_to,
                platforms,
            }) => {
                on_add(
                    name,
                    is_passing_loop,
                    connect_to.map(NodeIndex::new),
                    platforms,
                );
            }
            Ok(AddStationResult::AddBatch {
                entries,
                connect_to,
                platforms,
                tracks,
            }) => {
                on_add_batch(
                    entries,
                    connect_to.map(NodeIndex::new),
                    platforms,
                    tracks,
                );
            }
            Err(e) => {
                leptos::logging::error!("Failed to parse AddStationResult: {}", e);
            }
        }
        on_close_for_result();
    });

    view! {
        <NativeWindow
            is_open=is_open
            title=Signal::derive(|| "Add New Station".to_string())
            on_close=move || on_close_for_window()
            window_type="add-station"
            init_data=Signal::derive(move || {
                if !is_open.get() {
                    return String::new();
                }
                let current_graph = graph.get();
                let suggested_name = format!("Station {}", current_graph.graph.node_count() + 1);
                let current_settings = settings.get();

                serde_json::to_string(&AddStationInit {
                    graph: current_graph,
                    suggested_name,
                    last_added_station: last_added_station.get().map(NodeIndex::index),
                    track_handedness: current_settings.track_handedness,
                    default_node_distance_grid_squares: current_settings.default_node_distance_grid_squares,
                }).unwrap_or_default()
            })
            on_result=result_handler
            size=(500, 500)
            position_key="add-station"
        />
    }
}
