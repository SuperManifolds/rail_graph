use crate::components::native_window::NativeWindow;
use crate::models::{RailwayGraph, Track, Line};
use crate::window_protocol::{EditTrackInit, EditTrackResult};
use leptos::{component, IntoView, ReadSignal, Signal, SignalGet, view};
use petgraph::stable_graph::EdgeIndex;
use std::rc::Rc;

type SaveTrackCallback = Rc<dyn Fn(EdgeIndex, Vec<Track>, Option<f64>)>;

#[component]
pub fn EditTrack(
    editing_track: ReadSignal<Option<EdgeIndex>>,
    on_close: Rc<dyn Fn()>,
    on_save: SaveTrackCallback,
    on_delete: Rc<dyn Fn(EdgeIndex)>,
    graph: ReadSignal<RailwayGraph>,
    lines: ReadSignal<Vec<Line>>,
    settings: ReadSignal<crate::models::ProjectSettings>,
) -> impl IntoView {
    let is_open = Signal::derive(move || editing_track.get().is_some());

    let on_close_for_window = on_close.clone();
    let on_close_for_result = on_close.clone();

    let result_handler: Box<dyn Fn(String)> = Box::new(move |json: String| {
        match serde_json::from_str::<EditTrackResult>(&json) {
            Ok(EditTrackResult::Save { edge_idx, tracks, distance }) => {
                on_save(EdgeIndex::new(edge_idx), tracks, distance);
            }
            Ok(EditTrackResult::Delete { edge_idx }) => {
                on_delete(EdgeIndex::new(edge_idx));
            }
            Err(e) => {
                leptos::logging::error!("Failed to parse EditTrackResult: {}", e);
            }
        }
        on_close_for_result();
    });

    view! {
        <NativeWindow
            is_open=is_open
            title=Signal::derive(|| "Edit Track".to_string())
            on_close=move || on_close_for_window()
            window_type="edit-track"
            init_data=Signal::derive(move || {
                let Some(edge_idx) = editing_track.get() else {
                    return String::new();
                };
                let current_graph = graph.get();
                let current_lines = lines.get();

                let (tracks, distance) = current_graph
                    .graph
                    .edge_weight(edge_idx)
                    .map(|ts| (ts.tracks.clone(), ts.distance))
                    .unwrap_or_default();

                let (from_name, to_name) = current_graph
                    .graph
                    .edge_endpoints(edge_idx)
                    .map(|(from, to)| {
                        let f = current_graph.graph.node_weight(from).map_or_else(String::new, railgraph_core::models::Node::display_name);
                        let t = current_graph.graph.node_weight(to).map_or_else(String::new, railgraph_core::models::Node::display_name);
                        (f, t)
                    })
                    .unwrap_or_default();

                let edge_index = edge_idx.index();
                let affected: Vec<String> = current_lines
                    .iter()
                    .filter(|line| line.uses_edge(edge_index))
                    .map(|line| line.name.clone())
                    .collect();

                serde_json::to_string(&EditTrackInit {
                    edge_idx: edge_idx.index(),
                    tracks,
                    distance,
                    from_station_name: from_name,
                    to_station_name: to_name,
                    affected_lines: affected,
                    track_handedness: settings.get().track_handedness,
                }).unwrap_or_default()
            })
            on_result=result_handler
            size=(500, 450)
            position_key="edit-track"
        />
    }
}
