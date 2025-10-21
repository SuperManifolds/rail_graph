use crate::models::RailwayGraph;
use leptos::{component, view, For, IntoView, ReadSignal, SignalGet};
use petgraph::stable_graph::{EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use std::rc::Rc;

#[component]
pub fn RoutingRuleEditor(
    junction_idx: ReadSignal<Option<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    on_rule_change: Rc<dyn Fn(EdgeIndex, EdgeIndex, bool)>,
) -> impl IntoView {
    // Get all connected edges for the junction
    let all_edges = move || {
        let Some(idx) = junction_idx.get() else {
            return Vec::new();
        };

        let current_graph = graph.get();
        let mut result = Vec::new();

        // Get incoming edges (trains arriving FROM these nodes)
        for edge in current_graph.graph.edges_directed(idx, Direction::Incoming) {
            result.push((edge.id(), edge.source(), true)); // true = incoming
        }

        // Get outgoing edges (trains departing TO these nodes)
        for edge in current_graph.graph.edges(idx) {
            result.push((edge.id(), edge.target(), false)); // false = outgoing
        }

        result
    };

    // Get the junction's current routing rules
    let get_junction = move || {
        let idx = junction_idx.get()?;

        let current_graph = graph.get();
        current_graph
            .graph
            .node_weight(idx)
            .and_then(|node| node.as_junction())
            .cloned()
    };

    // Helper to get node name (station or junction)
    let get_node_label = move |node_idx: NodeIndex| {
        let current_graph = graph.get();
        current_graph.get_node_name(node_idx)
            .unwrap_or_else(|| format!("Node {node_idx:?}"))
    };

    view! {
        <div class="routing-rules">
            <h3>"Routing Rules"</h3>
            {
                let callback = on_rule_change.clone();
                move || {
                    let edges_list = all_edges();

                    if edges_list.is_empty() {
                        return view! { <p class="no-edges">"No edges connected to this junction yet."</p> }.into_view();
                    }

                    let junction = get_junction();

                    view! {
                        <div class="routing-visual">
                            <For
                                each=move || all_edges()
                                key=|(from_edge, _, _)| from_edge.index()
                                children={
                                    let callback = callback.clone();
                                    move |(from_edge, from_node, _from_is_incoming)| {
                                        let from_label = get_node_label(from_node);

                                        view! {
                                            <div class="routing-row">
                                                <div class="from-direction">
                                                    <span class="direction-label">"FROM "{from_label}</span>
                                                </div>
                                                <div class="arrow-connector">"→"</div>
                                                <div class="to-directions">
                                                    {
                                                        let callback = callback.clone();
                                                        let junction = junction.clone();
                                                        all_edges().into_iter().filter_map(move |(to_edge, to_node, _to_is_incoming)| {
                                                            // Skip same edge
                                                            if from_edge == to_edge {
                                                                return None;
                                                            }

                                                            let is_allowed = if let Some(ref j) = junction {
                                                                j.is_routing_allowed(from_edge, to_edge)
                                                            } else {
                                                                true
                                                            };

                                                            let to_label = get_node_label(to_node);
                                                            let callback = callback.clone();
                                                            let class = if is_allowed { "route-btn allowed" } else { "route-btn forbidden" };

                                                            Some(view! {
                                                                <button
                                                                    class=class
                                                                    on:click=move |_| {
                                                                        callback(from_edge, to_edge, !is_allowed);
                                                                    }
                                                                    title=if is_allowed { "Click to forbid this route" } else { "Click to allow this route" }
                                                                >
                                                                    "TO "{to_label}
                                                                </button>
                                                            }.into_view())
                                                        }).collect::<Vec<_>>()
                                                    }
                                                </div>
                                            </div>
                                        }
                                    }
                                }
                            />
                        </div>
                    }.into_view()
                }
            }
        </div>
    }
}
