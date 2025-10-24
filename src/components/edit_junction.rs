use crate::components::window::Window;
use crate::components::routing_rule_editor::RoutingRuleEditor;
use crate::models::{RailwayGraph, Junctions};
use leptos::{component, create_effect, create_signal, event_target_value, IntoView, ReadSignal, Signal, SignalGet, SignalSet, WriteSignal, view};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::rc::Rc;

#[component]
pub fn EditJunction(
    editing_junction: ReadSignal<Option<NodeIndex>>,
    on_close: Rc<dyn Fn()>,
    on_save: Rc<dyn Fn(NodeIndex, Option<String>)>,
    on_delete: Rc<dyn Fn(NodeIndex)>,
    graph: ReadSignal<RailwayGraph>,
    set_graph: WriteSignal<RailwayGraph>,
) -> impl IntoView {
    let (junction_name, set_junction_name) = create_signal(String::new());

    // Load current junction data when dialog opens
    create_effect(move |_| {
        if let Some(idx) = editing_junction.get() {
            let current_graph = graph.get();
            if let Some(node) = current_graph.graph.node_weight(idx) {
                if let Some(junction) = node.as_junction() {
                    set_junction_name.set(junction.name.clone().unwrap_or_default());
                }
            }
        }
    });

    let on_close_clone = on_close.clone();
    let handle_save = move |_| {
        if let Some(idx) = editing_junction.get() {
            let name = junction_name.get();
            let name_opt = if name.is_empty() { None } else { Some(name) };
            on_save(idx, name_opt);
        }
    };

    let handle_delete = move |_| {
        if let Some(idx) = editing_junction.get() {
            on_delete(idx);
        }
    };

    // Handle routing rule changes
    let handle_routing_rule_change = Rc::new(move |from_edge: EdgeIndex, to_edge: EdgeIndex, allowed: bool| {
        let Some(junction_idx) = editing_junction.get() else { return };

        let mut current_graph = graph.get();
        if let Some(junction) = current_graph.get_junction_mut(junction_idx) {
            junction.set_routing_rule(from_edge, to_edge, allowed);
        }
        set_graph.set(current_graph);
    });

    let is_open = Signal::derive(move || editing_junction.get().is_some());

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Edit Junction".to_string())
            on_close=move || on_close_clone()
            position_key="edit-junction"
        >
            <div class="add-station-form">
                <div class="form-field">
                    <label>"Junction Name (optional)"</label>
                    <input
                        type="text"
                        placeholder="Unnamed Junction"
                        value=move || junction_name.get()
                        on:input=move |ev| set_junction_name.set(event_target_value(&ev))
                    />
                </div>

                <RoutingRuleEditor
                    junction_idx=editing_junction
                    graph=graph
                    on_rule_change=handle_routing_rule_change
                />

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
