use crate::components::routing_rule_editor::RoutingRuleEditor;
use crate::models::Junctions;
use crate::window_protocol::{EditJunctionInit, EditJunctionResult};
use leptos::{component, create_signal, event_target_value, view, IntoView, SignalGet, SignalSet};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use std::rc::Rc;

#[component]
#[must_use]
pub fn EditJunctionChild(init: EditJunctionInit, session: String) -> impl IntoView {
    let junction_idx_raw = init.junction_idx;
    let (graph, set_graph) = create_signal(init.graph);
    let (junction_idx, _) = create_signal(Some(NodeIndex::new(init.junction_idx)));
    let (junction_name, set_junction_name) = create_signal(init.junction_name);

    let handle_routing_rule_change =
        Rc::new(move |from_edge: EdgeIndex, to_edge: EdgeIndex, allowed: bool| {
            let mut current_graph = graph.get();
            if let Some(junction) = current_graph.get_junction_mut(NodeIndex::new(junction_idx_raw))
            {
                junction.set_routing_rule(from_edge, to_edge, allowed);
            }
            set_graph.set(current_graph);
        });

    let session_save = session.clone();
    let handle_save = move |_| {
        let name = junction_name.get();
        let name_opt = if name.is_empty() { None } else { Some(name) };

        let current_graph = graph.get();
        let routing_rules = current_graph
            .get_junction(NodeIndex::new(junction_idx_raw))
            .map(|j| j.routing_rules.clone())
            .unwrap_or_default();

        let result = EditJunctionResult::Save {
            junction_idx: junction_idx_raw,
            name: name_opt,
            routing_rules,
        };
        let json = serde_json::to_string(&result).unwrap_or_default();
        let event_name = format!("result:{session_save}");
        leptos::spawn_local(async move {
            let _ = crate::tauri_bridge::emit_event(&event_name, &json).await;
        });
    };

    let session_delete = session.clone();
    let handle_delete = move |_| {
        let result = EditJunctionResult::Delete {
            junction_idx: junction_idx_raw,
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
                <label>"Junction Name (optional)"</label>
                <input
                    type="text"
                    placeholder="Unnamed Junction"
                    prop:value=move || junction_name.get()
                    on:input=move |ev| set_junction_name.set(event_target_value(&ev))
                />
            </div>

            <RoutingRuleEditor
                junction_idx=junction_idx
                graph=graph
                on_rule_change=handle_routing_rule_change
            />

            <div class="form-buttons">
                <button class="danger" on:click=handle_delete>"Delete"</button>
                <div class="flex-spacer"></div>
                <button on:click=handle_cancel>"Cancel"</button>
                <button class="primary" on:click=handle_save>"Save"</button>
            </div>
        </div>
    }
}
