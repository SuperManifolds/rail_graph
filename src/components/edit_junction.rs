use crate::components::native_window::NativeWindow;
use crate::models::{Junctions, RailwayGraph, RoutingRule};
use crate::window_protocol::{EditJunctionInit, EditJunctionResult};
use leptos::{component, IntoView, ReadSignal, Signal, SignalGet, view};
use petgraph::stable_graph::NodeIndex;
use std::rc::Rc;

type SaveJunctionCallback = Rc<dyn Fn(NodeIndex, Option<String>, Vec<RoutingRule>)>;

#[component]
pub fn EditJunction(
    editing_junction: ReadSignal<Option<NodeIndex>>,
    on_close: Rc<dyn Fn()>,
    on_save: SaveJunctionCallback,
    on_delete: Rc<dyn Fn(NodeIndex)>,
    graph: ReadSignal<RailwayGraph>,
) -> impl IntoView {
    let is_open = Signal::derive(move || editing_junction.get().is_some());

    let on_close_for_window = on_close.clone();
    let on_close_for_result = on_close.clone();

    let result_handler: Box<dyn Fn(String)> = Box::new(move |json: String| {
        match serde_json::from_str::<EditJunctionResult>(&json) {
            Ok(EditJunctionResult::Save { junction_idx, name, routing_rules }) => {
                on_save(NodeIndex::new(junction_idx), name, routing_rules);
            }
            Ok(EditJunctionResult::Delete { junction_idx }) => {
                on_delete(NodeIndex::new(junction_idx));
            }
            Err(e) => {
                leptos::logging::error!("Failed to parse EditJunctionResult: {}", e);
            }
        }
        on_close_for_result();
    });

    view! {
        <NativeWindow
            is_open=is_open
            title=Signal::derive(|| "Edit Junction".to_string())
            on_close=move || on_close_for_window()
            window_type="edit-junction"
            init_data=Signal::derive(move || {
                let Some(junction_idx) = editing_junction.get() else {
                    return String::new();
                };
                let current_graph = graph.get();

                let junction_name = current_graph
                    .get_junction(junction_idx)
                    .and_then(|j| j.name.clone())
                    .unwrap_or_default();

                serde_json::to_string(&EditJunctionInit {
                    junction_idx: junction_idx.index(),
                    junction_name,
                    graph: current_graph,
                }).unwrap_or_default()
            })
            on_result=result_handler
            size=(500, 400)
            position_key="edit-junction"
        />
    }
}
