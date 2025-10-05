use crate::components::window::Window;
use crate::models::RailwayGraph;
use leptos::*;
use petgraph::graph::NodeIndex;
use std::rc::Rc;

#[component]
pub fn AddStation(
    is_open: ReadSignal<bool>,
    on_close: Rc<dyn Fn()>,
    on_add: Rc<dyn Fn(String, bool, Option<NodeIndex>)>,
    graph: ReadSignal<RailwayGraph>,
) -> impl IntoView {
    let (station_name, set_station_name) = create_signal(String::new());
    let (is_passing_loop, set_is_passing_loop) = create_signal(false);
    let (connect_to_station, set_connect_to_station) = create_signal(None::<NodeIndex>);

    // Reset form when dialog opens
    create_effect(move |_| {
        if is_open.get() {
            set_station_name.set(format!("Station {}", graph.get().graph.node_count() + 1));
            set_is_passing_loop.set(false);
            set_connect_to_station.set(None);
        }
    });

    let on_close_clone = on_close.clone();
    let handle_add = move |_| {
        let name = station_name.get();
        if !name.is_empty() {
            on_add(name, is_passing_loop.get(), connect_to_station.get());
        }
    };

    view! {
        <Window
            is_open=is_open
            title=Signal::derive(|| "Add New Station".to_string())
            on_close=move || on_close_clone()
            initial_size=(400.0, 300.0)
        >
            <div class="add-station-form">
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
                <div class="form-field">
                    <label>"Connect to (optional)"</label>
                    <select
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
                            current_graph.graph.node_indices().enumerate().map(|(i, idx)| {
                                let name = current_graph.get_station_name(idx).unwrap_or("").to_string();
                                view! {
                                    <option value=i.to_string()>{name}</option>
                                }
                            }).collect::<Vec<_>>()
                        }}
                    </select>
                </div>
                <div class="form-buttons">
                    <button on:click=move |_| on_close()>"Cancel"</button>
                    <button class="primary" on:click=handle_add>"Add"</button>
                </div>
            </div>
        </Window>
    }
}
