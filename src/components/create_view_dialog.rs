use leptos::{component, view, IntoView, ReadSignal, Signal, SignalGet, SignalSet, SignalUpdate, create_signal, event_target_value, For, use_context, create_effect, WriteSignal};
use petgraph::stable_graph::NodeIndex;
use std::rc::Rc;
use crate::models::RailwayGraph;
use crate::components::window::Window;

#[component]
#[allow(clippy::too_many_lines)]
fn CreateViewDialogContent(
    waypoints: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    validation_error: ReadSignal<Option<String>>,
    on_close: Rc<dyn Fn()>,
    on_create: Rc<dyn Fn(String, Vec<NodeIndex>)>,
    on_add_waypoint: Rc<dyn Fn(NodeIndex)>,
    on_remove_waypoint: Rc<dyn Fn(usize)>,
) -> impl IntoView {
    let (view_name, set_view_name) = create_signal(String::new());
    let (selected_node, set_selected_node) = create_signal(String::new());

    // Get resize trigger from Window context to auto-resize as waypoints are added
    let set_resize_trigger = use_context::<WriteSignal<u32>>();

    // Trigger window resize when waypoints or validation error change
    create_effect(move |_| {
        let _ = waypoints.get();
        let _ = validation_error.get();
        if let Some(trigger) = set_resize_trigger {
            trigger.update(|val| *val = val.wrapping_add(1));
        }
    });

    let get_node_name = move |idx: NodeIndex| -> String {
        let g = graph.get();
        g.graph.node_weight(idx)
            .map(|node| node.display_name().clone())
            .unwrap_or_default()
    };

    // Get all nodes (stations and junctions) for the dropdown
    let get_all_nodes = move || -> Vec<(NodeIndex, String)> {
        let g = graph.get();
        g.graph.node_indices()
            .filter_map(|idx| {
                g.graph.node_weight(idx).map(|node| {
                    (idx, node.display_name().clone())
                })
            })
            .collect()
    };

    let create_view = {
        let on_create = on_create.clone();
        move || {
            let wps = waypoints.get();
            if wps.len() >= 2 && !view_name.get().trim().is_empty() && validation_error.get().is_none() {
                on_create(view_name.get().trim().to_string(), wps);
                set_view_name.set(String::new());
            }
        }
    };

    let add_waypoint_from_dropdown = {
        let on_add = on_add_waypoint.clone();
        move |ev| {
            let selected = event_target_value(&ev);
            if !selected.is_empty() {
                if let Ok(idx) = selected.parse::<usize>() {
                    on_add(NodeIndex::new(idx));
                    set_selected_node.set(String::new()); // Reset dropdown
                }
            }
        }
    };

    view! {
        <div class="add-station-form">
            <div class="form-field">
                <label>"View Name"</label>
                <input
                    type="text"
                    placeholder="Enter view name"
                    value=view_name
                    on:input=move |ev| set_view_name.set(event_target_value(&ev))
                    on:keydown={
                        let create_view = create_view.clone();
                        move |ev| {
                            if ev.key() == "Enter" {
                                create_view();
                            }
                        }
                    }
                    prop:autofocus=true
                />
            </div>

            <div class="form-field">
                <label>"Waypoints (" {move || waypoints.get().len().to_string()} ")"</label>
                <div class="stops-list">
                    <div class="stops-header">
                        <span>"Station / Junction"</span>
                        <span></span>
                    </div>
                    {
                        let on_remove = on_remove_waypoint.clone();
                        move || {
                            if waypoints.get().is_empty() {
                                view! {
                                    <div class="no-stops">"Click stations/junctions on the map or use the dropdown below"</div>
                                }.into_view()
                            } else {
                                let on_remove_for_list = on_remove.clone();
                                view! {
                                    <For
                                        each=move || {
                                            waypoints.get().into_iter().enumerate().collect::<Vec<_>>()
                                        }
                                        key=|(i, _)| *i
                                        children=move |(i, idx)| {
                                            let on_remove_inner = on_remove_for_list.clone();
                                            view! {
                                                <div class="stop-row">
                                                    <span class="station-name">{format!("{}. {}", i + 1, get_node_name(idx))}</span>
                                                    <button
                                                        class="delete-stop-button"
                                                        on:click=move |_| on_remove_inner(i)
                                                        title="Remove waypoint"
                                                    >
                                                        <i class="fa-solid fa-circle-minus"></i>
                                                    </button>
                                                </div>
                                            }
                                        }
                                    />
                                }.into_view()
                            }
                        }
                    }
                </div>
            </div>

            <div class="form-field">
                <label>"Add Waypoint"</label>
                <select
                    value=selected_node
                    on:change=add_waypoint_from_dropdown
                >
                    <option value="">"Select a station or junction..."</option>
                    <For
                        each=get_all_nodes
                        key=|(idx, _)| idx.index()
                        children=move |(idx, name)| {
                            view! {
                                <option value=idx.index().to_string()>{name}</option>
                            }
                        }
                    />
                </select>
            </div>

            {move || {
                if let Some(error) = validation_error.get() {
                    view! {
                        <div class="view-error-message">
                            {error}
                        </div>
                    }.into_view()
                } else {
                    view! { <div></div> }.into_view()
                }
            }}

            <div class="form-buttons">
                <button on:click=move |_| on_close()>"Cancel"</button>
                <button
                    class="primary"
                    on:click=move |_| create_view()
                    prop:disabled=move || {
                        let wps = waypoints.get();
                        wps.len() < 2 || view_name.get().trim().is_empty() || validation_error.get().is_some()
                    }
                >
                    "Create View"
                </button>
            </div>
        </div>
    }
}

#[component]
#[allow(clippy::too_many_lines)]
pub fn CreateViewDialog(
    is_open: ReadSignal<bool>,
    waypoints: ReadSignal<Vec<NodeIndex>>,
    graph: ReadSignal<RailwayGraph>,
    validation_error: ReadSignal<Option<String>>,
    on_close: Rc<dyn Fn()>,
    on_create: Rc<dyn Fn(String, Vec<NodeIndex>)>,
    on_add_waypoint: Rc<dyn Fn(NodeIndex)>,
    on_remove_waypoint: Rc<dyn Fn(usize)>,
) -> impl IntoView {
    let on_close_for_window = on_close.clone();
    let on_close_for_content = on_close.clone();
    let on_create_for_content = on_create.clone();
    let on_add_waypoint_for_content = on_add_waypoint.clone();
    let on_remove_waypoint_for_content = on_remove_waypoint.clone();

    view! {
        <Window
            is_open=leptos::MaybeSignal::Dynamic(is_open.into())
            title=Signal::derive(|| "Create View".to_string())
            on_close=move || on_close_for_window()
            position_key="create-view"
        >
            <CreateViewDialogContent
                waypoints=waypoints
                graph=graph
                validation_error=validation_error
                on_close=on_close_for_content
                on_create=on_create_for_content
                on_add_waypoint=on_add_waypoint_for_content
                on_remove_waypoint=on_remove_waypoint_for_content
            />
        </Window>
    }
}
